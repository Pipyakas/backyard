#!/usr/bin/env python3
"""Agentic performance-optimization eval (Anthropic's original_performance_takehome).

The model gets a disposable copy of evals/perf-takehome and must optimize
KernelBuilder.build_kernel in perf_takehome.py. Only that file is writable;
tests/ and problem.py are read-only (cheating by editing tests or N_CORES
fails verification against the pristine checkout).

Scoring mirrors tests/submission_tests.py thresholds:
  baseline 147734 < updated 18532 < opus4 2164 < opus45_casual 1790
  < opus45_2hr 1579 < sonnet45 1548 < opus45_11hr 1487 < improved 1363
Primary leaderboard metric is perf_speedup (BASELINE/cycles, higher better).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any


DEFAULT_BASE_URL = "http://127.0.0.1:8080/v1"
DEFAULT_MODEL = "default"
BASELINE = 147734
THRESHOLDS = [
    ("updated_start", 18532),
    ("opus4", 2164),
    ("opus45_casual", 1790),
    ("opus45_2hr", 1579),
    ("sonnet45", 1548),
    ("opus45_11hr", 1487),
    ("opus45_improved", 1363),
]
EDITABLE = {"perf_takehome.py"}
PROBE = "from perf_takehome import do_kernel_test; do_kernel_test(6, 4, 64)"

SYSTEM_PROMPT = """You are a performance engineer optimizing a kernel for a simulated machine.
Work only with the tools provided. Rules:
- You may ONLY modify perf_takehome.py (KernelBuilder.build_kernel and helpers).
- NEVER touch tests/, problem.py, or anything else. N_CORES=1 is intentional.
- Validate with run_tests before finishing; iterate on the cycle count.
- Reply with a <tool_call>{"name": ..., "arguments": {...}}</tool_call> per turn, or plain text when done."""

TASK_PROMPT = """Optimize KernelBuilder.build_kernel in perf_takehome.py to minimize CYCLES.

Context: problem.py defines a simulated single-core machine (Engine slots: alu/load/store/flow/debug,
VLEN-wide vectors, scratchpad). perf_takehome.py has a scalar baseline (~147734 cycles). Reference
semantics live in reference_kernel2 — your kernel must produce identical outputs.

Strategy hints: use vector ops, pack multiple slots per bundle (build() packs one slot per bundle by
default — override it), hoist loop-invariant loads, unroll the batch loop.

Thresholds (cycles, lower better): starter 147734, updated-start 18532, opus4 2164, opus45-casual 1790,
opus45-2hr 1579, sonnet45 1548, opus45-11hr 1487, improved-harness 1363.

Plan: 1) read_file perf_takehome.py and problem.py, 2) edit, 3) run_tests, 4) repeat until cycles stop
improving, then stop. You have limited rounds — make each edit count."""


def safe_path(root: Path, raw: str) -> Path:
    path = (root / raw).resolve()
    if path != root and root not in path.parents:
        raise ValueError(f"path escapes workdir: {raw}")
    return path


def compact(text: str, limit: int = 12000) -> str:
    if len(text) <= limit:
        return text
    return text[: limit // 2] + f"\n...[{len(text) - limit} chars truncated]...\n" + text[-limit // 2 :]


def run_subprocess(args: list[str], cwd: Path, timeout: int) -> dict[str, Any]:
    try:
        proc = subprocess.run(args, cwd=cwd, capture_output=True, text=True, timeout=timeout)
        return {"returncode": proc.returncode, "output": compact(proc.stdout + proc.stderr)}
    except subprocess.TimeoutExpired:
        return {"returncode": -1, "output": f"TIMEOUT after {timeout}s"}


def execute_tool(root: Path, name: str, args: dict[str, Any]) -> dict[str, Any]:
    if name == "read_file":
        return {"content": compact(safe_path(root, args["path"]).read_text(encoding="utf-8"))}
    if name == "write_file":
        rel = args["path"].replace("\\", "/").lstrip("./")
        if rel not in EDITABLE:
            return {"error": f"read-only: only {sorted(EDITABLE)} may be modified"}
        path = safe_path(root, args["path"])
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(args["content"], encoding="utf-8", newline="\n")
        return {"written": rel, "bytes": len(args["content"])}
    if name == "run_tests":
        return run_subprocess(
            [sys.executable, "-c", PROBE], root, timeout=600,
        )
    return {"error": f"unknown tool: {name}"}


TOOLS = [
    {"type": "function", "function": {"name": "read_file", "description": "Read a file in the takehome repo.", "parameters": {"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}}},
    {"type": "function", "function": {"name": "write_file", "description": "Rewrite perf_takehome.py (only writable file).", "parameters": {"type": "object", "properties": {"path": {"type": "string"}, "content": {"type": "string"}}, "required": ["path", "content"]}}},
    {"type": "function", "function": {"name": "run_tests", "description": "Run a quick correctness+cycles probe (small config).", "parameters": {"type": "object", "properties": {}}}},
]


def _auth_headers(api_key: str | None) -> dict[str, str]:
    import os as _os

    key = api_key or _os.environ.get("OPENAI_API_KEY") or "not-needed"
    if _os.environ.get("CCGW_PROVIDER") == "ccgw":
        return {"Content-Type": "application/json", "x-ccgw-key": key}
    return {"Content-Type": "application/json", "Authorization": f"Bearer {key}"}


def call_model(base_url: str, model: str, messages: list[dict[str, Any]], max_tokens: int, no_thinking: bool = False, api_key: str | None = None) -> dict[str, Any]:
    payload = {
        "model": model, "messages": messages, "tools": TOOLS, "tool_choice": "auto",
        "temperature": 0.2, "top_p": 0.95, "top_k": 20, "max_tokens": max_tokens, "stream": False,
    }
    if no_thinking:
        payload["chat_template_kwargs"] = {"enable_thinking": False}
    request = urllib.request.Request(
        base_url.rstrip("/") + "/chat/completions",
        data=json.dumps(payload).encode("utf-8"),
        headers=_auth_headers(api_key),
    )
    try:
        with urllib.request.urlopen(request, timeout=240) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        raise RuntimeError(f"model HTTP {error.code}: {error.read().decode('utf-8', errors='replace')}") from error


def parse_xml_tool_call(text: str) -> tuple[str, dict[str, Any]] | None:
    match = re.search(r"<tool_call>\s*(.*?)\s*</tool_call>", text, re.DOTALL)
    if not match:
        return None
    try:
        obj = json.loads(match.group(1).strip())
        return str(obj["name"]), dict(obj.get("arguments", {}))
    except (ValueError, KeyError, TypeError):
        return None


def sha_tree(root: Path, sub: str) -> dict[str, str]:
    result: dict[str, str] = {}
    base = root / sub
    if not base.exists():
        return result
    for path in sorted(p for p in base.rglob("*") if p.is_file()):
        if "__pycache__" in path.parts or path.suffix == ".pyc":
            continue
        result[str(path.relative_to(root)).replace("\\", "/")] = hashlib.sha256(path.read_bytes()).hexdigest()
    return result


def final_scoring(workdir: Path, pristine: Path) -> dict[str, Any]:
    """Run the real submission tests; verify tests/ untouched."""
    tests_clean = sha_tree(workdir, "tests") == sha_tree(pristine, "tests")
    problem_clean = sha_tree(workdir, "problem.py") == sha_tree(pristine, "problem.py")
    # Upstream invocation: `python tests/submission_tests.py` (puts tests/ on sys.path
    # so `from frozen_problem import ...` resolves; -m unittest does NOT).
    res = run_subprocess([sys.executable, "tests/submission_tests.py", "-v"], workdir, timeout=1500)
    out = res.get("output", "")
    cycles = [int(v) for v in re.findall(r"CYCLES:\s*(\d+)", out)]
    best = min(cycles) if cycles else None
    speed_passed = len(re.findall(r"(test_\w+)\s+\(tests\.submission_tests\.SpeedTests\.\w+\)\s+\.\.\.\s+ok", out))
    if not speed_passed:  # non-verbose fallback: count ok lines after SpeedTests ran
        speed_passed = max(0, out.count("... ok") - 1)  # minus correctness test
    correct = "test_kernel_correctness" in out and "FAILED" not in out.split("test_kernel_correctness")[0][-200:]
    return {
        "tests_untouched": tests_clean and problem_clean,
        "correct": bool(correct) and tests_clean,
        "cycles": best,
        "speedup": round(BASELINE / best, 3) if best else 0.0,
        "thresholds_beaten": [n for n, t in THRESHOLDS if best is not None and best < t],
        "speed_tests_passed": speed_passed,
        "raw_tail": out[-3000:],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--takehome-dir", type=Path, default=None)
    parser.add_argument("--base-url", default=DEFAULT_BASE_URL)
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument("--api-key", default=None)
    parser.add_argument("--max-rounds", type=int, default=12)
    parser.add_argument("--max-tokens", type=int, default=4096)
    parser.add_argument("--no-thinking", action="store_true")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()

    src = args.takehome_dir or (Path(__file__).resolve().parent / "perf-takehome")
    if not (src / "perf_takehome.py").exists():
        print(json.dumps({"passed": False, "error": f"takehome not found in {src}"}))
        return 1

    with tempfile.TemporaryDirectory(prefix="perf-takehome-") as directory:
        root = Path(directory).resolve()
        shutil.copytree(src, root, dirs_exist_ok=True, ignore=shutil.ignore_patterns(".git", "__pycache__"))
        messages: list[dict[str, Any]] = [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": TASK_PROMPT},
        ]
        transcript, tool_calls, rounds = [], 0, 0
        error = None
        start = time.perf_counter()
        try:
            for rounds in range(1, args.max_rounds + 1):
                response = call_model(args.base_url, args.model, messages, args.max_tokens, args.no_thinking, args.api_key)
                choice = response["choices"][0]
                message = choice["message"]
                messages.append(message)
                transcript.append({"round": rounds, "message": message})
                calls = list(message.get("tool_calls") or [])
                if not calls:
                    fallback = parse_xml_tool_call(message.get("content") or "")
                    if fallback:
                        calls = [{"id": f"fallback-{rounds}", "function": {"name": fallback[0], "arguments": json.dumps(fallback[1])}}]
                if not calls:
                    break
                for call in calls:
                    tool_calls += 1
                    name = call.get("function", {}).get("name", "")
                    raw_args = call.get("function", {}).get("arguments", {})
                    try:
                        tool_args = json.loads(raw_args) if isinstance(raw_args, str) else raw_args
                        result = execute_tool(root, name, tool_args)
                    except Exception as exc:
                        result = {"error": f"{type(exc).__name__}: {exc}"}
                    transcript.append({"round": rounds, "name": name, "result": result})
                    messages.append({"role": "tool", "tool_call_id": call.get("id", f"c{tool_calls}"), "name": name, "content": json.dumps(result, ensure_ascii=False)})
            else:
                error = f"maximum rounds exceeded ({args.max_rounds})"
        except Exception as exc:
            error = f"{type(exc).__name__}: {exc}"

        scoring = final_scoring(root, src)
        duration = round(time.perf_counter() - start, 2)
        report = {
            "model": args.model, "base_url": args.base_url,
            "passed": scoring["correct"] and scoring["cycles"] is not None and scoring["cycles"] < BASELINE,
            "error": error, "rounds": rounds, "tool_calls": tool_calls,
            "duration_seconds": duration, "scoring": scoring,
            "transcript": transcript,
        }
        encoded = json.dumps(report, indent=2, ensure_ascii=False)
        print(encoded)
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(encoded + "\n", encoding="utf-8")
        return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Small, repeatable coding-agent and tool-call evaluation for a local GGUF server.

The harness uses the OpenAI-compatible API exposed by llama-server, which is the
same API surface consumed by Pi and OpenCode. Each case runs in a disposable
directory and exposes only repository-style tools; arbitrary shell access is not
available to the model.
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
DEFAULT_MODEL = "Mach-1-Additive-35B-Q4_K_M.gguf"


TOOLS = [
    {
        "type": "function",
        "function": {
            "name": "list_files",
            "description": "List repository files below a relative directory.",
            "parameters": {
                "type": "object",
                "properties": {"path": {"type": "string", "default": "."}},
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "read_file",
            "description": "Read a UTF-8 text file in the repository.",
            "parameters": {
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "required": ["path"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "search",
            "description": "Search repository text files with a regular expression.",
            "parameters": {
                "type": "object",
                "properties": {
                    "pattern": {"type": "string"},
                    "path": {"type": "string", "default": "."},
                },
                "required": ["pattern"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "write_file",
            "description": "Write a UTF-8 text file in the repository.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"},
                },
                "required": ["path", "content"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "run_tests",
            "description": "Run the repository's unittest suite and return its output.",
            "parameters": {"type": "object", "properties": {}},
        },
    },
]


CASES: dict[str, dict[str, Any]] = {
    "fix_calculator": {
        "prompt": (
            "Fix the bug in src/calculator.py. Work like a coding agent: inspect "
            "the README and source, run the tests, edit only production code, and "
            "run the tests again until they pass. Do not modify tests. Do not just "
            "describe a patch; make the change with tools."
        ),
        "files": {
            "README.md": (
                "# Invoice calculator\n\n"
                "`apply_discount(price, percent)` applies a percentage discount.\n"
                "`total(items, percent)` applies that discount to every item.\n"
            ),
            "src/calculator.py": (
                "def apply_discount(price: float, percent: float) -> float:\n"
                "    # This implementation has a deliberate percentage bug.\n"
                "    return round(price - percent, 2)\n\n"
                "\n"
                "def total(items: list[float], percent: float = 0) -> float:\n"
                "    return round(sum(apply_discount(item, percent) for item in items), 2)\n"
            ),
            "tests/test_calculator.py": (
                "import unittest\n\n"
                "from src.calculator import apply_discount, total\n\n\n"
                "class CalculatorTests(unittest.TestCase):\n"
                "    def test_percentage_discount(self):\n"
                "        self.assertEqual(apply_discount(100.0, 15), 85.0)\n"
                "        self.assertEqual(total([10.0, 20.0, 30.0], 10), 54.0)\n\n"
                "    def test_zero_discount(self):\n"
                "        self.assertEqual(apply_discount(12.5, 0), 12.5)\n\n\n"
                "if __name__ == '__main__':\n"
                "    unittest.main()\n"
            ),
        },
        "verify": "calculator",
    },
    "config_port": {
        "prompt": (
            "Update the service configuration so the HTTP port is 9090. Inspect "
            "the repository first, change only app/config.json, run the tests, and "
            "report the result. Do not modify tests or README files."
        ),
        "files": {
            "README.md": (
                "# Demo service\n\nThe service reads its HTTP port from app/config.json.\n"
            ),
            "app/config.json": (
                '{\n  "name": "demo",\n  "port": 8080,\n  "workers": 2\n}\n'
            ),
            "tests/test_config.py": (
                "import json\nimport unittest\nfrom pathlib import Path\n\n\n"
                "class ConfigTests(unittest.TestCase):\n"
                "    def test_port_is_9090(self):\n"
                "        config = json.loads(Path('app/config.json').read_text())\n"
                "        self.assertEqual(config['port'], 9090)\n"
                "        self.assertEqual(config['workers'], 2)\n\n\n"
                "if __name__ == '__main__':\n"
                "    unittest.main()\n"
            ),
        },
        "verify": "port",
    },
    "read_only": {
        "prompt": (
            "Use repository tools to find the project version in app/config.json. "
            "This is a read-only task: do not write or change any file. Answer with "
            "the version and its source path."
        ),
        "files": {
            "README.md": "# Read-only fixture\n",
            "app/config.json": '{\n  "name": "agent-eval",\n  "version": "1.4.2"\n}\n',
        },
        "verify": "read_only",
    },
}


SYSTEM_PROMPT = (
    "You are an autonomous coding agent in a disposable repository. "
    "Use the provided repository tools for all inspection and edits. "
    "Never invent tool results. Keep changes narrowly scoped. "
    "Do not modify tests unless the user explicitly permits it. "
    "For coding tasks, run the test tool after editing and do not claim success "
    "until the tests pass. When the task is complete, give a concise summary."
)


def safe_path(root: Path, raw: str) -> Path:
    root = root.resolve()
    if not isinstance(raw, str) or not raw:
        raise ValueError("path must be a non-empty string")
    candidate = (root / raw).resolve()
    if candidate != root and root not in candidate.parents:
        raise ValueError("path escapes the evaluation repository")
    return candidate


def compact(text: str, limit: int = 12000) -> str:
    if len(text) <= limit:
        return text
    return text[:limit] + "\n...[truncated]..."


def execute_tool(root: Path, name: str, args: dict[str, Any]) -> dict[str, Any]:
    if name == "list_files":
        path = safe_path(root, args.get("path", "."))
        if not path.is_dir():
            raise ValueError("not a directory")
        files = sorted(
            str(item.relative_to(root)).replace("\\", "/")
            for item in path.rglob("*")
            if item.is_file() and ".git" not in item.parts
        )
        return {"files": files[:500], "count": len(files)}

    if name == "read_file":
        path = safe_path(root, args["path"])
        if not path.is_file():
            raise FileNotFoundError(args["path"])
        return {"path": args["path"], "content": compact(path.read_text(encoding="utf-8"), 20000)}

    if name == "search":
        base = safe_path(root, args.get("path", "."))
        regex = re.compile(args["pattern"])
        matches: list[dict[str, Any]] = []
        candidates = [base] if base.is_file() else base.rglob("*")
        for item in candidates:
            if not item.is_file() or ".git" in item.parts:
                continue
            try:
                lines = item.read_text(encoding="utf-8").splitlines()
            except UnicodeDecodeError:
                continue
            for line_no, line in enumerate(lines, 1):
                if regex.search(line):
                    matches.append({"path": str(item.relative_to(root)).replace("\\", "/"), "line": line_no, "text": line})
                    if len(matches) >= 100:
                        return {"matches": matches, "truncated": True}
        return {"matches": matches, "truncated": False}

    if name == "write_file":
        path = safe_path(root, args["path"])
        if len(args.get("content", "")) > 100000:
            raise ValueError("content is too large")
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(args["content"], encoding="utf-8", newline="\n")
        return {"written": args["path"], "bytes": path.stat().st_size}

    if name == "run_tests":
        result = subprocess.run(
            [sys.executable, "-m", "unittest", "discover", "-s", "tests", "-v"],
            cwd=root,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=60,
        )
        return {
            "passed": result.returncode == 0,
            "returncode": result.returncode,
            "stdout": compact(result.stdout),
            "stderr": compact(result.stderr),
        }

    raise ValueError(f"unknown tool: {name}")


def _auth_headers(api_key: str | None) -> dict[str, str]:
    # ccgw gateway uses x-ccgw-key; plain OpenAI-compatible servers use Bearer.
    # The harness passes the endpoint's api_key via OPENAI_API_KEY and the
    # provider via CCGW_PROVIDER (set by run_micro_swe/run_perf_takehome).
    import os as _os

    key = api_key or _os.environ.get("OPENAI_API_KEY") or "not-needed"
    if _os.environ.get("CCGW_PROVIDER") == "ccgw":
        return {"Content-Type": "application/json", "x-ccgw-key": key}
    return {"Content-Type": "application/json", "Authorization": f"Bearer {key}"}


def call_model(base_url: str, model: str, messages: list[dict[str, Any]], max_tokens: int, no_thinking: bool = False, api_key: str | None = None) -> dict[str, Any]:
    payload = {
        "model": model,
        "messages": messages,
        "tools": TOOLS,
        "tool_choice": "auto",
        "temperature": 0.2,
        "top_p": 0.95,
        "top_k": 20,
        "max_tokens": max_tokens,
        "stream": False,
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
        body = error.read().decode("utf-8", errors="replace")
        raise RuntimeError(f"model HTTP {error.code}: {body}") from error


def parse_xml_tool_call(text: str) -> tuple[str, dict[str, Any]] | None:
    match = re.search(r"<tool_call>\s*(.*?)\s*</tool_call>", text, re.DOTALL)
    if not match:
        return None
    body = match.group(1).strip()
    try:
        obj = json.loads(body)
        return str(obj["name"]), dict(obj.get("arguments", {}))
    except (ValueError, KeyError, TypeError):
        pass
    function = re.search(r"<function=([^>]+)>(.*?)</function>", body, re.DOTALL)
    if not function:
        return None
    args: dict[str, Any] = {}
    for item in re.finditer(r"<parameter=([^>]+)>\s*(.*?)\s*</parameter>", function.group(2), re.DOTALL):
        args[item.group(1)] = item.group(2).strip()
    return function.group(1).strip(), args


def sha_tree(root: Path) -> dict[str, str]:
    result: dict[str, str] = {}
    for path in sorted(item for item in root.rglob("*") if item.is_file()):
        if "__pycache__" in path.parts or path.suffix == ".pyc":
            continue
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        result[str(path.relative_to(root)).replace("\\", "/")] = digest
    return result


def verify_case(case_id: str, root: Path, before: dict[str, str], final_text: str) -> dict[str, Any]:
    after = sha_tree(root)
    changed = sorted(path for path in set(before) | set(after) if before.get(path) != after.get(path))
    tests_changed = [path for path in changed if path.startswith("tests/")]
    result: dict[str, Any] = {"changed_files": changed, "tests_changed": tests_changed, "passed": False}

    if case_id == "calculator":
        check = subprocess.run(
            [sys.executable, "-c", "from src.calculator import apply_discount,total; assert apply_discount(80,25)==60; assert total([12.5,7.5],20)==16"],
            cwd=root,
            capture_output=True,
            text=True,
            timeout=30,
        )
        result["hidden_check"] = check.returncode == 0
        result["passed"] = check.returncode == 0 and not tests_changed
    elif case_id == "port":
        config = json.loads((root / "app/config.json").read_text(encoding="utf-8"))
        result["port"] = config.get("port")
        result["passed"] = config.get("port") == 9090 and not tests_changed
    elif case_id == "read_only":
        result["version_found"] = "1.4.2" in final_text
        result["passed"] = before == after and result["version_found"]
    return result


def run_case(case_id: str, case: dict[str, Any], base_url: str, model: str, max_rounds: int, max_tokens: int, no_thinking: bool = False, api_key: str | None = None) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix=f"micro-swe-{case_id}-") as directory:
        root = Path(directory).resolve()
        for relative, content in case["files"].items():
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8", newline="\n")
        before = sha_tree(root)
        messages: list[dict[str, Any]] = [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": case["prompt"]},
        ]
        transcript: list[dict[str, Any]] = []
        tool_calls = 0
        tool_errors = 0
        rounds = 0
        start = time.perf_counter()
        final_message: dict[str, Any] = {}
        api_timings: list[dict[str, Any]] = []
        error: str | None = None

        try:
            for rounds in range(1, max_rounds + 1):
                response = call_model(base_url, model, messages, max_tokens, no_thinking, api_key)
                choice = response["choices"][0]
                message = choice["message"]
                final_message = message
                api_timings.append(response.get("timings", {}))
                messages.append(message)
                transcript.append({"round": rounds, "role": "assistant", "message": message, "finish_reason": choice.get("finish_reason")})

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
                        args = json.loads(raw_args) if isinstance(raw_args, str) else raw_args
                        result = execute_tool(root, name, args)
                    except Exception as exc:  # The model should receive tool failures and recover.
                        tool_errors += 1
                        result = {"error": f"{type(exc).__name__}: {exc}"}
                    transcript.append({"round": rounds, "role": "tool", "name": name, "arguments": raw_args, "result": result})
                    messages.append({
                        "role": "tool",
                        "tool_call_id": call.get("id", f"call-{tool_calls}"),
                        "name": name,
                        "content": json.dumps(result, ensure_ascii=False),
                    })
            else:
                error = f"maximum rounds exceeded ({max_rounds})"
        except Exception as exc:
            error = f"{type(exc).__name__}: {exc}"

        final_text = final_message.get("content") or ""
        verification = verify_case(case["verify"], root, before, final_text)
        duration = time.perf_counter() - start
        return {
            "case": case_id,
            "passed": not error and verification["passed"],
            "error": error,
            "rounds": rounds,
            "tool_calls": tool_calls,
            "tool_errors": tool_errors,
            "duration_seconds": round(duration, 2),
            "final_text": final_text,
            "verification": verification,
            "timings": api_timings,
            "transcript": transcript,
        }


def main() -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-url", default=DEFAULT_BASE_URL)
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument("--api-key", default=None)
    parser.add_argument("--case", action="append", choices=sorted(CASES))
    parser.add_argument("--max-rounds", type=int, default=10)
    parser.add_argument("--max-tokens", type=int, default=1024)
    parser.add_argument(
        "--no-thinking", action="store_true",
        help="Send chat_template_kwargs enable_thinking=false (fast tool calls for reasoning models)",
    )
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    no_thinking = args.no_thinking

    import os as _os2
    if _os2.environ.get("CCGW_PROVIDER") is None and args.api_key:
        pass  # provider comes from env (set by harness); key from --api-key
    selected = args.case or list(CASES)
    results = [run_case(case_id, CASES[case_id], args.base_url, args.model, args.max_rounds, args.max_tokens, no_thinking, args.api_key) for case_id in selected]
    report = {
        "model": args.model,
        "base_url": args.base_url,
        "cases": results,
        "passed": all(item["passed"] for item in results),
    }
    encoded = json.dumps(report, indent=2, ensure_ascii=False)
    print(encoded)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded + "\n", encoding="utf-8")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())

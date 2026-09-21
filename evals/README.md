# Eval harnesses

Backyard runs quality evals by shelling out to vendored harnesses in this directory
(`BACKYARD_EVALS_DIR`, see `src/backend/src/harness.rs`). Only the two driver
scripts are tracked in the main repo; upstream checkouts are git submodules.

## Layout

| Path | Source | Purpose |
| ---- | ------ | ------- |
| `micro_swe_tool_eval.py` | tracked | agentic coding + tool-call eval (`micro_swe`) |
| `perf_takehome_eval.py` | tracked | Anthropic kernel-optimization eval (`perf_takehome`) |
| `patches/` | tracked | local patches applied on top of submodules |
| `deep-swe/` | submodule `https://github.com/datacurve-ai/deep-swe.git` @ `435ee89` | long-horizon coding (`deepswe` via `pier run`) |
| `livecodebench/` | submodule `https://github.com/LiveCodeBench/LiveCodeBench.git` @ `28fef95` | code generation (`livecodebench`) |
| `perf-takehome/` | submodule `https://github.com/anthropics/original_performance_takehome` @ `5452f74` | kernel-opt task source |
| `swe-bench/` | submodule `https://github.com/princeton-nlp/SWE-bench.git` @ `7a21e05` | SWE-bench Verified (`swebench`) |
| `tau2-bench/` | submodule `https://github.com/sierra-research/tau2-bench.git` @ `c339866` | tool-agent tasks (`tau2_*`) |
| `jobs/` | ignored | `harbor`/`pier` run outputs (`result.json` parsed by the worker) |
| `results/` | ignored | `--output` JSON from the two driver scripts |

`tbench` needs no vendored tasks: `harness.rs` calls `harbor run -d
terminal-bench/terminal-bench-2`, which pulls the dataset from the Harbor Hub
at runtime. A stale 413MB `terminal-bench/tasks/` snapshot may exist in older
working trees — it is ignored and safe to delete.

## Setup

```bash
git submodule update --init --recursive
# fresh clones: the livecodebench checkout needs the local endpoint patch
# (already applied in this working tree; the file below is the source of truth):
git -C evals/livecodebench apply ../../patches/livecodebench-backyard-endpoint.patch
```

## Patches

- `patches/livecodebench-backyard-endpoint.patch` — routes the LCB OpenAI runner
  through `OPENAI_BASE_URL`/`BACKYARD_MODEL` so local endpoints work, and makes
  the `anthropic` prompt imports optional (API mode needs no anthropic SDK).

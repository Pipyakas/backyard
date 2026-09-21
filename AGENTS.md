# AGENTS.md

This file is the operating guide for coding agents working in this repository.

## 1) What This Repo Is

Backyard is a local-first **speedtest / eval benchmark hub for local models on
local hardware** — Artificial Analysis, but everything runs against your own
endpoints. A Rust/Axum binary serves a vanilla-JS leaderboard UI and a queue
worker that runs speed tests (HTTP tok/s, latency, TTFT) and quality evals
(agentic coding, terminal-bench, tau2, livecodebench, …) against any
OpenAI-compatible inference endpoint (llama.cpp server, vLLM, Ollama, ccgw).

The old Tauri desktop app lives on the `legacy/desktop-app` branch. `main`
has no Tauri, no Docker orchestration, no model library management.

## 2) Project Structure

```
compose.yml / Containerfile   # single image `backyard-web` (:8080, /data volume)
src/
  frontend/       # Vanilla JS web UI (Vite build, served by Axum)
    static/       # CSS, JS (app.js), assets
    templates/    # HTML templates (index.html, auth.html)
  backend/        # Rust/Axum benchmark hub (binary `backyard`)
    src/
      main.rs     # tokio entry point: init State, start server
      state/      # SQLite state (rusqlite): runs, results, eval_endpoints, machines
      server/     # Axum routes + SPA fallback + static serving
      bench_http.rs # speedtest vs OpenAI-compatible endpoints
      harness.rs  # quality-eval subprocess drivers (harbor/pier/python)
      worker.rs   # 2s queue poller: bench vs eval dispatch
      auth.rs     # argon2 admin login, `backyard_session` cookie
evals/            # harness checkouts (submodules) + our two driver scripts
  README.md       # submodule pins, patch workflow
  micro_swe_tool_eval.py / perf_takehome_eval.py
  patches/… no — patches live in ./patches/ (livecodebench endpoint patch)
patches/          # local patches applied on top of eval submodules
containers/       # llama.cpp CUDA/ROCm Dockerfiles (reference; not orchestrated)
docs/             # hardware notes (ROCm)
```

## 3) Build Commands

```bash
cd src/frontend && npm install && npm run build
cd ../backend && cargo build --release
# Or for development:
cd src/backend && cargo run
```

Or via compose: `docker compose up --build` (serves on `127.0.0.1:8080`).

## 4) Architecture

- **Core abstraction**: `eval_endpoints` rows (`name, base_url, api_key, model,
  provider`) — BYO inference server, anything OpenAI-compatible.
- **Speedtest**: `POST /api/benchmarks` queues `runs(kind='bench')`;
  `bench_http.rs` loops chat completions, records `tps, latency_ms/p50/p95,
  ttft_ms, success_rate` into `results`.
- **Quality evals**: `POST /api/evals/runs` queues `runs(kind='eval')`;
  `worker.rs` hands off to `harness.rs` on the blocking pool, which shells out
  to `harbor` / `pier` / `python3` and parses stdout/result files into metrics.
- **Leaderboard**: `GET /api/leaderboard` returns latest `done` value per
  `model_label` for each known metric, ordered desc. Frontend merges speed
  buckets into one row per model × endpoint, renders AA-style tables.
- **Database**: SQLite via rusqlite at `BACKYARD_DATA_DIR/data.db`
  (default `~/.backyard/data.db`).

## 5) Test Commands

```bash
cd src/backend && cargo test
```

## 6) Type Checking

```bash
cd src/frontend && npx tsc --noEmit && node --check static/js/app.js
cd src/backend && cargo check
```

## 7) Code Style Guidelines

### Rust (backend)
- **Naming**: `camelCase` functions/variables, `PascalCase` types, `snake_case` database tables
- **Error handling**: Use `anyhow::Result` with `.context()` for all operations
- **Platform checks**: Use `#[cfg(target_os = "...")]` for platform-specific code
- **Home directory**: Use `USERPROFILE` on Windows, `HOME` on Linux (with cfg! macros)

### JavaScript (frontend)
- Use `async/await` for all fetch operations
- Always handle fetch failure paths and show UI feedback via `showNotification`
- Validate user input before sending to API

## 8) API Design Patterns

- **Response format**: Return JSON consistently:
  - Success: `{ success: true, ...data }`
  - Failure: `{ success: false, error: "..." }`
- **HTTP status codes**: Use appropriate codes (200, 400, 404, 500)
- **Input validation**: Validate incoming JSON in route handlers

## 9) Key Environment Variables

- `BACKYARD_HOST` (default `0.0.0.0`) - Axum server bind address
- `PORT` / `BACKYARD_PORT` (default `8080`) - Axum server port
- `BACKYARD_DATA_DIR` (default `~/.backyard`) - SQLite + state directory
- `BACKYARD_EVALS_DIR` (default `<repo>/evals`) - harness checkout directory
- `BACKYARD_AUTH_DISABLED=1` - single-user mode, skip admin auth (compose default)
- `FRONTEND_DIST` - override frontend bundle path

## 10) Quick Command Cheat Sheet

```bash
# Development
cd src/backend && cargo run                  # API + worker on :8080
cd src/frontend && npm run dev               # Vite dev server (standalone)

# Evals setup (fresh clone)
git submodule update --init --recursive
git -C evals/livecodebench apply ../../patches/livecodebench-backyard-endpoint.patch

# Type checking
cd src/frontend && npx tsc --noEmit
cd src/backend && cargo check
```

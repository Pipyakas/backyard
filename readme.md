# Backyard

Local-first speedtest / eval benchmark hub for local models running on local
hardware — Artificial Analysis, but everything runs against your own endpoints.

Point Backyard at any OpenAI-compatible inference server (llama.cpp `--server`,
vLLM, Ollama, ccgw, …), queue speed tests and quality evals from the web UI,
and compare models on the leaderboard.

## Quick Start

### 1. Clone the repository

```bash
git clone git@github.com:Pipyakas/backyard.git
cd backyard
git submodule update --init --recursive
# apply the local livecodebench endpoint patch:
git -C evals/livecodebench apply ../../patches/livecodebench-backyard-endpoint.patch
```

### 2. Run with compose

```bash
docker compose up --build
```

Access the app at: **http://127.0.0.1:8080**

Or build locally:

```bash
cd src/frontend && npm install && npm run build
cd ../backend && cargo run
```

### 3. Add an endpoint, run a benchmark

1. Open the **Endpoints** tab, add your inference server
   (e.g. `http://127.0.0.1:8095`, model alias, optional API key).
2. **Speed** tab → Run Speed Test (tok/s, latency p50/p95, TTFT, success rate).
3. **Quality** tab → Run Quality Eval (`micro_swe`, `tbench`, `tau2_*`,
   `livecodebench`, `swebench`, `deepswe`, `perf_takehome`, …).
4. **Leaderboard** tab → compare models across endpoints, AA-style.

## Project Architecture

- **Backend (`src/backend`):** Rust binary (`axum`, `tokio`, `rusqlite`).
  Serves the UI, runs a queue worker for benchmarks.
- **Frontend (`src/frontend`):** Vanilla JS + Vite, served by the backend.
- **Evals (`evals/`):** vendored harnesses as git submodules plus two local
  driver scripts; see `evals/README.md`.
- **Data:** SQLite at `BACKYARD_DATA_DIR/data.db` (`~/.backyard` by default,
  `/data` in compose).

## API Endpoints

- `GET  /api/health` — liveness.
- `GET  /api/endpoints` / `POST /api/endpoints` / `DELETE /api/endpoints/{id}` — manage inference endpoints.
- `POST /api/endpoints/{id}/health` — probe (`/v1/models`, then minimal chat).
- `POST /api/benchmarks` — queue a speed test (`endpoint_id`, `prompt`, `iterations`, `max_tokens`, `temperature`, `stream`).
- `POST /api/evals/runs` — queue a quality eval (`endpoint_id`, `harness`, `config`).
- `GET  /api/runs` / `GET /api/runs/{id}` / `GET /api/runs/{id}/results` — runs and parsed metrics.
- `GET  /api/leaderboard` — latest score per model per metric.
- `POST /api/system/reset` — clear runs + results.

(`GET /api/machines` remains as a read-only compat shim for the local-machine row.)

## Environment

- `BACKYARD_HOST` (default `0.0.0.0`), `PORT` / `BACKYARD_PORT` (default `8080`)
- `BACKYARD_DATA_DIR` (default `~/.backyard`), `BACKYARD_EVALS_DIR` (default `<repo>/evals`)
- `BACKYARD_AUTH_DISABLED=1` — single-user mode, skip admin login

## Legacy desktop app

The old Tauri desktop shell lives on the `legacy/desktop-app` branch.
`main` is the web-only benchmark hub.

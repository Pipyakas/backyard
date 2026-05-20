# AGENTS.md

This file is the operating guide for coding agents working in this repository.

## 1) What This Repo Is

Backyard is a local-first LLM lab desktop app with:
- Tauri v2 desktop shell (cross-platform Windows + Linux)
- Rust/Axum backend API (`src/backend/src/`)
- Vanilla JS frontend with Vite (`src/frontend/`)
- SQLite persistence with rusqlite (`~/.backyard/data.db`)
- System tray with background running support

## 2) Project Structure

```
.github/
src/
  frontend/       # Vanilla JS web UI (Vite)
    static/       # CSS, JS, assets
    templates/    # HTML templates (index.html, auth.html)
  backend/        # Rust/Axum backend (Tauri desktop app + API)
    src/
      main.rs     # Tauri entry point, system tray, window management
      state/      # Database state (rusqlite)
      server/     # Axum HTTP server and API routes
      cli/        # CLI subcommands
      orchestrator/ # Docker container management
      memory.rs   # RAM usage tracking
      config.rs   # App config persistence
    Cargo.toml
    tauri.conf.json
    capabilities/
  scripts/        # Build + run scripts
    Makefile      # Build orchestration
    build.bat     # Windows build script
    run.bat       # Windows run script
containers/       # Dockerfiles for various engines
docs/             # Documentation and benchmarks
```

## 3) Build Commands

```bash
make bootstrap                 # Install all dependencies
make build                     # Build frontend + Tauri desktop app
make dev                       # Run in Tauri dev mode (frontend + desktop)
make frontend                  # Build frontend only
make install                   # Install binary to PATH (Linux/macOS)
```

Or directly:
```bash
cd src/frontend && npm install && npm run build
cd src/backend && cargo build --release
# Or for development:
cd src/backend && cargo run
```

## 4) Architecture

- **Desktop app**: Tauri v2 wraps the web frontend in a native window
- **Backend API**: Axum HTTP server runs on `localhost:5556` in a Tauri background task
- **System tray**: Minimizes to tray on close; tray menu has Show/Hide/Quit
- **Frontend**: Vanilla JS makes `fetch()` calls to the Axum API
- **Database**: SQLite via rusqlite, stored at `$HOME/.backyard/data.db` (Linux) or `%USERPROFILE%\.backyard\data.db` (Windows)

## 5) Platform Support

- **Windows**: Full support via Tauri; Docker/GPU features require Docker Desktop + WSL2
- **Linux**: Full support including systemd service management and GPU passthrough
- System tray works on both Windows and GNOME/KDE (requires `libayatana-appindicator3-1` on Linux)

## 6) Test Commands

```bash
cd src/backend && cargo test
```

## 7) Type Checking

```bash
cd src/frontend && npx tsc --noEmit
cd src/backend && cargo check
```

## 8) Code Style Guidelines

### Rust (backend)
- **Naming**: `camelCase` functions/variables, `PascalCase` types, `snake_case` database tables
- **Error handling**: Use `anyhow::Result` with `.context()` for all operations
- **Platform checks**: Use `#[cfg(target_os = "...")]` for platform-specific code
- **Home directory**: Use `USERPROFILE` on Windows, `HOME` on Linux (with cfg! macros)

### JavaScript (frontend)
- Use `async/await` for all fetch operations
- Always handle fetch failure paths and show UI feedback via `showNotification`
- Validate user input before sending to API

## 9) API Design Patterns

- **Response format**: Return JSON consistently:
  - Success: `{ success: true, ...data }`
  - Failure: `{ success: false, error: "..." }`
- **HTTP status codes**: Use appropriate codes (200, 400, 404, 500)
- **Input validation**: Validate incoming JSON in route handlers

## 10) Key Environment Variables

- `BACKYARD_HOST` (default `127.0.0.1`) - Axum server bind address
- `BACKYARD_PORT` (default `5556`) - Axum server port
- `HF_TOKEN` - HuggingFace token for downloads

## 11) Quick Command Cheat Sheet

```bash
# Development
cd src/backend && cargo run      # Tauri dev mode
cd src/frontend && npm run dev     # Vite dev server (standalone)

# Building
make build                         # Full build
cd src/backend && cargo build --release  # Tauri release build

# Type checking
cd src/frontend && npx tsc --noEmit
cd src/backend && cargo check
```

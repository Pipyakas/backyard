# Backyard

Unified Rust-based backend and modern Web UI for managing, benchmarking, and deploying local LLM inference engines (llama.cpp) via Docker.

## Project Architecture
- **Backend (`src/backend`):** High-performance Rust binary using `axum`, `tokio`, and `bollard`.
- **Frontend (`src/frontend`):** Modernized Web UI served directly by the Rust backend.
- **Unified Library Logic:** Supports multiple storage locations across different drives with recursive scanning.
- **Mixed GPU Support:** Dynamic hardware passthrough for both NVIDIA (CUDA) and AMD (ROCm) via Docker.

## Quick Start

### 1. Clone the repository
```bash
git clone git@github.com:Pipyakas/backyard.git
cd backyard
```

### 2. Build and Install
Use the provided Makefile to bootstrap dependencies and build the unified binary.
```bash
make bootstrap  # Install frontend dependencies
make build      # Build backend (Rust) and frontend (Vite)
make install    # Link CLI to ~/.local/bin
```

### 3. Run the Application
```bash
backyard
```
Access the app at: **http://localhost:5556**

## System Service (Linux)

Install Backyard as a **systemd user service** for background operation.

```bash
# Install the user service
backyard service install

# Control the service
backyard service start
backyard service stop
backyard service status

# View logs
backyard service log
```

## CLI Usage

The `backyard` command provides full system control:

- `backyard`: Starts the web server and API (default).
- `backyard models`: Lists registered models with family and quantization info.
- `backyard download <repo_id> <filename>`: Trigger a background model download from Hugging Face.
- `backyard service install`: Installs the systemd unit file.
- `backyard service start|stop|status`: Controls the background process.
- `backyard service log`: Streams service logs.

## API Endpoints

- `GET  /api/libraries`: List storage libraries.
- `POST /api/libraries/scan`: Recursively find and register models.
- `GET  /api/models`: Get unified model list with metadata.
- `POST /api/models/download`: Trigger a model download.
- `POST /api/engines/start`: Launch a Docker inference container.
- `POST /api/engines/build`: Rebuild Docker images (CUDA/ROCm) on-demand.
- `POST /api/benchmarks`: Run `llama-bench` on a specific model.
- `POST /api/system/reset`: Clear the system database.

## Docker Engines
Hardware-accelerated inference is provided by Dockerfiles in `containers/llama.cpp/`. The backend dynamically selects the appropriate image based on your GPU type (NVIDIA or AMD).

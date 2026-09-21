# syntax=docker/dockerfile:1

# --- Frontend builder ---
FROM docker.io/node:20-alpine AS frontend
WORKDIR /app/src/frontend
COPY src/frontend/package.json src/frontend/package-lock.json* ./
RUN npm ci 2>/dev/null || npm install
COPY src/frontend/ ./
RUN npm run build

# --- Backend builder ---
FROM docker.io/rust:1.88-bookworm AS backend
WORKDIR /app
COPY src/backend/Cargo.toml src/backend/Cargo.lock* src/backend/
# Cache deps
RUN mkdir -p src/backend/src && echo "fn main(){}" > src/backend/src/main.rs && echo "fn main(){}" > src/backend/build.rs
RUN cargo build --manifest-path src/backend/Cargo.toml --release 2>/dev/null || true
# Real source
COPY src/backend/ src/backend/
COPY --from=frontend /app/src/frontend/dist src/frontend/dist
RUN cargo build --manifest-path src/backend/Cargo.toml --release

# --- Runtime ---
FROM docker.io/debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates python3 python3-pip libsqlite3-0 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=backend /app/src/backend/target/release/backyard /usr/local/bin/backyard
COPY --from=frontend /app/src/frontend/dist /app/frontend/dist
COPY src/frontend/static /app/frontend/dist/static
COPY evals /app/evals
# Harness python deps (best-effort; lcb-runner needed for livecodebench).
# Install from the baked evals/livecodebench checkout (no vllm/torch needed for API mode).
RUN pip3 install --no-cache-dir --break-system-packages requests datasets openai pebble anthropic cohere google-genai "mistralai==0.4.2" together tokenizers 2>/dev/null \
 || pip3 install --no-cache-dir requests datasets openai pebble anthropic cohere google-genai "mistralai==0.4.2" together tokenizers 2>/dev/null || true
RUN pip3 install --no-cache-dir --break-system-packages torch --index-url https://download.pytorch.org/whl/cpu 2>/dev/null || true
RUN cd /app/evals/livecodebench && pip3 install --no-cache-dir --no-deps --break-system-packages -e . 2>/dev/null \
 || (cd /app/evals/livecodebench && pip3 install --no-cache-dir --no-deps -e .) 2>/dev/null || true
ENV PORT=8080 BACKYARD_DATA_DIR=/data BACKYARD_EVALS_DIR=/app/evals
EXPOSE 8080
VOLUME ["/data"]
CMD ["backyard"]

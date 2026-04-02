# AGENTS.md

This file is the operating guide for coding agents working in this repository.

## 1) What This Repo Is

Backyard is a local-first LLM lab app with:
- TypeScript/Fastify backend API (`apps/api/`)
- Vanilla JS frontend with Vite (`apps/web/`)
- SQLite persistence with Drizzle ORM (`~/.local/share/backyard/data/db/`)
- Unified CLI for management (`apps/api/bin/backyard`)

## 2) Project Structure

```
apps/
  api/           # TypeScript/Fastify backend
    src/
      index.ts   # Entry point
      app.ts     # Fastify app setup
      db/        # Database (Drizzle ORM + better-sqlite3)
      routes/    # API routes
      services/  # Business logic
    bin/
      backyard   # Unified CLI
  web/           # Vanilla JS frontend with Vite
    src/
      app.ts     # Main frontend logic
      api/       # API client
      types/     # TypeScript types
containers/      # Dockerfiles for various engines
docs/            # Documentation and benchmarks
```

## 3) Build Commands

```bash
make bootstrap                 # Install all dependencies
make build                     # Build both api and web
make install                   # Link backyard CLI to ~/.local/bin
```

## 4) Test Commands

```bash
make test                      # Run all E2E tests
backyard status                # Check service status via CLI
backyard service log           # Check logs via CLI
```

## 5) Type Checking

```bash
cd apps/api && npx tsc --noEmit
cd apps/web && npx tsc --noEmit
```

## 6) Code Style Guidelines

### TypeScript (backend)
- **Types**: Use explicit types, avoid `any`. Define interfaces for all data structures.
- **Imports**: Use ES modules with `.js` extension (e.g., `import { foo } from './bar.js'`)
- **Naming**: `camelCase` functions/variables, `PascalCase` types, `snake_case` database tables
- **Error handling**: Return `{ success: false, error: "..." }` for API failures
- **Validation**: Use Zod schemas for input validation (see `apps/api/src/routes/models.ts`)
- **Logging**: Use Fastify's built-in logger (`app.log.info`, `app.log.error`)

### JavaScript (frontend)
- Use `async/await` for all fetch operations
- Always handle fetch failure paths and show UI feedback via `showNotification`
- Validate user input before sending to API

### Shell scripts
- Use `#!/usr/bin/env bash` and `set -euo pipefail`
- Quote all variable expansions: `"$variable"`
- Prefer idempotent behavior

## 7) API Design Patterns

- **Response format**: Always return JSON consistently:
  - Success: `{ success: true, ...data }`
  - Failure: `{ success: false, error: "..." }`
- **Input validation**: Use Zod schemas to validate incoming JSON
- **HTTP status codes**: Use appropriate codes (200, 400, 404, 500)
- **Avoid**: Never expose raw stack traces to clients

## 8) Error Handling + Logging

- Backend uses Fastify's built-in logger
- Wrap all subprocess and file operations in try/catch
- Log concise context with failures (include relevant IDs, names, or state)
- For long-running operations (downloads, benchmarks), set meaningful status: `queued` → `running` → `completed`/`failed`

## 9) Persistence and State Rules

- All runtime data must go through SQLite with Drizzle ORM
- Keep status transitions explicit: `queued/running/completed/failed`
- Define table schemas in `apps/api/src/db/schema.ts`
- Use migrations: `npm run db:generate` and `npm run db:migrate`

## 10) Repo-Specific Guardrails

- Do not commit model weights, secrets, or credentials
- `.env` and `*.key` files are gitignored
- Avoid editing `engines/*` unless explicitly required

## 11) API-First Principle

**Always prioritize using Backyard's own APIs before resorting to bash commands.** We are building functionalities into the app — use the app to do things.

- Use `curl` or the frontend to interact with models, libraries, downloads, servers, benchmarks, and engines via their API endpoints
- Only use direct `sqlite3` commands, filesystem manipulation, or shell scripts when:
  - No API endpoint exists yet for the operation
  - You are debugging or investigating state
  - The task is explicitly about bootstrapping or infrastructure setup
- If you find yourself needing to do something via bash that should be an API action, implement the API endpoint first, then use it
- This keeps behavior consistent, testable, and aligned with how users will actually interact with the app

## 12) Environment Variables

Key env vars (defined in `apps/api/src/app.ts`):
- `BACKYARD_HOST` (default `0.0.0.0`)
- `BACKYARD_PORT` (default `5556`)
- `BACKYARD_MODELS_DIR` (default `~/.local/share/backyard/models`)
- `BACKYARD_DB_PATH` (default `~/.local/share/backyard/data/db/backyard.db`)
- `SECRET_KEY` - Cookie signing (auto-generated if missing)
- `BACKYARD_ADMIN_API_KEY` - Admin API key
- `HF_TOKEN` - HuggingFace token for downloads

## 13) Typical Agent Workflow

1. Read impacted backend/frontend files before changing behavior
2. Implement smallest safe change
3. **Rebuild the application**: `make build`
4. **Interact via the CLI**: Use the `backyard` binary to verify changes (e.g., `backyard models`, `backyard service restart`)
5. **Verify with Tests**: `make test`
6. Run type check: `npx tsc --noEmit`
7. Verify frontend changes visually

## 14) Quick Command Cheat Sheet

```bash
# Development
cd apps/api && npm run dev      # Backend
cd apps/web && npm run dev      # Frontend
make dev                        # Both

# Testing
cd apps/api && npm run test:e2e
cd apps/api && npx playwright test tests/e2e/dashboard.spec.ts

# Type checking
cd apps/api && npx tsc --noEmit
cd apps/web && npx tsc --noEmit

# Health check
backyard service status
```

When in doubt: prefer small, reversible changes; verify with a focused command; keep API contracts stable.


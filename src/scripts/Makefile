.PHONY: help bootstrap build dev install clean frontend

help: ## Show this help message
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-15s\033[0m %s\n", $$1, $$2}'

bootstrap: ## Install dependencies for frontend and Tauri
	cd ../frontend && npm install
	cd ../backend && cargo fetch

build: ## Build frontend and Tauri desktop app
	cd ../frontend && npm run build
	cd ../backend && cargo build --release

dev: ## Run Tauri dev mode (frontend + desktop app)
	cd ../backend && cargo run

install: build ## Install backyard to system PATH
	@echo "Copying binary to install location..."
	@if [ "$$(uname)" = "Linux" ] || [ "$$(uname)" = "Darwin" ]; then \
		mkdir -p ~/.local/bin && \
		cp -f ../backend/target/release/backyard ~/.local/bin/backyard && \
		echo "Installed to ~/.local/bin/backyard"; \
	else \
		echo "On Windows, run: copy ..\\backend\\target\\release\\backyard.exe %USERPROFILE%\\.local\\bin\\backyard.exe"; \
	fi

clean: ## Remove build artifacts
	rm -rf ../frontend/dist ../backend/target
	find . -type d -name "node_modules" -exec rm -rf {} +

frontend: ## Build frontend only
	cd ../frontend && npm run build

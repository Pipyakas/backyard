.PHONY: help bootstrap build install clean logs status

help: ## Show this help message
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-15s\033[0m %s\n", $$1, $$2}'

bootstrap: ## Install dependencies for frontend
	cd src/frontend && npm install

build: ## Build backend (Rust) and frontend (Vite)
	cd src/frontend && npm run build
	cd src/backend && cargo build --release

install: build ## Link backyard CLI to ~/.local/bin
	mkdir -p ~/.local/bin
	ln -sf $(CURDIR)/src/backend/target/release/backyard ~/.local/bin/backyard
	@echo "Linked backyard CLI to ~/.local/bin/backyard"

clean: ## Remove build artifacts
	rm -rf src/frontend/dist src/backend/target
	find . -type d -name "node_modules" -exec rm -rf {} +

logs: ## View backend logs
	backyard service log

status: ## Check service status
	backyard service status

# Developer entry points. Every target here is also what CI runs, so a
# green `make check` locally means a green pipeline.

SHELL := /bin/bash
.DEFAULT_GOAL := help

.PHONY: help setup db-up db-down db-reset api web dev check check-rust check-web e2e

help: ## List available targets
	@grep -hE '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'

setup: ## Install web dependencies and create .env from the example
	@test -f .env || (cp .env.example .env && echo "created .env from .env.example")
	pnpm install --frozen-lockfile

db-up: ## Start PostgreSQL and wait until it accepts connections
	docker compose up -d --wait postgres

db-down: ## Stop PostgreSQL, keeping its data volume
	docker compose stop postgres

db-reset: ## Delete the development database and its data volume
	docker compose down -v

api: ## Run the API (applies pending migrations on start)
	cargo run --bin homecloud-api

web: ## Run the web app against the API
	pnpm --filter @homecloud/web dev

dev: db-up ## Start PostgreSQL, the API, and the web app together
	./scripts/dev.sh

check: check-rust check-web ## Run every gate CI runs, except end-to-end tests

check-rust: ## Format, lint, and test the Rust workspace
	cargo fmt --all --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	cargo test --workspace

check-web: ## Lint, typecheck, and test the web app
	pnpm --filter @homecloud/web lint
	pnpm --filter @homecloud/web typecheck
	pnpm --filter @homecloud/web test

e2e: ## Run the browser end-to-end tests
	pnpm --filter @homecloud/web test:e2e

# Developer entry points. Every target here is also what CI runs, so a
# green `make check` locally means a green pipeline.

SHELL := /bin/bash

# Connection used only to create and drop the end-to-end test database.
HOMECLOUD_E2E_ADMIN_URL ?= postgres://homecloud:homecloud@127.0.0.1:5432/postgres
.DEFAULT_GOAL := help

.PHONY: help setup db-up db-down db-reset api web dev check check-rust check-web e2e e2e-full

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

e2e: ## Run the browser tests that need no server
	pnpm --filter @homecloud/web test:e2e

e2e-full: ## Run the full-stack journeys (API + PostgreSQL + web)
	# A fresh database each run: these journeys start at first-run setup.
	psql "$(HOMECLOUD_E2E_ADMIN_URL)" -c 'DROP DATABASE IF EXISTS homecloud_e2e'
	psql "$(HOMECLOUD_E2E_ADMIN_URL)" -c 'CREATE DATABASE homecloud_e2e'
	rm -rf apps/web/.playwright-library
	pnpm --filter @homecloud/web test:e2e:full

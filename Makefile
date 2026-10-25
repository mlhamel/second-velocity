# Second Velocity: The Living Galaxy — Workspace Makefile

CARGO   ?= cargo
PYTHON  ?= python3
PORT    ?= 3456
DB      ?= shard.db
HOST    ?= 127.0.0.1:$(PORT)
USER    ?= pilot
PASS    ?= secret

.DEFAULT_GOAL := help

.PHONY: help \
	build build-release build-core build-data build-protocol build-server build-client build-mcp \
	test test-core test-data test-protocol test-server test-mcp test-playthrough \
	bench bench-json check lint clippy fmt fmt-check commit-check hooks \
	run run-client run-online run-server run-server-ephemeral run-mcp \
	tour capture-tour art placeholder-art clean

##@ General

help: ## Show available targets and configurable variables
	@awk 'BEGIN {FS = ":.*##"; printf "\nUsage:\n  make \033[36m<target>\033[0m [PORT=3456] [DB=shard.db] [HOST=127.0.0.1:3456] [USER=pilot] [PASS=secret] [SCENARIO=id]\n"} \
		/^[a-zA-Z0-9_-]+:.*?##/ { printf "  \033[36m%-22s\033[0m %s\n", $$1, $$2 } \
		/^##@/ { printf "\n\033[1m%s\033[0m\n", substr($$0, 5) } ' $(MAKEFILE_LIST)

hooks: ## Configure git to use tracked hooks in .githooks/ (and install cocogitto hook if cog is present)
	chmod +x .githooks/commit-msg
	git config core.hooksPath .githooks

##@ Build Targets

build: ## Build all workspace crates (debug)
	$(CARGO) build --workspace

build-release: ## Build all workspace crates (release)
	$(CARGO) build --workspace --release

build-core: ## Build sv-core (headless simulation math)
	$(CARGO) build -p sv-core

build-data: ## Build sv-data (RON content schema and loader)
	$(CARGO) build -p sv-data

build-protocol: ## Build sv-protocol (replicated components and intents)
	$(CARGO) build -p sv-protocol

build-server: ## Build sv-server (standalone authoritative shard)
	$(CARGO) build -p sv-server

build-client: ## Build sv-client (Bevy desktop client)
	$(CARGO) build -p sv-client

build-mcp: ## Build sv-mcp (MCP agent server)
	$(CARGO) build -p sv-mcp

##@ Testing & Verification

check: fmt-check lint commit-check test ## Run full verification suite (fmt-check + clippy + commit-check + workspace tests)

bench: ## Run gameplay benchmark scenarios and print Markdown table (optional SCENARIO=id)
	$(CARGO) run -p sv-mcp -- --bench $(SCENARIO)

bench-json: ## Run gameplay benchmark scenarios and print JSON report (optional SCENARIO=id)
	$(CARGO) run -p sv-mcp -- --bench $(SCENARIO) --json

commit-check: ## Verify git history follows Conventional Commits (via cocogitto)
	@if command -v cog >/dev/null 2>&1; then \
		cog check; \
	else \
		git log --format=%s | grep -Ev '^(Merge|Revert|fixup!|squash!) ' | grep -Ev '^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\([a-zA-Z0-9_,./-]+\))?!?: .+' && { echo "Non-conventional commits found!" >&2; exit 1; } || echo "All commits follow Conventional Commits."; \
	fi

test: ## Run all unit and integration tests across the workspace
	$(CARGO) test --workspace

test-core: ## Run sv-core unit tests
	$(CARGO) test -p sv-core

test-data: ## Run sv-data content pack validation tests
	$(CARGO) test -p sv-data

test-protocol: ## Run sv-protocol serialization roundtrip tests
	$(CARGO) test -p sv-protocol

test-server: ## Run sv-server gameplay, networking, and SQLite persistence tests
	$(CARGO) test -p sv-server

test-mcp: ## Run sv-mcp integration tests
	$(CARGO) test -p sv-mcp

test-playthrough: ## Run MCP agent playthrough integration tests with output
	$(CARGO) test -p sv-mcp --test playthrough -- --nocapture

lint: ## Run clippy across all workspace targets with warnings denied
	$(CARGO) clippy --workspace --all-targets -- -D warnings

clippy: lint ## Alias for lint

fmt: ## Format all Rust code in the workspace
	$(CARGO) fmt --all

fmt-check: ## Check formatting without modifying files
	$(CARGO) fmt --all --check

##@ Run & Workflows

run: run-client ## Launch offline single-player client (alias for run-client)

run-client: ## Launch Bevy desktop client with embedded server (offline single-player)
	$(CARGO) run -p sv-client

run-verbose: ## Launch offline single-player client with --verbose logging
	$(CARGO) run -p sv-client -- --verbose

run-online: ## Connect client to a shard (override HOST, USER, PASS)
	$(CARGO) run -p sv-client -- --connect $(HOST) --username $(USER) --password $(PASS)

run-online-verbose: ## Connect client to a shard with --verbose logging (override HOST, USER, PASS)
	$(CARGO) run -p sv-client -- --connect $(HOST) --username $(USER) --password $(PASS) --verbose

run-server: ## Run standalone UDP server with SQLite persistence (override PORT, DB)
	$(CARGO) run -p sv-server -- $(PORT) --storage $(DB)

run-server-verbose: ## Run standalone UDP server with SQLite persistence and --verbose logging
	$(CARGO) run -p sv-server -- $(PORT) --storage $(DB) --verbose

run-server-ephemeral: ## Run standalone UDP server with in-memory storage (override PORT)
	$(CARGO) run -p sv-server -- $(PORT)

run-mcp: ## Run sv-mcp server on stdio
	$(CARGO) run -p sv-mcp

tour: capture-tour ## Capture automated screenshot tour (alias for capture-tour)

capture-tour: ## Run client in automated screenshot tour mode (outputs to docs/screenshots/)
	$(CARGO) run -p sv-client -- --capture-tour

art: placeholder-art ## Regenerate placeholder PNG art assets (alias for placeholder-art)

placeholder-art: ## Generate placeholder PNG sprites and portraits into assets/textures/
	$(PYTHON) ops/generate_placeholder_art.py

clean: ## Remove Cargo build artifacts
	$(CARGO) clean

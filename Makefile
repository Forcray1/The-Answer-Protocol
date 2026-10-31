# ============================================================================
#  TAP - Makefile (Cargo workspace wrapper)
#
#  Usage:   make help
#  Run all targets from the repository root: the server reads ./world.yaml and
#  creates ./game.db and ./logs.json in the current directory.
# ============================================================================

CARGO       ?= cargo
# Release builds are strongly recommended for the Bevy GUI (debug is very slow).
# Use `make run-client-gui PROFILE=debug` for faster compilation while hacking.
PROFILE     ?= release
ifeq ($(PROFILE),release)
CARGO_FLAGS := --release
else
CARGO_FLAGS :=
endif

# Packages (see the [workspace] members in Cargo.toml)
SERVER_PKG  := serveur
CLI_PKG     := client
GUI_PKG     := client_bevy

# Structured JSON logging is enabled with the server's --logs flag.
# Disable with: make run-server LOGS=0
LOGS        ?= 1
ifeq ($(LOGS),1)
SERVER_ARGS := --logs
else
SERVER_ARGS :=
endif

# Files generated at runtime by the server
RUNTIME_FILES := game.db game.db-shm game.db-wal logs.json

.DEFAULT_GOAL := help

.PHONY: help all install install-deps-linux check-toolchain build build-server \
	build-client build-gui run-server run-client run-client-gui lint fmt \
	fmt-check clippy test check clean fclean re

help: ## Show this help
	@echo "TAP - available targets:"
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}'
	@echo ""
	@echo "Variables: PROFILE=release|debug (default release)  LOGS=1|0 (default 1)"

all: build ## Build the whole workspace

# ----------------------------------------------------------------------------
#  Dependencies
# ----------------------------------------------------------------------------
check-toolchain: ## Verify that cargo/rustc are installed (Rust >= 1.85 required)
	@command -v $(CARGO) >/dev/null 2>&1 || { \
		echo "error: cargo not found. Install Rust with https://rustup.rs"; exit 1; }
	@echo "Using $$(rustc --version) / $$($(CARGO) --version)"
	@echo "note: client_bevy uses edition 2024, which needs Rust >= 1.85"

install: check-toolchain ## Fetch all Rust dependencies (crates)
	$(CARGO) fetch --locked 2>/dev/null || $(CARGO) fetch
	@echo "Dependencies fetched. On Linux, GUI system libraries may be needed: make install-deps-linux"

install-deps-linux: ## (Debian/Ubuntu) Install system libraries needed by Bevy
	sudo apt-get update
	sudo apt-get install -y pkg-config build-essential libasound2-dev libudev-dev \
		libwayland-dev libxkbcommon-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev

# ----------------------------------------------------------------------------
#  Build
# ----------------------------------------------------------------------------
build: check-toolchain ## Build server, CLI client and GUI client
	$(CARGO) build --workspace $(CARGO_FLAGS)

build-server: check-toolchain ## Build the server only
	$(CARGO) build -p $(SERVER_PKG) $(CARGO_FLAGS)

build-client: check-toolchain ## Build the CLI client only
	$(CARGO) build -p $(CLI_PKG) $(CARGO_FLAGS)

build-gui: check-toolchain ## Build the GUI client only
	$(CARGO) build -p $(GUI_PKG) $(CARGO_FLAGS)

# ----------------------------------------------------------------------------
#  Run
# ----------------------------------------------------------------------------
run-server: check-toolchain ## Run the server (listens on 127.0.0.1:4243)
	$(CARGO) run -p $(SERVER_PKG) $(CARGO_FLAGS) -- $(SERVER_ARGS)

run-client: check-toolchain ## Run the CLI client (connects to 127.0.0.1:4243)
	$(CARGO) run -p $(CLI_PKG) $(CARGO_FLAGS)

run-client-gui: check-toolchain ## Run the Bevy GUI client (connects to 127.0.0.1:4243)
	@test -d $(GUI_PKG)/assets || echo "warning: $(GUI_PKG)/assets not found - sprites will be missing"
	$(CARGO) run -p $(GUI_PKG) $(CARGO_FLAGS)

# ----------------------------------------------------------------------------
#  Quality
# ----------------------------------------------------------------------------
fmt: ## Format all code with rustfmt
	$(CARGO) fmt --all

fmt-check: ## Check formatting without modifying files
	$(CARGO) fmt --all -- --check

clippy: ## Run clippy on the whole workspace, warnings are errors
	$(CARGO) clippy --workspace --all-targets -- -D warnings

lint: fmt-check clippy ## Lint: rustfmt check + clippy (run `make fmt` first if it fails)

test: check-toolchain ## Run the unit tests of the workspace
	$(CARGO) test --workspace

check: check-toolchain ## Fast type-check of the workspace (no binaries)
	$(CARGO) check --workspace --all-targets

# ----------------------------------------------------------------------------
#  Clean
# ----------------------------------------------------------------------------
clean: ## Remove build artifacts (target/)
	$(CARGO) clean

fclean: clean ## clean + delete the database and logs (ALL ACCOUNTS ARE LOST)
	rm -f $(RUNTIME_FILES)

re: fclean all ## Full rebuild from scratch
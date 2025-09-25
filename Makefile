# Oversee Release Testing Makefile
# This Makefile provides convenient targets for testing the release process locally

# Load environment variables if .env exists
-include scripts/.env
export

# Colors for output
RED := \033[0;31m
GREEN := \033[0;32m
YELLOW := \033[0;33m
BLUE := \033[0;34m
NC := \033[0m # No Color

# Default target - show help
.PHONY: help
help:
	@echo "$(BLUE)Oversee Release Testing Commands$(NC)"
	@echo ""
	@echo "$(GREEN)Setup:$(NC)"
	@echo "  make setup           - Initial setup (copy .env.example to .env)"
	@echo ""
	@echo "$(GREEN)Testing (dry-run):$(NC)"
	@echo "  make test-crates     - Test crates.io publishing (dry-run)"
	@echo "  make test-homebrew   - Test Homebrew formula generation"
	@echo "  make test-release    - Test full release process locally"
	@echo ""
	@echo "$(GREEN)Building:$(NC)"
	@echo "  make build-release   - Build release binary for Apple Silicon"
	@echo "  make build-apple     - Build for Apple Silicon"
	@echo ""
	@echo "$(GREEN)Publishing (actual):$(NC)"
	@echo "  make publish-crates  - Actually publish to crates.io"
	@echo "  make publish-homebrew - Actually update Homebrew tap"
	@echo ""
	@echo "$(GREEN)Debugging:$(NC)"
	@echo "  make debug-crates    - Debug crates.io publishing issues"
	@echo "  make debug-homebrew  - Debug Homebrew formula issues"
	@echo ""
	@echo "$(GREEN)Utilities:$(NC)"
	@echo "  make clean           - Clean build artifacts"
	@echo "  make check-env       - Verify environment variables are set"
	@echo ""

# Setup - copy example env file
.PHONY: setup
setup:
	@if [ ! -f scripts/.env ]; then \
		echo "$(BLUE)Creating .env file from example...$(NC)"; \
		cp scripts/.env.example scripts/.env; \
		echo "$(GREEN)✓ Created scripts/.env$(NC)"; \
		echo "$(YELLOW)⚠ Please edit scripts/.env and add your tokens$(NC)"; \
	else \
		echo "$(YELLOW).env file already exists$(NC)"; \
	fi

# Check environment variables
.PHONY: check-env
check-env:
	@echo "$(BLUE)Checking environment variables...$(NC)"
	@if [ -z "$(CRATES_IO_TOKEN)" ]; then \
		echo "$(RED)✗ CRATES_IO_TOKEN not set$(NC)"; \
		exit 1; \
	else \
		echo "$(GREEN)✓ CRATES_IO_TOKEN is set$(NC)"; \
	fi
	@if [ -z "$(HOMEBREW_TAP_TOKEN)" ]; then \
		echo "$(RED)✗ HOMEBREW_TAP_TOKEN not set$(NC)"; \
		exit 1; \
	else \
		echo "$(GREEN)✓ HOMEBREW_TAP_TOKEN is set$(NC)"; \
	fi
	@echo "$(GREEN)All required environment variables are set!$(NC)"

# Test crates.io publishing (dry-run)
.PHONY: test-crates
test-crates:
	@echo "$(BLUE)Testing crates.io publishing (dry-run)...$(NC)"
	@./scripts/publish_crates.sh --dry-run

# Test Homebrew formula generation
.PHONY: test-homebrew
test-homebrew:
	@echo "$(BLUE)Testing Homebrew formula generation...$(NC)"
	@./scripts/publish_homebrew.sh --dry-run

# Test full release process
.PHONY: test-release
test-release:
	@echo "$(BLUE)Testing full release process...$(NC)"
	@./scripts/test_release.sh

# Build release binaries
.PHONY: build-release
build-release: build-apple
	@echo "$(GREEN)✓ Built release binary$(NC)"

# Build for Apple Silicon
.PHONY: build-apple
build-apple:
	@echo "$(BLUE)Building for Apple Silicon (aarch64-apple-darwin)...$(NC)"
	cargo build --release --target aarch64-apple-darwin
	@echo "$(GREEN)✓ Built Apple Silicon binary$(NC)"


# Actually publish to crates.io
.PHONY: publish-crates
publish-crates: check-env
	@echo "$(YELLOW)⚠ This will ACTUALLY publish to crates.io!$(NC)"
	@read -p "Are you sure? (y/N) " -n 1 -r; \
	echo; \
	if [[ $$REPLY =~ ^[Yy]$$ ]]; then \
		./scripts/publish_crates.sh; \
	else \
		echo "$(YELLOW)Cancelled$(NC)"; \
	fi

# Actually update Homebrew tap
.PHONY: publish-homebrew
publish-homebrew: check-env
	@echo "$(YELLOW)⚠ This will ACTUALLY update the Homebrew tap!$(NC)"
	@read -p "Are you sure? (y/N) " -n 1 -r; \
	echo; \
	if [[ $$REPLY =~ ^[Yy]$$ ]]; then \
		./scripts/publish_homebrew.sh; \
	else \
		echo "$(YELLOW)Cancelled$(NC)"; \
	fi

# Clean build artifacts
.PHONY: clean
clean:
	@echo "$(BLUE)Cleaning build artifacts...$(NC)"
	cargo clean
	@echo "$(GREEN)✓ Cleaned$(NC)"

# Create release archives (for testing Homebrew formula)
.PHONY: create-archives
create-archives: build-release
	@echo "$(BLUE)Creating release archives...$(NC)"
	@mkdir -p target/release-archives
	@version=$$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2); \
	cd target/aarch64-apple-darwin/release && \
	tar czf ../../release-archives/oversee-$$version-aarch64-apple-darwin.tar.gz oversee && \
	echo "$(GREEN)✓ Created Apple Silicon archive$(NC)"

# Debug targets
.PHONY: debug-crates
debug-crates:
	@echo "$(BLUE)Running crates.io publishing with debug output...$(NC)"
	@./scripts/publish_crates.sh --dry-run --debug

.PHONY: debug-homebrew
debug-homebrew:
	@echo "$(BLUE)Running Homebrew formula generation with debug output...$(NC)"
	@./scripts/publish_homebrew.sh --dry-run --debug
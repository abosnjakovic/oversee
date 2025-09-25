#!/bin/bash

# Publish to crates.io - Test and optionally publish the package
set -e

# Parse command line arguments
DEBUG_MODE=false
DRY_RUN_FLAG=""

for arg in "$@"; do
    case $arg in
        --dry-run)
            DRY_RUN_FLAG="--dry-run"
            ;;
        --debug)
            DEBUG_MODE=true
            ;;
        --help)
            echo "Usage: $0 [--dry-run] [--debug] [--help]"
            echo "  --dry-run   Test without actually publishing"
            echo "  --debug     Enable verbose debugging output"
            echo "  --help      Show this help message"
            exit 0
            ;;
    esac
done

# Enable debug mode if requested
if [ "$DEBUG_MODE" = true ]; then
    echo "🐛 Debug mode enabled"
    set -x
fi

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Load environment variables
if [ -f "scripts/.env" ]; then
    source scripts/.env
else
    echo -e "${RED}Error: scripts/.env not found. Run 'make setup' first.${NC}"
    exit 1
fi

# Check if dry-run (from argument or env)
if [[ "$DRY_RUN_FLAG" == "--dry-run" ]] || [[ "$DRY_RUN" == "true" ]]; then
    DRY_RUN_FLAG="--dry-run"
    echo -e "${BLUE}=== Crates.io Publishing Test (DRY RUN) ===${NC}"
else
    echo -e "${YELLOW}=== Crates.io Publishing (ACTUAL) ===${NC}"
fi
echo ""

# Check for token
if [ -z "$CRATES_IO_TOKEN" ]; then
    echo -e "${RED}Error: CRATES_IO_TOKEN not set in .env${NC}"
    echo "Get a token from: https://crates.io/settings/tokens"
    exit 1
fi

# Get current version from Cargo.toml
current_version=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)
package_name=$(grep '^name' Cargo.toml | head -1 | cut -d'"' -f2)

echo -e "${BLUE}Package Information:${NC}"
echo "  Name: ${package_name}"
echo "  Version: ${current_version}"
echo "  Registry: crates.io"
echo ""

# Step 1: Check if package exists on crates.io
echo -e "${BLUE}Step 1: Checking crates.io for existing package...${NC}"
if curl -s "https://crates.io/api/v1/crates/${package_name}" | grep -q "\"name\":\"${package_name}\""; then
    echo -e "${GREEN}✓ Package '${package_name}' exists on crates.io${NC}"

    # Check latest version
    latest_version=$(curl -s "https://crates.io/api/v1/crates/${package_name}" | grep -o '"max_version":"[^"]*' | cut -d'"' -f4)
    echo "  Latest published version: ${latest_version}"

    if [ "$latest_version" == "$current_version" ]; then
        echo -e "${YELLOW}⚠ Version ${current_version} is already published${NC}"
        if [ -z "$DRY_RUN_FLAG" ]; then
            echo -e "${RED}Cannot publish the same version twice${NC}"
            exit 1
        fi
    else
        echo -e "${GREEN}✓ Version ${current_version} is not yet published${NC}"
    fi
else
    echo -e "${BLUE}Package '${package_name}' not found on crates.io (will be created)${NC}"
fi
echo ""

# Step 2: Validate Cargo.toml
echo -e "${BLUE}Step 2: Validating Cargo.toml...${NC}"

# Check required fields
required_fields=("name" "version" "edition" "description" "license")
for field in "${required_fields[@]}"; do
    if grep -q "^${field}" Cargo.toml; then
        value=$(grep "^${field}" Cargo.toml | head -1 | cut -d'"' -f2 || grep "^${field}" Cargo.toml | head -1 | cut -d'=' -f2 | tr -d ' ')
        echo -e "${GREEN}✓ ${field}: ${value}${NC}"
    else
        echo -e "${RED}✗ Missing required field: ${field}${NC}"
        exit 1
    fi
done

# Check optional but recommended fields
optional_fields=("authors" "repository" "homepage" "keywords" "categories")
for field in "${optional_fields[@]}"; do
    if grep -q "^${field}" Cargo.toml; then
        echo -e "${GREEN}✓ ${field} is present${NC}"
    else
        echo -e "${YELLOW}⚠ Optional field '${field}' not found${NC}"
    fi
done
echo ""

# Step 3: Build test
echo -e "${BLUE}Step 3: Testing build...${NC}"
if cargo build --release 2>/dev/null; then
    echo -e "${GREEN}✓ Release build successful${NC}"
else
    echo -e "${RED}✗ Build failed${NC}"
    exit 1
fi
echo ""

# Step 4: Run tests
echo -e "${BLUE}Step 4: Running tests...${NC}"
if cargo test --quiet 2>/dev/null; then
    echo -e "${GREEN}✓ All tests passed${NC}"
else
    echo -e "${YELLOW}⚠ Some tests failed (continuing anyway)${NC}"
fi
echo ""

# Step 5: Check package size (non-fatal)
echo -e "${BLUE}Step 5: Checking package size...${NC}"

# Temporarily disable exit-on-error for this section
set +e

# Check package file list
if cargo package --list > /tmp/cargo-package-list.txt 2>/dev/null; then
    file_count=$(wc -l < /tmp/cargo-package-list.txt)
    echo "  Files to be included: ${file_count}"

    if [ "$file_count" -gt 100 ]; then
        echo -e "${YELLOW}⚠ Large number of files. Consider adding more entries to .gitignore${NC}"
    fi
else
    echo -e "${YELLOW}⚠ Could not get package file list${NC}"
    file_count=0
fi

# Estimate package size (non-fatal)
package_output=$(cargo package --allow-dirty 2>&1)
package_exit_code=$?

if [ $package_exit_code -eq 0 ]; then
    size_info=$(echo "$package_output" | grep "Packaged" | tail -1)
    if [ -n "$size_info" ]; then
        echo "  ${size_info}"
    else
        echo "  Package created successfully"
    fi
else
    echo -e "${YELLOW}⚠ Package size check failed (this is non-fatal)${NC}"
    echo "  Error: $(echo "$package_output" | head -3 | tail -1)"
    echo "  Continuing with publish process..."
fi

# Re-enable exit-on-error
set -e
echo ""

# Step 6: Publish or dry-run
if [ -n "$DRY_RUN_FLAG" ]; then
    echo -e "${BLUE}Step 6: Running publish dry-run...${NC}"

    if cargo publish --dry-run --allow-dirty --token "${CRATES_IO_TOKEN}" 2>&1 | tee /tmp/publish-output.txt; then
        echo ""
        echo -e "${GREEN}✓ Dry-run successful!${NC}"
        echo -e "${BLUE}The package is ready to publish.${NC}"
        echo ""
        echo "To actually publish, run:"
        echo "  make publish-crates"
        echo "Or:"
        echo "  ./scripts/publish_crates.sh"
    else
        echo -e "${RED}✗ Dry-run failed${NC}"
        echo "Check the errors above and fix them before publishing."
        exit 1
    fi
else
    echo -e "${YELLOW}Step 6: Publishing to crates.io...${NC}"
    echo -e "${YELLOW}This action cannot be undone!${NC}"
    read -p "Are you sure you want to publish ${package_name} v${current_version}? (y/N) " -n 1 -r
    echo

    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo -e "${BLUE}Publishing...${NC}"

        if cargo publish --allow-dirty --token "${CRATES_IO_TOKEN}"; then
            echo ""
            echo -e "${GREEN}✓ Successfully published ${package_name} v${current_version} to crates.io!${NC}"
            echo ""
            echo "View your package at:"
            echo "  https://crates.io/crates/${package_name}"
            echo ""
            echo "Users can now install with:"
            echo "  cargo install ${package_name}"
        else
            echo -e "${RED}✗ Publishing failed${NC}"
            echo "Check the errors above. Common issues:"
            echo "  - Version already exists"
            echo "  - Invalid token"
            echo "  - Network issues"
            exit 1
        fi
    else
        echo -e "${YELLOW}Publishing cancelled${NC}"
        exit 0
    fi
fi
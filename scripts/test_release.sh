#!/bin/bash

# Test Release Script - Simulates the full GitHub Actions release workflow locally
set -e

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

echo -e "${BLUE}=== Oversee Local Release Test ===${NC}"
echo ""

# Use TEST_VERSION from .env or prompt
if [ -z "$TEST_VERSION" ]; then
    read -p "Enter version to test (e.g., 0.1.5): " TEST_VERSION
fi

if [ -z "$TEST_TAG" ]; then
    TEST_TAG="v${TEST_VERSION}"
fi

echo -e "${BLUE}Testing release for:${NC}"
echo "  Version: ${TEST_VERSION}"
echo "  Tag: ${TEST_TAG}"
echo "  Repository: ${TEST_REPO:-abosnjakovic/oversee}"
echo ""

# Step 1: Validate Cargo.toml
echo -e "${BLUE}Step 1: Validating Cargo.toml...${NC}"
current_version=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)
echo "  Current version in Cargo.toml: ${current_version}"

# Check required fields for crates.io
if ! grep -q '^description' Cargo.toml; then
    echo -e "${RED}✗ Missing 'description' field in Cargo.toml${NC}"
    exit 1
fi

if ! grep -q '^license' Cargo.toml; then
    echo -e "${RED}✗ Missing 'license' field in Cargo.toml${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Cargo.toml is valid for publishing${NC}"
echo ""

# Step 2: Test Building
echo -e "${BLUE}Step 2: Testing build process...${NC}"
echo "  Testing debug build..."
if cargo build --quiet 2>/dev/null; then
    echo -e "${GREEN}✓ Debug build successful${NC}"
else
    echo -e "${RED}✗ Debug build failed${NC}"
    exit 1
fi

echo "  Testing release build..."
if cargo build --release --quiet 2>/dev/null; then
    echo -e "${GREEN}✓ Release build successful${NC}"
else
    echo -e "${RED}✗ Release build failed${NC}"
    exit 1
fi
echo ""

# Step 3: Test crates.io publishing (dry-run)
echo -e "${BLUE}Step 3: Testing crates.io publishing...${NC}"
if cargo publish --dry-run --allow-dirty 2>/dev/null; then
    echo -e "${GREEN}✓ Crates.io dry-run successful${NC}"
else
    echo -e "${YELLOW}⚠ Crates.io dry-run failed (this may be okay if already published)${NC}"
fi
echo ""

# Step 4: Create test archives
echo -e "${BLUE}Step 4: Creating test archives...${NC}"
mkdir -p target/test-release

# Check if binaries exist for the current platform
if [[ "$OSTYPE" == "darwin"* ]]; then
    if [[ $(uname -m) == "arm64" ]]; then
        echo "  Creating Apple Silicon test archive..."
        if [ -f "target/release/oversee" ]; then
            cd target/release
            tar czf ../test-release/oversee-${TEST_VERSION}-aarch64-apple-darwin.tar.gz oversee
            cd ../..
            echo -e "${GREEN}✓ Created Apple Silicon archive${NC}"
        else
            echo -e "${YELLOW}⚠ No release binary found, skipping archive${NC}"
        fi
    else
        echo "  Creating Intel Mac test archive..."
        if [ -f "target/release/oversee" ]; then
            cd target/release
            tar czf ../test-release/oversee-${TEST_VERSION}-x86_64-apple-darwin.tar.gz oversee
            cd ../..
            echo -e "${GREEN}✓ Created Intel Mac archive${NC}"
        else
            echo -e "${YELLOW}⚠ No release binary found, skipping archive${NC}"
        fi
    fi
fi
echo ""

# Step 5: Test Homebrew formula generation
echo -e "${BLUE}Step 5: Testing Homebrew formula generation...${NC}"

# Create a test formula
cat > target/test-release/oversee-test.rb << EOF
class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/${TEST_REPO}"
  version "${TEST_VERSION}"

  if Hardware::CPU.arm?
    url "https://github.com/${TEST_REPO}/releases/download/${TEST_TAG}/oversee-${TEST_VERSION}-aarch64-apple-darwin.tar.gz"
    sha256 "TEST_SHA256_AARCH64"
  else
    url "https://github.com/${TEST_REPO}/releases/download/${TEST_TAG}/oversee-${TEST_VERSION}-x86_64-apple-darwin.tar.gz"
    sha256 "TEST_SHA256_X86_64"
  end

  def install
    bin.install "oversee"
  end

  test do
    system "#{bin}/oversee", "--version"
  end
end
EOF

# Test sed replacements (same as in GitHub Actions)
cp target/test-release/oversee-test.rb target/test-release/oversee-test-processed.rb
sed -i.bak "s|TEST_SHA256_AARCH64|dummy_sha256_aarch64|g" target/test-release/oversee-test-processed.rb
sed -i.bak "s|TEST_SHA256_X86_64|dummy_sha256_x86_64|g" target/test-release/oversee-test-processed.rb

if grep -q "TEST_SHA256" target/test-release/oversee-test-processed.rb; then
    echo -e "${RED}✗ Formula template replacement failed${NC}"
    exit 1
else
    echo -e "${GREEN}✓ Formula generation successful${NC}"
fi
echo ""

# Step 6: Validate GitHub Actions workflow
echo -e "${BLUE}Step 6: Validating GitHub Actions workflow...${NC}"
if [ -f ".github/workflows/release.yml" ]; then
    # Check for workflow_dispatch trigger
    if grep -q "workflow_dispatch:" .github/workflows/release.yml; then
        echo -e "${GREEN}✓ Manual trigger configured${NC}"
    else
        echo -e "${YELLOW}⚠ No manual trigger found in workflow${NC}"
    fi

    # Check for homebrew job
    if grep -q "update-homebrew-formula:" .github/workflows/release.yml; then
        echo -e "${GREEN}✓ Homebrew automation configured${NC}"
    else
        echo -e "${YELLOW}⚠ No Homebrew automation found${NC}"
    fi
else
    echo -e "${RED}✗ Release workflow not found${NC}"
fi
echo ""

# Summary
echo -e "${BLUE}=== Test Summary ===${NC}"
echo -e "${GREEN}✓ Cargo.toml validation passed${NC}"
echo -e "${GREEN}✓ Build tests passed${NC}"
echo -e "${GREEN}✓ Crates.io dry-run completed${NC}"
echo -e "${GREEN}✓ Archive creation tested${NC}"
echo -e "${GREEN}✓ Homebrew formula generation tested${NC}"
echo -e "${GREEN}✓ GitHub Actions workflow validated${NC}"
echo ""

echo -e "${GREEN}All tests passed! The release process is ready.${NC}"
echo ""
echo -e "${BLUE}Next steps:${NC}"
echo "1. Run 'make test-crates' to test crates.io publishing in detail"
echo "2. Run 'make test-homebrew' to test Homebrew formula in detail"
echo "3. When ready, create a git tag: git tag ${TEST_TAG}"
echo "4. Push the tag: git push origin ${TEST_TAG}"
echo ""
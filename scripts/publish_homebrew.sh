#!/bin/bash

# Publish to Homebrew - Generate and update Homebrew formula
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

# Check if dry-run
DRY_RUN_MODE=false
if [[ "$1" == "--dry-run" ]] || [[ "$DRY_RUN" == "true" ]]; then
    DRY_RUN_MODE=true
    echo -e "${BLUE}=== Homebrew Formula Generation (DRY RUN) ===${NC}"
else
    echo -e "${YELLOW}=== Homebrew Formula Update (ACTUAL) ===${NC}"
fi
echo ""

# Check for token
if [ -z "$HOMEBREW_TAP_TOKEN" ] && [ "$DRY_RUN_MODE" == "false" ]; then
    echo -e "${RED}Error: HOMEBREW_TAP_TOKEN not set in .env${NC}"
    echo "Get a token from: https://github.com/settings/tokens"
    echo "(Token needs 'repo' permissions)"
    exit 1
fi

# Get version and package info
VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)
PACKAGE_NAME=$(grep '^name' Cargo.toml | head -1 | cut -d'"' -f2)
TAG="v${VERSION}"
REPO="${TEST_REPO:-abosnjakovic/oversee}"
REPO_OWNER=$(echo "$REPO" | cut -d'/' -f1)
REPO_NAME=$(echo "$REPO" | cut -d'/' -f2)

echo -e "${BLUE}Package Information:${NC}"
echo "  Name: ${PACKAGE_NAME}"
echo "  Version: ${VERSION}"
echo "  Tag: ${TAG}"
echo "  Repository: ${REPO}"
echo ""

# Step 1: Check for release archive (Apple Silicon only)
echo -e "${BLUE}Step 1: Checking for Apple Silicon release archive...${NC}"

AARCH64_ARCHIVE=""

# Check in multiple locations for Apple Silicon archive
ARCHIVE_LOCATIONS=(
    "target/release-archives"
    "target/test-release"
    "target/aarch64-apple-darwin/release"
    "."
)

for location in "${ARCHIVE_LOCATIONS[@]}"; do
    if [ -f "${location}/oversee-${VERSION}-aarch64-apple-darwin.tar.gz" ]; then
        AARCH64_ARCHIVE="${location}/oversee-${VERSION}-aarch64-apple-darwin.tar.gz"
        echo -e "${GREEN}✓ Found Apple Silicon archive: ${AARCH64_ARCHIVE}${NC}"
        break
    fi
done

# If not found, check if we can create it from binary
if [ -z "$AARCH64_ARCHIVE" ]; then
    echo -e "${YELLOW}Archive not found. Checking for binary...${NC}"

    mkdir -p target/release-archives

    # Try to create Apple Silicon archive
    if [ -f "target/aarch64-apple-darwin/release/oversee" ]; then
        echo "  Creating Apple Silicon archive..."
        cd target/aarch64-apple-darwin/release
        tar czf ../../release-archives/oversee-${VERSION}-aarch64-apple-darwin.tar.gz oversee
        cd ../../..
        AARCH64_ARCHIVE="target/release-archives/oversee-${VERSION}-aarch64-apple-darwin.tar.gz"
        echo -e "${GREEN}✓ Created Apple Silicon archive${NC}"
    fi
fi

# For dry-run, we can proceed without archive
if [ -z "$AARCH64_ARCHIVE" ] && [ "$DRY_RUN_MODE" == "false" ]; then
    echo -e "${RED}No Apple Silicon archive found. Please build first:${NC}"
    echo "  make build-apple"
    echo "  make create-archives"
    exit 1
fi
echo ""

# Step 2: Calculate SHA256 hash
echo -e "${BLUE}Step 2: Calculating SHA256 hash...${NC}"

if [ -n "$AARCH64_ARCHIVE" ] && [ -f "$AARCH64_ARCHIVE" ]; then
    AARCH64_SHA256=$(shasum -a 256 "$AARCH64_ARCHIVE" | cut -d' ' -f1)
    echo "  Apple Silicon: ${AARCH64_SHA256}"
else
    AARCH64_SHA256="PLACEHOLDER_SHA256_AARCH64"
    echo -e "${YELLOW}  Apple Silicon: Using placeholder (archive not found)${NC}"
fi
echo ""

# Step 3: Generate Homebrew formula
echo -e "${BLUE}Step 3: Generating Homebrew formula...${NC}"

mkdir -p target/homebrew

cat > target/homebrew/oversee.rb << EOF
class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/${REPO}"
  version "${VERSION}"

  # Apple Silicon only
  depends_on arch: :arm64
  url "https://github.com/${REPO}/releases/download/${TAG}/oversee-${VERSION}-aarch64-apple-darwin.tar.gz"
  sha256 "${AARCH64_SHA256}"

  def install
    bin.install "oversee"
  end

  test do
    system "#{bin}/oversee", "--version"
  end
end
EOF

echo -e "${GREEN}✓ Formula generated at target/homebrew/oversee.rb${NC}"
cat target/homebrew/oversee.rb
echo ""

# Step 4: Update Homebrew tap (if not dry-run)
if [ "$DRY_RUN_MODE" == "true" ]; then
    echo -e "${BLUE}Step 4: Homebrew tap update (SKIPPED - dry run)${NC}"
    echo ""
    echo -e "${GREEN}✓ Dry-run successful!${NC}"
    echo ""
    echo "The formula has been generated at: target/homebrew/oversee.rb"
    echo ""
    echo "To publish to Homebrew tap, run:"
    echo "  make publish-homebrew"
    echo "Or:"
    echo "  ./scripts/publish_homebrew.sh"
else
    echo -e "${BLUE}Step 4: Updating Homebrew tap...${NC}"

    # Configure git
    git config --global user.name "github-actions[bot]"
    git config --global user.email "41898282+github-actions[bot]@users.noreply.github.com"

    # Clone the main repository
    TAP_REPO="oversee"
    TAP_URL="https://x-access-token:${HOMEBREW_TAP_TOKEN}@github.com/${REPO_OWNER}/${TAP_REPO}.git"
    TAP_DIR="target/main-repo"

    rm -rf "$TAP_DIR"

    # Clone the main repository
    echo "  Cloning main repository..."
    git clone "${TAP_URL}" "$TAP_DIR"

    # Ensure Formula directory exists
    mkdir -p "$TAP_DIR/Formula"

    # Copy formula and commit
    cp target/homebrew/oversee.rb "$TAP_DIR/Formula/oversee.rb"
    cd "$TAP_DIR"

    git add Formula/oversee.rb
    if git diff --staged --quiet; then
        echo -e "${YELLOW}No changes to commit (formula unchanged)${NC}"
    else
        git commit -m "Update ${PACKAGE_NAME} to ${VERSION}"

        echo -e "${YELLOW}Ready to push. This will update the public tap!${NC}"
        read -p "Push changes to ${REPO_OWNER}/${TAP_REPO}? (y/N) " -n 1 -r
        echo

        if [[ $REPLY =~ ^[Yy]$ ]]; then
            if git push origin main 2>/dev/null || git push origin master 2>/dev/null; then
                echo -e "${GREEN}✓ Successfully updated Homebrew tap!${NC}"
                echo ""
                echo "Users can now install with:"
                echo "  brew tap ${REPO_OWNER}/${PACKAGE_NAME} https://github.com/${REPO_OWNER}/${PACKAGE_NAME}"
                echo "  brew install ${PACKAGE_NAME}"
            else
                echo -e "${RED}✗ Failed to push to tap repository${NC}"
                echo "You may need to create the repository first at:"
                echo "  https://github.com/new"
                echo "Repository name: ${TAP_REPO}"
                exit 1
            fi
        else
            echo -e "${YELLOW}Push cancelled. Changes are committed locally in:${NC}"
            echo "  ${TAP_DIR}"
            echo ""
            echo "To install locally for testing:"
            echo "  brew install ${TAP_DIR}/Formula/oversee.rb"
            echo ""
            echo "Or install via tap:"
            echo "  brew tap ${REPO_OWNER}/${TAP_REPO} https://github.com/${REPO_OWNER}/${TAP_REPO}"
            echo "  brew install ${PACKAGE_NAME}"
        fi
    fi

    cd ../..
fi

echo ""
echo -e "${BLUE}=== Summary ===${NC}"
if [ "$DRY_RUN_MODE" == "true" ]; then
    echo "Formula generated successfully (dry-run mode)"
    echo "Location: target/homebrew/oversee.rb"
else
    echo "Formula updated in main repository"
    echo "Repository: ${REPO_OWNER}/${TAP_REPO}"
    echo "Install with:"
    echo "  brew tap ${REPO_OWNER}/${TAP_REPO} https://github.com/${REPO_OWNER}/${TAP_REPO}"
    echo "  brew install ${PACKAGE_NAME}"
fi
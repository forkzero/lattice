#!/bin/sh
# Lattice installer
# Usage: curl -fsSL https://forkzero.ai/lattice/install.sh | sh
#
# Environment variables:
#   INSTALL_DIR  Override install location (default: ~/.local/bin)
#   VERSION      Install a specific version (default: latest)

set -e

REPO="forkzero/lattice"

# Default to ~/.local/bin (user-writable, no sudo needed)
if [ -z "$INSTALL_DIR" ]; then
    if [ -w "/usr/local/bin" ]; then
        INSTALL_DIR="/usr/local/bin"
    else
        INSTALL_DIR="${HOME}/.local/bin"
    fi
fi

# Detect OS
OS="$(uname -s)"
case "$OS" in
    Linux)  OS="unknown-linux-gnu" ;;
    Darwin) OS="apple-darwin" ;;
    *)      echo "Unsupported OS: $OS"; exit 1 ;;
esac

# Detect architecture
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64|amd64)   ARCH="x86_64" ;;
    arm64|aarch64)  ARCH="aarch64" ;;
    *)              echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

TARGET="${ARCH}-${OS}"

# Get latest version
if [ -z "$VERSION" ]; then
    VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')
fi

if [ -z "$VERSION" ]; then
    echo "Failed to detect latest version"
    exit 1
fi

ARCHIVE="lattice-${VERSION}-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/v${VERSION}/${ARCHIVE}"
CHECKSUM_URL="https://github.com/${REPO}/releases/download/v${VERSION}/checksums.txt"

echo "Installing lattice v${VERSION} for ${TARGET}..."

# Create temp directory
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

# Download archive
echo "Downloading ${URL}..."
curl -fsSL "$URL" -o "${TMP_DIR}/${ARCHIVE}"

# Download and verify checksum
echo "Verifying checksum..."
curl -fsSL "$CHECKSUM_URL" -o "${TMP_DIR}/checksums.txt"
cd "$TMP_DIR"
if command -v sha256sum > /dev/null 2>&1; then
    grep "$ARCHIVE" checksums.txt | sha256sum -c - > /dev/null 2>&1
elif command -v shasum > /dev/null 2>&1; then
    grep "$ARCHIVE" checksums.txt | shasum -a 256 -c - > /dev/null 2>&1
else
    echo "Warning: Could not verify checksum (sha256sum/shasum not found)"
fi

# Extract
echo "Extracting..."
tar -xzf "$ARCHIVE"

# Ensure install directory exists
mkdir -p "$INSTALL_DIR"

# Install
BINARY_DIR="lattice-${VERSION}-${TARGET}"
if [ -w "$INSTALL_DIR" ]; then
    mv "${BINARY_DIR}/lattice" "$INSTALL_DIR/"
else
    echo "Installing to ${INSTALL_DIR} (requires sudo)..."
    sudo mv "${BINARY_DIR}/lattice" "$INSTALL_DIR/"
fi

# Check if install dir is in PATH
case ":$PATH:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
        echo ""
        echo "Add ${INSTALL_DIR} to your PATH:"
        SHELL_NAME="$(basename "$SHELL")"
        case "$SHELL_NAME" in
            zsh)  echo "  echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ~/.zshrc && source ~/.zshrc" ;;
            bash) echo "  echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ~/.bashrc && source ~/.bashrc" ;;
            fish) echo "  fish_add_path ${INSTALL_DIR}" ;;
            *)    echo "  export PATH=\"${INSTALL_DIR}:\$PATH\"" ;;
        esac
        echo ""
        ;;
esac

# Verify
if command -v lattice > /dev/null 2>&1; then
    echo ""
    echo "Successfully installed lattice $(lattice --version)"
    echo ""
    echo "Get started:"
    echo "  lattice init          # Initialize a lattice"
    echo "  lattice --help        # Show all commands"
    echo "  lattice help concepts # Learn the domain model"
else
    echo ""
    echo "Installed to ${INSTALL_DIR}/lattice"
    echo "Make sure ${INSTALL_DIR} is in your PATH."
fi

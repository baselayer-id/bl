#!/bin/bash
# Baselayer CLI (bl) installer
# Usage: curl -fsSL https://raw.githubusercontent.com/baselayer-id/bl/main/install.sh | bash
#
# Installs the `bl` binary to ~/.local/bin (or $BL_INSTALL_DIR if set).
# Detects macOS architecture automatically.

set -euo pipefail

REPO="baselayer-id/bl"
BINARY_NAME="bl"
INSTALL_DIR="${BL_INSTALL_DIR:-$HOME/.local/bin}"

# ── Colors ──
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

info()  { echo -e "${CYAN}→${NC} $*"; }
ok()    { echo -e "${GREEN}✓${NC} $*"; }
warn()  { echo -e "${YELLOW}!${NC} $*"; }
fail()  { echo -e "${RED}✗${NC} $*" >&2; exit 1; }

# ── Platform detection ──

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin) ;;
  Linux)  fail "Linux is not yet supported. Coming soon." ;;
  *)      fail "Unsupported OS: $OS. Only macOS is currently supported." ;;
esac

case "$ARCH" in
  arm64|aarch64) TARGET="aarch64-apple-darwin" ;;
  x86_64)        TARGET="x86_64-apple-darwin" ;;
  *)             fail "Unsupported architecture: $ARCH" ;;
esac

info "Detected: macOS ${ARCH}"

# ── Find latest release ──

info "Finding latest release..."

LATEST_TAG=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep '"tag_name"' \
  | head -1 \
  | sed -E 's/.*"v([^"]+)".*/\1/' \
) || fail "Could not fetch releases from GitHub.\n\n  Check your internet connection and try again.\n  You can also download manually from:\n  https://github.com/${REPO}/releases"

if [ -z "$LATEST_TAG" ]; then
  fail "No releases found.\n\n  Build from source instead:\n    git clone https://github.com/${REPO}\n    cd bl && cargo build --release\n    cp target/release/bl ~/.local/bin/"
fi

VERSION="$LATEST_TAG"
info "Latest version: v${VERSION}"

# ── Download ──

TARBALL="bl-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/v${VERSION}/${TARBALL}"
CHECKSUM_URL="https://github.com/${REPO}/releases/download/v${VERSION}/checksums.txt"

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

info "Downloading ${TARBALL}..."
curl -fsSL -o "${TMPDIR}/${TARBALL}" "$DOWNLOAD_URL" \
  || fail "Download failed.\n\n  URL: ${DOWNLOAD_URL}\n  Check that the release exists at:\n  https://github.com/${REPO}/releases/tag/v${VERSION}"

# ── Verify checksum ──

info "Verifying checksum..."
curl -fsSL -o "${TMPDIR}/checksums.txt" "$CHECKSUM_URL" 2>/dev/null || true

if [ -f "${TMPDIR}/checksums.txt" ]; then
  EXPECTED=$(grep "${TARBALL}" "${TMPDIR}/checksums.txt" | awk '{print $1}')
  ACTUAL=$(shasum -a 256 "${TMPDIR}/${TARBALL}" | awk '{print $1}')
  if [ -n "$EXPECTED" ] && [ "$EXPECTED" != "$ACTUAL" ]; then
    fail "Checksum mismatch!\n\n  Expected: ${EXPECTED}\n  Got:      ${ACTUAL}\n\n  The download may be corrupted. Try again, or download manually from:\n  https://github.com/${REPO}/releases"
  fi
  ok "Checksum verified"
else
  warn "Checksums file not found — skipping verification"
fi

# ── Install ──

info "Installing to ${INSTALL_DIR}..."
mkdir -p "$INSTALL_DIR"

tar -xzf "${TMPDIR}/${TARBALL}" -C "${TMPDIR}"
chmod +x "${TMPDIR}/${BINARY_NAME}"
mv "${TMPDIR}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"

ok "Installed ${BINARY_NAME} v${VERSION} to ${INSTALL_DIR}/${BINARY_NAME}"

# ── PATH check ──

if ! echo "$PATH" | tr ':' '\n' | grep -q "^${INSTALL_DIR}$"; then
  echo ""
  warn "${INSTALL_DIR} is not in your PATH."
  echo ""
  echo "  Add it by appending this to your ~/.zshrc (or ~/.bashrc):"
  echo ""
  echo "    export PATH=\"${INSTALL_DIR}:\$PATH\""
  echo ""
  echo "  Then restart your shell or run: source ~/.zshrc"
fi

# ── Next steps ──

echo ""
echo -e "${GREEN}━━━ Baselayer CLI installed! ━━━${NC}"
echo ""
echo "  Get started:"
echo "    bl auth login       Sign in (opens browser)"
echo "    bl setup claude     Install Claude Code hooks"
echo "    bl ask \"question\"   Ask your knowledge vault"
echo ""
echo "  Docs: https://github.com/${REPO}"
echo ""

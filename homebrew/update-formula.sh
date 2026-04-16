#!/bin/bash
# Updates the Homebrew formula with new version and checksums.
# Run after a GitHub release is created.
#
# Usage: ./update-formula.sh 0.1.0
#
# This script:
# 1. Downloads the release tarballs
# 2. Computes SHA256 checksums
# 3. Updates the formula file
# 4. Optionally pushes to the tap repo

set -euo pipefail

VERSION="${1:?Usage: $0 <version>}"
REPO="baselayer-id/bl"
FORMULA="$(dirname "$0")/bl.rb"
BASE_URL="https://github.com/${REPO}/releases/download/v${VERSION}"

echo "Updating formula for bl v${VERSION}..."

# Download and hash
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

for arch in aarch64-apple-darwin x86_64-apple-darwin; do
  echo "  Downloading bl-${arch}.tar.gz..."
  curl -fsSL -o "${TMPDIR}/bl-${arch}.tar.gz" "${BASE_URL}/bl-${arch}.tar.gz"
done

SHA_ARM64=$(shasum -a 256 "${TMPDIR}/bl-aarch64-apple-darwin.tar.gz" | awk '{print $1}')
SHA_X86=$(shasum -a 256 "${TMPDIR}/bl-x86_64-apple-darwin.tar.gz" | awk '{print $1}')

echo "  ARM64 SHA256: ${SHA_ARM64}"
echo "  x86_64 SHA256: ${SHA_X86}"

# Update formula
sed -i '' \
  -e "s/version \".*\"/version \"${VERSION}\"/" \
  -e "s/PLACEHOLDER_ARM64_SHA256\|[0-9a-f]\{64\}\"\$/${SHA_ARM64}\"/" \
  "$FORMULA"

# The x86 hash is on a different line — match by context
python3 -c "
import re, sys

with open('${FORMULA}') as f:
    content = f.read()

# Replace version
content = re.sub(r'version \"[^\"]+\"', 'version \"${VERSION}\"', content)

# Replace ARM64 SHA (after on_arm block)
lines = content.split('\n')
in_arm = False
in_intel = False
result = []
for line in lines:
    if 'on_arm' in line:
        in_arm = True
        in_intel = False
    elif 'on_intel' in line:
        in_arm = False
        in_intel = True
    elif in_arm and 'sha256' in line:
        line = re.sub(r'sha256 \"[^\"]+\"', 'sha256 \"${SHA_ARM64}\"', line)
        in_arm = False
    elif in_intel and 'sha256' in line:
        line = re.sub(r'sha256 \"[^\"]+\"', 'sha256 \"${SHA_X86}\"', line)
        in_intel = False
    result.append(line)

with open('${FORMULA}', 'w') as f:
    f.write('\n'.join(result))
"

echo ""
echo "✓ Formula updated: ${FORMULA}"
echo ""
echo "  To publish, copy this file to the homebrew-tap repo:"
echo "    cp ${FORMULA} ~/src/homebrew-tap/Formula/bl.rb"
echo "    cd ~/src/homebrew-tap && git add -A && git commit -m 'bl ${VERSION}' && git push"

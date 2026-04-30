#!/usr/bin/env bash
# cnb CLI installer.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/cnb-cool/cnb/main/scripts/install.sh | bash
#   curl -fsSL .../install.sh | bash -s -- --version v0.4.0 --prefix ~/.local/bin
#
# Honors:
#   CNB_VERSION   — release tag to install (default: latest GitHub Release)
#   CNB_PREFIX    — install dir (default: $HOME/.local/bin or /usr/local/bin if writable)
#   CNB_REPO      — GitHub slug for the release source (default: cnb-cool/cnb)
#
# The script does not require root unless installing to a system path. It
# verifies the SHA-256 of the downloaded archive against the published .sha256
# sidecar file; failure aborts the install.

set -euo pipefail

REPO="${CNB_REPO:-cnb-cool/cnb}"
VERSION="${CNB_VERSION:-}"
PREFIX="${CNB_PREFIX:-}"
BIN="cnb"

# Parse --version / --prefix flags.
while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) VERSION="$2"; shift 2 ;;
    --prefix)  PREFIX="$2";  shift 2 ;;
    --repo)    REPO="$2";    shift 2 ;;
    -h|--help)
      sed -n '2,/^set -e/p' "$0" | sed 's/^# \{0,1\}//' | head -20
      exit 0
      ;;
    *) echo "unknown flag: $1" >&2; exit 2 ;;
  esac
done

# --- Detect target ---
detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os" in
    Linux*)
      case "$arch" in
        x86_64|amd64)         echo "x86_64-unknown-linux-gnu" ;;
        aarch64|arm64)        echo "aarch64-unknown-linux-gnu" ;;
        *) echo "unsupported linux arch: $arch" >&2; return 1 ;;
      esac ;;
    Darwin*)
      case "$arch" in
        x86_64)  echo "x86_64-apple-darwin" ;;
        arm64)   echo "aarch64-apple-darwin" ;;
        *) echo "unsupported macos arch: $arch" >&2; return 1 ;;
      esac ;;
    MINGW*|MSYS*|CYGWIN*)     echo "x86_64-pc-windows-msvc" ;;
    *) echo "unsupported OS: $os" >&2; return 1 ;;
  esac
}

TARGET="$(detect_target)"
EXT=".tar.gz"
[[ "$TARGET" == *windows* ]] && EXT=".zip"

# --- Resolve version ---
if [[ -z "$VERSION" ]]; then
  echo "→ Resolving latest release of $REPO ..."
  VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
             | grep -m1 '"tag_name"' | cut -d'"' -f4)"
  if [[ -z "$VERSION" ]]; then
    echo "could not determine latest version; pass --version" >&2
    exit 1
  fi
fi
echo "  version: $VERSION"
echo "  target:  $TARGET"

# --- Resolve prefix ---
if [[ -z "$PREFIX" ]]; then
  if [[ -w "/usr/local/bin" ]]; then
    PREFIX="/usr/local/bin"
  else
    PREFIX="$HOME/.local/bin"
  fi
fi
mkdir -p "$PREFIX"
echo "  prefix:  $PREFIX"

# --- Download + verify ---
ARCHIVE="${BIN}-${VERSION}-${TARGET}${EXT}"
SUM="${ARCHIVE}.sha256"
URL_BASE="https://github.com/${REPO}/releases/download/${VERSION}"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "→ Downloading $URL_BASE/$ARCHIVE"
curl -fsSL -o "$TMP/$ARCHIVE" "$URL_BASE/$ARCHIVE"
echo "→ Downloading $URL_BASE/$SUM"
curl -fsSL -o "$TMP/$SUM"     "$URL_BASE/$SUM"

echo "→ Verifying SHA-256"
( cd "$TMP" && shasum -a 256 -c "$SUM" )

# --- Extract + install ---
echo "→ Extracting"
( cd "$TMP" && \
  if [[ "$EXT" == ".tar.gz" ]]; then tar xzf "$ARCHIVE"; else unzip -q "$ARCHIVE"; fi )

INNER_DIR="$TMP/${BIN}-${VERSION}-${TARGET}"
EXE="$INNER_DIR/${BIN}"
[[ "$TARGET" == *windows* ]] && EXE="${EXE}.exe"

if [[ ! -f "$EXE" ]]; then
  echo "expected binary not found at $EXE" >&2
  exit 1
fi

install -m 0755 "$EXE" "$PREFIX/${BIN}$([[ "$TARGET" == *windows* ]] && echo .exe)"
echo "✓ Installed cnb $VERSION to $PREFIX"

# Hint about PATH.
if ! echo ":$PATH:" | grep -q ":$PREFIX:"; then
  echo
  echo "  Note: $PREFIX is not on your PATH. Add to your shell rc:"
  echo "    export PATH=\"$PREFIX:\$PATH\""
fi

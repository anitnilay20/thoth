#!/usr/bin/env bash
#
# Thoth installer / updater (macOS + Linux).
#
#   curl -fsSL https://raw.githubusercontent.com/anitnilay20/thoth/main/install.sh | bash
#
# Re-running installs the latest release, so the same command updates Thoth.
# Windows users: see install.ps1.
set -euo pipefail

REPO="anitnilay20/thoth"
APP_NAME="Thoth"
BIN_NAME="thoth"

# ── pretty logging ────────────────────────────────────────────────────────────
if [ -t 1 ]; then
  BOLD=$(printf '\033[1m'); DIM=$(printf '\033[2m'); RED=$(printf '\033[31m')
  GREEN=$(printf '\033[32m'); RESET=$(printf '\033[0m')
else
  BOLD=""; DIM=""; RED=""; GREEN=""; RESET=""
fi
info() { printf '%s==>%s %s\n' "$BOLD" "$RESET" "$1"; }
warn() { printf '%swarning:%s %s\n' "$RED" "$RESET" "$1" >&2; }
die()  { printf '%serror:%s %s\n' "$RED" "$RESET" "$1" >&2; exit 1; }

need() { command -v "$1" >/dev/null 2>&1 || die "'$1' is required but not installed."; }
need curl
need tar

# ── detect platform → release target triple ──────────────────────────────────
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Darwin)
    case "$arch" in
      arm64|aarch64) target="aarch64-apple-darwin" ;;
      x86_64)        target="x86_64-apple-darwin" ;;
      *) die "unsupported macOS architecture: $arch" ;;
    esac ;;
  Linux)
    case "$arch" in
      x86_64) target="x86_64-unknown-linux-gnu" ;;
      *) die "no prebuilt Linux binary for '$arch' — build from source instead." ;;
    esac ;;
  *) die "unsupported OS: $os (use install.ps1 on Windows)." ;;
esac

# ── resolve the latest release tag ────────────────────────────────────────────
info "Finding the latest Thoth release…"
tag="$(curl -fsSL --connect-timeout 10 --max-time 30 "https://api.github.com/repos/$REPO/releases/latest" \
  | grep -m1 '"tag_name"' \
  | sed -E 's/.*"tag_name"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/')"
[ -n "$tag" ] || die "could not determine the latest release tag."
info "Latest release: ${BOLD}${tag}${RESET}"

asset="${BIN_NAME}-${target}.tar.gz"
url="https://github.com/$REPO/releases/download/$tag/$asset"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

info "Downloading ${DIM}${asset}${RESET}"
curl -fSL --connect-timeout 10 --max-time 300 --progress-bar "$url" -o "$tmp/$asset" \
  || die "download failed: $url"
tar -xzf "$tmp/$asset" -C "$tmp" || die "failed to extract $asset"

# ── install ───────────────────────────────────────────────────────────────────
if [ "$os" = "Darwin" ]; then
  src="$tmp/$APP_NAME.app"
  [ -d "$src" ] || die "archive did not contain $APP_NAME.app"
  # The app is unsigned; strip the quarantine flag so Gatekeeper allows launch.
  xattr -cr "$src" 2>/dev/null || true

  dest="/Applications/$APP_NAME.app"
  info "Installing to ${BOLD}${dest}${RESET}"
  if rm -rf "$dest" 2>/dev/null && mv "$src" "$dest" 2>/dev/null; then
    :
  else
    warn "/Applications is not writable — retrying with sudo (you may be prompted)."
    sudo rm -rf "$dest"
    sudo mv "$src" "$dest"
  fi
  printf '%s✓ Installed %s %s%s — open it from /Applications or run: %sopen -a %s%s\n' \
    "$GREEN" "$APP_NAME" "$tag" "$RESET" "$DIM" "$APP_NAME" "$RESET"
else
  src="$tmp/$BIN_NAME"
  [ -f "$src" ] || die "archive did not contain the '$BIN_NAME' binary"
  chmod +x "$src"
  dest_dir="${THOTH_INSTALL_DIR:-$HOME/.local/bin}"
  mkdir -p "$dest_dir"
  info "Installing to ${BOLD}${dest_dir}/${BIN_NAME}${RESET}"
  mv "$src" "$dest_dir/$BIN_NAME"
  printf '%s✓ Installed %s %s%s\n' "$GREEN" "$BIN_NAME" "$tag" "$RESET"
  case ":$PATH:" in
    *":$dest_dir:"*) ;;
    *) warn "$dest_dir is not on your PATH. Add it, e.g.:"
       printf '    %sexport PATH="%s:$PATH"%s\n' "$DIM" "$dest_dir" "$RESET" ;;
  esac
fi

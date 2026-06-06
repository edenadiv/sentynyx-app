#!/usr/bin/env bash
#
# Sentynyx installer (macOS).
# Downloads the latest signed .dmg from GitHub Releases and installs
# Sentynyx.app into /Applications.
#
#   curl -fsSL https://raw.githubusercontent.com/edenadiv/sentynyx-app/main/scripts/install.sh | bash
#
set -euo pipefail

REPO="${SENTYNYX_REPO:-edenadiv/sentynyx-app}"
APP="Sentynyx"

err()  { printf '\033[31m✗ %s\033[0m\n' "$1" >&2; exit 1; }
info() { printf '\033[36m▸ %s\033[0m\n' "$1"; }
ok()   { printf '\033[32m✓ %s\033[0m\n' "$1"; }

[ "$(uname -s)" = "Darwin" ] || err "This installer is macOS-only. On Windows/Linux, build from source — see the README."
command -v curl >/dev/null  || err "curl is required."
command -v hdiutil >/dev/null || err "hdiutil not found (are you on macOS?)."

info "Finding the latest Sentynyx release…"
api="https://api.github.com/repos/${REPO}/releases/latest"
dmg_url="$(curl -fsSL "$api" | grep -o 'https://[^"]*\.dmg' | head -n1 || true)"
[ -n "${dmg_url:-}" ] || err "No .dmg asset in the latest release. Download manually: https://github.com/${REPO}/releases"

tmp="$(mktemp -d)"
mnt="/Volumes/${APP}"
cleanup() { hdiutil detach "$mnt" >/dev/null 2>&1 || true; rm -rf "$tmp"; }
trap cleanup EXIT

info "Downloading $(basename "$dmg_url")…"
curl -fSL --progress-bar "$dmg_url" -o "$tmp/${APP}.dmg" || err "Download failed."

info "Mounting disk image…"
hdiutil attach "$tmp/${APP}.dmg" -nobrowse -quiet || err "Could not mount the disk image."

src="$mnt/${APP}.app"
[ -d "$src" ] || src="$(/bin/ls -d "$mnt"/*.app 2>/dev/null | head -n1 || true)"
[ -d "$src" ] || err "Couldn't find ${APP}.app inside the disk image."

dest="/Applications/${APP}.app"
info "Installing to ${dest} (may prompt for your password)…"
if [ -w /Applications ]; then
  rm -rf "$dest"; cp -R "$src" "$dest"
else
  sudo rm -rf "$dest"; sudo cp -R "$src" "$dest"
fi

# Clear the quarantine bit so Gatekeeper launches the notarized app cleanly.
xattr -dr com.apple.quarantine "$dest" 2>/dev/null || true

ok "Installed ${APP} to /Applications."
info "Launch it from Spotlight, or run:  open -a ${APP}"

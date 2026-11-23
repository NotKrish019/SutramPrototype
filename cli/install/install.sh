#!/bin/sh
# YUKTI installer for macOS and Linux.
#
#   curl -fsSL https://raw.githubusercontent.com/NotKrish019/SutramPrototype/main/cli/install/install.sh | sh
#
# Installs `yukti` and the bundled `yukti-server` into ~/.yukti/bin, verifies the download's
# SHA-256, and adds the directory to your shell's PATH. Then run: yukti
#
# Environment:
#   YUKTI_VERSION      release tag to install, e.g. yukti-v0.1.0 (default: latest)
#   YUKTI_INSTALL_DIR  where the programs go (default: ~/.yukti/bin)
#   YUKTI_REPO         GitHub owner/repo (default: NotKrish019/SutramPrototype)
#   YUKTI_ARCHIVE      install from this local .tar.gz instead of downloading (testing)
#   YUKTI_NO_MODIFY_PATH=1  do not edit shell profiles

set -eu

REPO="${YUKTI_REPO:-NotKrish019/SutramPrototype}"
VERSION="${YUKTI_VERSION:-latest}"
INSTALL_DIR="${YUKTI_INSTALL_DIR:-$HOME/.yukti/bin}"

if [ -t 1 ]; then
  RED="$(printf '\033[1;38;5;161m')"; DIM="$(printf '\033[38;5;242m')"
  OK="$(printf '\033[38;5;72m')"; BOLD="$(printf '\033[1m')"; RESET="$(printf '\033[0m')"
else
  RED=""; DIM=""; OK=""; BOLD=""; RESET=""
fi

say()  { printf '  %s\n' "$*"; }
step() { printf '  %s▌%s %s\n' "$RED" "$RESET" "$*"; }
fail() { printf '  %serror:%s %s\n' "$RED" "$RESET" "$*" >&2; exit 1; }

printf '\n  %s▌ YUKTI%s  %sDigital Evidence Correlation & Crime Reconstruction%s\n\n' "$RED" "$RESET" "$DIM" "$RESET"

# --- platform -------------------------------------------------------------------------------
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Darwin) os_part="apple-darwin" ;;
  Linux)  os_part="unknown-linux-gnu" ;;
  MINGW*|MSYS*|CYGWIN*) fail "on Windows, use PowerShell: irm https://raw.githubusercontent.com/$REPO/main/cli/install/install.ps1 | iex" ;;
  *) fail "unsupported operating system: $os" ;;
esac
case "$arch" in
  arm64|aarch64) arch_part="aarch64" ;;
  x86_64|amd64)  arch_part="x86_64" ;;
  *) fail "unsupported CPU architecture: $arch" ;;
esac
if [ "$os_part" = "unknown-linux-gnu" ] && [ "$arch_part" = "aarch64" ]; then
  fail "Linux on ARM has no prebuilt release yet; build from source: cd cli && cargo install --path ."
fi
target="${arch_part}-${os_part}"
asset="yukti-${target}.tar.gz"
step "Platform ${BOLD}${target}${RESET}"

# --- download -------------------------------------------------------------------------------
need() { command -v "$1" >/dev/null 2>&1 || fail "'$1' is required"; }
need tar
tmp="$(mktemp -d 2>/dev/null || mktemp -d -t yukti)"
trap 'rm -rf "$tmp"' EXIT INT TERM

if [ -n "${YUKTI_ARCHIVE:-}" ]; then
  [ -f "$YUKTI_ARCHIVE" ] || fail "YUKTI_ARCHIVE not found: $YUKTI_ARCHIVE"
  cp "$YUKTI_ARCHIVE" "$tmp/$asset"
  if [ -f "$YUKTI_ARCHIVE.sha256" ]; then cp "$YUKTI_ARCHIVE.sha256" "$tmp/$asset.sha256"; fi
  step "Using local archive $YUKTI_ARCHIVE"
else
  if [ "$VERSION" = "latest" ]; then
    base="https://github.com/$REPO/releases/latest/download"
  else
    base="https://github.com/$REPO/releases/download/$VERSION"
  fi
  fetch() {
    if command -v curl >/dev/null 2>&1; then curl -fsSL "$1" -o "$2"
    elif command -v wget >/dev/null 2>&1; then wget -qO "$2" "$1"
    else fail "curl or wget is required"; fi
  }
  step "Downloading $asset ($VERSION)"
  fetch "$base/$asset" "$tmp/$asset" || fail "download failed: $base/$asset (is there a published release for $target?)"
  fetch "$base/$asset.sha256" "$tmp/$asset.sha256" || fail "checksum download failed: $base/$asset.sha256"
fi

# --- verify ---------------------------------------------------------------------------------
if [ -f "$tmp/$asset.sha256" ]; then
  expected="$(awk '{print $1}' "$tmp/$asset.sha256" | tr 'A-F' 'a-f')"
  if command -v sha256sum >/dev/null 2>&1; then actual="$(sha256sum "$tmp/$asset" | awk '{print $1}')"
  else actual="$(shasum -a 256 "$tmp/$asset" | awk '{print $1}')"; fi
  [ "$expected" = "$actual" ] || fail "SHA-256 mismatch: expected $expected, got $actual. Nothing was installed."
  step "SHA-256 verified ${OK}✓${RESET} ${DIM}$actual${RESET}"
else
  [ -n "${YUKTI_ARCHIVE:-}" ] || fail "no checksum available; refusing to install"
  say "${DIM}(local archive without .sha256: checksum not verified)${RESET}"
fi

# --- install --------------------------------------------------------------------------------
tar -xzf "$tmp/$asset" -C "$tmp"
src="$tmp/yukti-$target"
[ -x "$src/yukti" ] && [ -x "$src/yukti-server" ] || fail "archive does not contain yukti and yukti-server"
mkdir -p "$INSTALL_DIR"
# Stop a running server from an older install so its binary can be replaced.
if [ -x "$INSTALL_DIR/yukti" ]; then "$INSTALL_DIR/yukti" server stop >/dev/null 2>&1 || true; fi
for bin in yukti yukti-server; do
  cp "$src/$bin" "$INSTALL_DIR/$bin.new"
  chmod 755 "$INSTALL_DIR/$bin.new"
  mv -f "$INSTALL_DIR/$bin.new" "$INSTALL_DIR/$bin"
done
if [ "$os_part" = "apple-darwin" ] && command -v xattr >/dev/null 2>&1; then
  xattr -d com.apple.quarantine "$INSTALL_DIR/yukti" "$INSTALL_DIR/yukti-server" 2>/dev/null || true
fi
step "Installed to ${BOLD}$INSTALL_DIR${RESET}"

# --- PATH -----------------------------------------------------------------------------------
on_path=0
case ":$PATH:" in *":$INSTALL_DIR:"*) on_path=1 ;; esac
if [ "$on_path" = 0 ] && [ "${YUKTI_NO_MODIFY_PATH:-0}" != "1" ]; then
  line="export PATH=\"$INSTALL_DIR:\$PATH\""
  shell_name="$(basename "${SHELL:-sh}")"
  case "$shell_name" in
    zsh)  profiles="$HOME/.zshrc" ;;
    bash) if [ "$os_part" = "apple-darwin" ]; then profiles="$HOME/.bash_profile"; else profiles="$HOME/.bashrc"; fi ;;
    fish) profiles="" ;;
    *)    profiles="$HOME/.profile" ;;
  esac
  if [ "$shell_name" = "fish" ]; then
    mkdir -p "$HOME/.config/fish/conf.d"
    printf 'fish_add_path %s\n' "$INSTALL_DIR" > "$HOME/.config/fish/conf.d/yukti.fish"
    step "Added to PATH in ~/.config/fish/conf.d/yukti.fish"
  else
    for profile in $profiles; do
      if ! grep -qs "$INSTALL_DIR" "$profile"; then
        printf '\n# YUKTI\n%s\n' "$line" >> "$profile"
      fi
      step "Added to PATH in $profile"
    done
  fi
  reload="open a new terminal (or run: $line)"
else
  reload=""
fi

version="$("$INSTALL_DIR/yukti" --version 2>/dev/null || echo yukti)"
printf '\n  %s✓%s %s%s%s installed.\n\n' "$OK" "$RESET" "$BOLD" "$version" "$RESET"
if [ -n "$reload" ]; then say "Next: $reload, then run:"; else say "Next, run:"; fi
printf '\n      %syukti%s\n\n' "$BOLD" "$RESET"
say "${DIM}It starts the local YUKTI server and opens the interactive session. Type 'help' there.${RESET}"
say "${DIM}Uninstall: yukti server stop && rm $INSTALL_DIR/yukti $INSTALL_DIR/yukti-server (case data stays in ~/.yukti/data)${RESET}"
printf '\n'

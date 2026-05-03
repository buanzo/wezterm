#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: packaging/windows/build_and_install_owt.sh [--no-build] [--no-install] [--force-install]

Build the native Windows OWT GUI, regenerate the stripped OWT.exe artifact from
wezterm-gui.exe, and install it to the local Windows OWT profile directory.

Environment:
  OWT_WINDOWS_INSTALL_DIR   WSL path for the install dir
                            (default: /mnt/c/Users/Usuario/AppData/Local/OWT)
  OWT_RUST_TOOLCHAIN        Cargo toolchain selector (default: +1.75.0)
  OWT_TARGET                Rust target (default: x86_64-pc-windows-gnu)
  RUSTFLAGS                 Defaults to the OpenSSL alias workaround
  TARGET_CXXFLAGS           Defaults to -Wa,-mbig-obj for HarfBuzz
USAGE
}

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
INSTALL_DIR="${OWT_WINDOWS_INSTALL_DIR:-/mnt/c/Users/Usuario/AppData/Local/OWT}"
RUST_TOOLCHAIN="${OWT_RUST_TOOLCHAIN:-+1.75.0}"
TARGET="${OWT_TARGET:-x86_64-pc-windows-gnu}"
RUSTFLAGS_VALUE="${RUSTFLAGS:--L native=/tmp/owt-openssl-gnu-lib}"
TARGET_CXXFLAGS_VALUE="${TARGET_CXXFLAGS:--Wa,-mbig-obj}"
BUILD=1
INSTALL=1
FORCE_INSTALL=0

while (($#)); do
  case "$1" in
    --no-build)
      BUILD=0
      ;;
    --no-install)
      INSTALL=0
      ;;
    --force-install)
      FORCE_INSTALL=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

RELEASE_DIR="$ROOT_DIR/target/$TARGET/release"
GUI_EXE="$RELEASE_DIR/wezterm-gui.exe"
OWT_EXE="$RELEASE_DIR/OWT.exe"
INSTALLED_EXE="$INSTALL_DIR/OWT.exe"
STAGED_EXE="$INSTALL_DIR/OWT.next.exe"
CONFIG_SOURCE="$ROOT_DIR/packaging/windows/wezterm.lua"
INSTALLED_CONFIG="$INSTALL_DIR/wezterm.lua"

cd "$ROOT_DIR"

if ((BUILD)); then
  env \
    RUSTFLAGS="$RUSTFLAGS_VALUE" \
    TARGET_CXXFLAGS="$TARGET_CXXFLAGS_VALUE" \
    cargo "$RUST_TOOLCHAIN" build \
      -p wezterm-gui \
      --target "$TARGET" \
      --release \
      --features wezterm-ssh/vendored-openssl-ssh2
fi

if [[ ! -f "$GUI_EXE" ]]; then
  echo "Missing expected GUI binary: $GUI_EXE" >&2
  exit 1
fi

strip_tool="${OWT_STRIP_TOOL:-x86_64-w64-mingw32-strip}"
if ! command -v "$strip_tool" >/dev/null 2>&1; then
  echo "Missing strip tool: $strip_tool" >&2
  exit 1
fi

"$strip_tool" --strip-unneeded -o "$OWT_EXE" "$GUI_EXE"

echo "Built stripped artifact:"
sha256sum "$OWT_EXE"

if ((!INSTALL)); then
  exit 0
fi

mkdir -p "$INSTALL_DIR"
cp "$CONFIG_SOURCE" "$INSTALLED_CONFIG"

owt_running=1
if powershell.exe -NoProfile -Command \
  'if (Get-Process -Name OWT -ErrorAction SilentlyContinue) { exit 0 } else { exit 1 }' \
  >/dev/null 2>&1; then
  owt_running=0
fi

if ((owt_running == 0 && FORCE_INSTALL == 0)); then
  cp "$OWT_EXE" "$STAGED_EXE"
  echo "OWT is running; staged install at: $STAGED_EXE" >&2
  echo "Close OWT, then rerun with --no-build to install the fresh artifact." >&2
  exit 0
fi

cp "$OWT_EXE" "$INSTALLED_EXE"

echo "Installed binary:"
sha256sum "$INSTALLED_EXE"

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
STAGED_MANIFEST="$INSTALL_DIR/OWT.next.json"
PREVIOUS_EXE="$INSTALL_DIR/OWT.previous.exe"
CONFIG_SOURCE="$ROOT_DIR/packaging/windows/wezterm.lua"
INSTALLED_CONFIG="$INSTALL_DIR/wezterm.lua"

json_escape() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  printf '%s' "$value"
}

sha256_value() {
  sha256sum "$1" | awk '{print $1}'
}

owt_is_running() {
  powershell.exe -NoProfile -Command \
    'if (Get-Process -Name OWT -ErrorAction SilentlyContinue) { exit 0 } else { exit 1 }' \
    >/dev/null 2>&1
}

write_stage_manifest() {
  local source_exe="$1"
  local artifact_hash="$2"
  local built_at_utc
  built_at_utc="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  cat >"$STAGED_MANIFEST" <<MANIFEST
{
  "schema": 1,
  "status": "staged_pending_owt_exit",
  "built_at_utc": "$(json_escape "$built_at_utc")",
  "sha256": "$(json_escape "$artifact_hash")",
  "source_exe": "$(json_escape "$source_exe")",
  "staged_exe": "$(json_escape "$STAGED_EXE")",
  "install_exe": "$(json_escape "$INSTALLED_EXE")",
  "reason": "OWT.exe was running during install; close OWT and rerun with --no-build to promote this exact artifact"
}
MANIFEST
}

promote_staged_install() {
  if [[ ! -f "$STAGED_EXE" ]]; then
    return 1
  fi
  if owt_is_running && ((FORCE_INSTALL == 0)); then
    echo "OWT is still running; staged install remains at: $STAGED_EXE" >&2
    if [[ -f "$STAGED_MANIFEST" ]]; then
      echo "Stage manifest: $STAGED_MANIFEST" >&2
    fi
    return 0
  fi

  local staged_hash expected_hash
  staged_hash="$(sha256_value "$STAGED_EXE")"
  expected_hash=""
  if [[ -f "$STAGED_MANIFEST" ]]; then
    expected_hash="$(sed -n 's/.*"sha256"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$STAGED_MANIFEST" | head -n 1)"
  fi
  if [[ -n "$expected_hash" && "$expected_hash" != "$staged_hash" ]]; then
    echo "Staged hash mismatch:" >&2
    echo "  manifest: $expected_hash" >&2
    echo "  staged:   $staged_hash" >&2
    exit 1
  fi

  if [[ -f "$INSTALLED_EXE" ]]; then
    cp "$INSTALLED_EXE" "$PREVIOUS_EXE"
  fi
  cp "$STAGED_EXE" "$INSTALLED_EXE"
  rm -f "$STAGED_EXE" "$STAGED_MANIFEST"

  echo "Promoted staged binary:"
  sha256sum "$INSTALLED_EXE"
  return 0
}

cd "$ROOT_DIR"

if ((INSTALL)); then
  mkdir -p "$INSTALL_DIR"
  cp "$CONFIG_SOURCE" "$INSTALLED_CONFIG"
  if ((BUILD == 0)) && promote_staged_install; then
    exit 0
  fi
fi

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

if owt_is_running && ((FORCE_INSTALL == 0)); then
  cp "$OWT_EXE" "$STAGED_EXE"
  artifact_hash="$(sha256_value "$STAGED_EXE")"
  write_stage_manifest "$OWT_EXE" "$artifact_hash"
  echo "OWT is running; staged install at: $STAGED_EXE" >&2
  echo "Stage manifest: $STAGED_MANIFEST" >&2
  echo "Close OWT, then rerun with --no-build to install the fresh artifact." >&2
  exit 0
fi

if [[ -f "$STAGED_EXE" ]]; then
  rm -f "$STAGED_EXE" "$STAGED_MANIFEST"
fi
cp "$OWT_EXE" "$INSTALLED_EXE"

echo "Installed binary:"
sha256sum "$INSTALLED_EXE"

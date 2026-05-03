# OWT Terminal Baseline

This source tree is the first native OWT terminal fork candidate. It starts from
WezTerm tag `20240203-110809-5046fc22` at commit
`5046fc225992db6ba2ef8812743fadfdfe4b184a`.

Local branches:

- `owt/baseline-20240203`: clean upstream baseline branch.
- `owt/windows-gnu-bootstrap`: first Windows GNU bootstrap branch.

## Windows GNU Bootstrap

The first native Windows build was produced from WSL with Rust `1.75.0` and the
`x86_64-pc-windows-gnu` target.

One source change is currently required on `owt/windows-gnu-bootstrap`:
`wezterm-ssh` defaults to the `ssh2` backend only. This keeps SSH support
available while avoiding the `libssh-rs` vendored C build, which skipped
Windows thread objects during cross-compilation from Linux and failed final
linking on unresolved `ssh_mutex_*` and `ssh_threads_get_default` symbols.

Working release build after `openssl-sys` has produced its vendored OpenSSL
libraries:

```sh
rustup toolchain install 1.75.0 --profile minimal
rustup target add x86_64-pc-windows-gnu --toolchain 1.75.0

OPENSSL_SSL_A="$(find target/x86_64-pc-windows-gnu -path '*/openssl-build/install/lib/libssl.a' | head -n 1)"
test -n "$OPENSSL_SSL_A"
OPENSSL_LIB_DIR="$(dirname "$OPENSSL_SSL_A")"
mkdir -p /tmp/owt-openssl-gnu-lib
ln -sf "$OPENSSL_LIB_DIR/libssl.a" /tmp/owt-openssl-gnu-lib/liblibssl.a
ln -sf "$OPENSSL_LIB_DIR/libcrypto.a" /tmp/owt-openssl-gnu-lib/liblibcrypto.a

RUSTFLAGS='-L native=/tmp/owt-openssl-gnu-lib' \
TARGET_CXXFLAGS='-Wa,-mbig-obj' \
cargo +1.75.0 build \
  -p wezterm-gui \
  --target x86_64-pc-windows-gnu \
  --release \
  --features wezterm-ssh/vendored-openssl-ssh2
```

Notes:

- `TARGET_CXXFLAGS='-Wa,-mbig-obj'` is required for the vendored HarfBuzz
  object size on Windows GNU.
- The `liblibssl.a` and `liblibcrypto.a` aliases satisfy older Windows GNU
  OpenSSL link names emitted by `libssh2-sys`/`openssl-sys`.
- On a clean tree, the first build may need to run far enough for
  `openssl-sys` to create `openssl-build/install/lib/`; if it fails afterward
  with a missing `libssl`, create the aliases and rerun the build. The
  `openssl-sys-*` hash is not stable. Find it with:

```sh
find target/x86_64-pc-windows-gnu -path '*/openssl-build/install/lib/libssl.a'
```

The first successful artifacts were:

- Debug: `target/x86_64-pc-windows-gnu/debug/wezterm-gui.exe`
  (`852M`, PE32+ GUI).
- Release: `target/x86_64-pc-windows-gnu/release/wezterm-gui.exe`
  (`102M`, PE32+ GUI).
- Stripped release copy: `target/x86_64-pc-windows-gnu/release/OWT.exe`
  (`73M`, PE32+ GUI).

Use `packaging/windows/build_and_install_owt.sh` for current Windows GNU
install iterations. It builds `wezterm-gui.exe`, regenerates the stripped
`OWT.exe` from that fresh GUI binary, installs the portable sibling config, and
prints target/installed hashes. This matters because `cargo build` updates
`wezterm-gui.exe`; `target/.../release/OWT.exe` is only current after the strip
step.

Windows install performed on 2026-05-01:

- Installed native binary:
  `C:\Users\Usuario\AppData\Local\OWT\OWT.exe`
- Installed portable config beside the executable:
  `C:\Users\Usuario\AppData\Local\OWT\wezterm.lua`
- Preserved previous WSLg launcher:
  `C:\Users\Usuario\AppData\Local\OWT\OWT.exe.launcher-20260501-211054.bak`

The installed `OWT.exe` launched successfully as a Windows native process:
`C:\Users\Usuario\AppData\Local\OWT\OWT.exe`.

As of 2026-05-02, `OWT.exe` auto-selects `wezterm.lua` beside the executable
when no `--config-file` is provided. This is required for taskbar/Explorer
launches, which pass no arguments. The portable config sets a Windows-present
font fallback stack:

```lua
wezterm.font_with_fallback({
  "Cascadia Mono",
  "Consolas",
  "Courier New",
})
```

This fixed a no-window launch regression where no-arg `OWT.exe` fell back to
upstream's default JetBrains Mono font and exited with `failed to get font
metrics` on this Windows install.

The portable config sets the default program to:

```text
wsl.exe -d Ubuntu --cd /home/buanzo/git/tools --exec /bin/bash -l
```

This keeps the native Windows terminal process while opening an Ubuntu WSL Bash
login shell instead of `cmd.exe`.

The portable config forwards native endpoint variables into the WSL shell via
`WSLENV` when `OWT.exe` exports them: `OWT_NATIVE_ENDPOINT`,
`OWT_NATIVE_TOKEN`, `OWT_NATIVE_PROTOCOL`, and `OWT_NATIVE_PID`. `OWT.exe` also
writes the same live endpoint data to
`%LOCALAPPDATA%\\OWT\\native-endpoint.json` and removes that locator on normal
exit if it still matches the current process token. This lets
`owt_current_bridge` recover the endpoint from WSL/MCP processes that did not
inherit the fresh environment. The locator contains the bearer token and must
remain local runtime state, not committed or pasted into logs. If the embedded
endpoint is unavailable or the locator is stale, the bridge/config falls back to
`OWT_NATIVE_WINDOW_ONLY=1` and `OWT_CONTROL_PLANE=native-window-only` so agents
do not attach to a stale WSLg prototype backend by mistake.

As of 2026-05-02, the embedded endpoint supports status plus semantic
`validate_interface`/`apply_interface`, side-effect-free `dispatch_action`,
and local interface lifecycle calls: `list_interfaces`, `save_interface`, and
`load_interface`. It also supports `update_node` as the first granular semantic
build operation: one node can be added or replaced against the active interface,
with one action optionally bound in the same transaction. Saved interface
documents are stored under the local OWT profile, not arbitrary caller-supplied
paths. The GUI paint path renders a
compact static LCARS control band for the active applied interface.
`owt_status` reports `native_render_passes` so the native render path can be
verified. The tools-repo bridge now filters native-mode MCP tools to the native
subset and exposes `apply_lcars_panel`, which builds and applies a simple
semantic panel through `OWT.exe`. Native LCARS layout now defaults to docked
top-band mode so terminal text renders below the panel; reserved `right_rail`
and `bottom_strip` modes can reflow terminal space around side/bottom surfaces;
`compact_top`, `right_browser`, and `alert_strip` profile aliases are accepted
as intent hints for the current static renderer; `structural_console` is now
accepted as the first top+left LCARS frame/profile, reserving left rail space
and treating the terminal as a content bay under the native chrome; explicit
overlay mode remains available for HUD-style interfaces. The first side-effect-free action dispatch
path is also present: `dispatch_action` records declared action ids and native
status reports `last_dispatched_action`; button hitboxes and
`Ctrl+Alt+Shift+1..9` both feed that native dispatch state. Current LCARS
top/right/bottom surfaces share marker/button primitives, avoid marker-over-text
rendering, show active `ACK` feedback for the last dispatched action, and keep
the default top band compact with thinner header geometry. The current native
renderer now has an RGBA image-quad substrate for anti-aliased rounded LCARS
chrome, first applied to top-band header bars and then tightened toward harder
LCARS console geometry: right-capped structural bars and dark command paddles
instead of generic filled desktop pills. The semantic renderer now recognizes
`frame`, `side_rail`, `bar_run`, `content_bay`, `data_cascade`, and
`command_grid` as first-class native LCARS construction primitives. This is the bridge toward rendering the
richer `owt.ui` visual grammar natively rather than trying to approximate it
with flat rectangles.
Pane control, external action execution, import/export package lifecycle, and
full scene-graph layout remain future native work. Manual live
mouse-click verification is still separate from MCP-level dispatch
verification.

## Windows App Identity And Icon

OWT branding is now part of the Windows GNU build:

- `wezterm-gui/build.rs` runs Windows resource embedding based on the Cargo
  `TARGET`, not the build host OS. This is required for Linux-hosted Windows
  cross-builds; otherwise the executable has no `.rsrc` section and pinned
  taskbar shortcuts can collapse to a generic icon.
- `wezterm-gui/Cargo.toml` keeps `cc` and `embed-resource` in normal
  `build-dependencies` so the Linux build host can compile the Windows resource
  script.
- `assets/windows/owt.ico` is preferred over upstream `terminal.ico` when
  present.
- The Windows AppUserModelID is `org.buanzo.owt`.
- Version metadata now reports `OWT` / `OWT - LCARS Agentic Terminal`.
- The pinned taskbar and Start Menu shortcuts should point at
  `%LOCALAPPDATA%\OWT\OWT.exe`, use `%LOCALAPPDATA%\OWT` as working directory,
  pass no arguments, and set icon location to `%LOCALAPPDATA%\OWT\OWT.ico,0`.

Verified artifact:

```sh
x86_64-w64-mingw32-objdump -x \
  target/x86_64-pc-windows-gnu/release/wezterm-gui.exe |
  rg -n 'Resource Directory|\\.rsrc|Entry 2' -C 2
```

When `%LOCALAPPDATA%\OWT\OWT.exe` is locked by a running OWT process, let
`packaging/windows/build_and_install_owt.sh` stage the stripped build as
`%LOCALAPPDATA%\OWT\OWT.next.exe`, close all OWT windows, then rerun the script
with `--no-build` to replace `OWT.exe`.

## Linux Baseline

Linux build was attempted with:

```sh
cargo +1.75.0 build -p wezterm-gui
```

Rust `1.75.0` avoids the `time 0.3.31` inference failure seen with Rust
`1.91.0`, but the current Ubuntu environment is missing X11/XCB development
packages. The first blocker was `x11-xcb.pc`.

Likely packages needed before retrying:

```sh
sudo apt install \
  libx11-xcb-dev \
  libxcb-randr0-dev \
  libxcb-xkb-dev \
  libxcb-icccm4-dev \
  libxcb-keysyms1-dev \
  libxcb-image0-dev \
  libxcb-ewmh-dev \
  libxkbcommon-x11-dev
```

Do not run this with automation; ask the operator to run `sudo`.

## Artifact Policy

Generated binaries and `target/` contents are build artifacts. They are useful
locally but must not be committed. Do not use Git LFS for these artifacts.

The next product step is to move beyond the first static LCARS renderer and
local save/load lifecycle: verify human click dispatch, add permissioned action
executors, add richer layout profiles, add import/export package handling,
promote branded defaults and launch profiles, and decide the durable Windows
build target GNU-vs-MVC. The embedded control plane direction is described in
`OWT_NATIVE_MCP.md`; the side-effect-free Rust scaffold for that work is
`owt-control/`, which currently owns typed protocol, interface document,
validation, runtime state, and native lifecycle tool-name constants.

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
step. If OWT is running, the script stages `OWT.next.exe` with an
`OWT.next.json` manifest containing the staged hash and source/target paths;
rerunning with `--no-build` promotes that exact staged artifact after OWT exits.

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

As of 2026-05-15, the portable OWT profile enables
`owt_lcars_window_chrome = true` with
`window_decorations = "INTEGRATED_BUTTONS|RESIZE"`. This keeps native Windows
resize borders and integrated window buttons while rendering an OWT-owned LCARS
custom titlebar path with a top-left OWT roundel, LCARS tab slabs, and a
reserved left rail for the native surface menu.

The LCARS window chrome build was installed on 2026-05-15 with matching target
and installed binary SHA256:
`fa5e1e8a453445af8d23aaf2e8a7bc8dff724d97aeb32039cb598c135b080ddf`.

The 2026-05-15 all-native LCARS visual grammar pass was installed with matching
target and installed binary SHA256:
`90497837db50ad251a6c7de531a629424c6c0f78f5890fc959f6e618329b3fc3`.
It applies OWT-owned, TheLCARS-inspired geometry and palette roles to native
window chrome and LCARS surfaces without vendoring web templates, fonts, or
audio assets.

The 2026-05-15 native LCARS surface-menu redo replaced the old text overlay
with a terminal-owned radial/pie menu rendered from native geometric/path
primitives. The path keeps saved-interface loading plus hide/show/dock/overlay
layout actions inside OWT, moves the readout bay outside the pie when space
allows, and does not use generated raster cap assets. The installed binary
SHA256 is:
`a54005753e1006921bcc0dfba1827424057956b789137634d96faf6fd8872ae0`.
Visual evidence was archived under:
`/home/buanzo/git/tools/output/owt/lcars-screenshots/2026-05-15/20260515T185059Z-lcars-redo-final-idle/`
and
`/home/buanzo/git/tools/output/owt/lcars-screenshots/2026-05-15/20260515T185126Z-lcars-redo-final-menu/`.

The 2026-05-16 native LCARS surface-menu refinement replaced that radial/pie
menu with an anchored elbow/slab chassis. It keeps the same native
hide/show/dock/overlay/load mutations, but the visible menu is now a connected
LCARS header, exit/back slab, vertical command bank, framed status bay, and
segmented left-rail extension with rectangular hit testing. The installed
binary SHA256 is:
`63d875f328c1a3c69aeb1ffaefde576f84a5b7ad6b72255c00eb014b015685fa`.
Visual evidence was archived under:
`/home/buanzo/git/tools/output/owt/lcars-screenshots/2026-05-16/20260516T104233Z-lcars-elbow-menu-polished/`.

The 2026-05-15 functional frame refinement tightened the same native LCARS
window chrome: the tab band is shorter, the left reservation was first
narrowed for terminal space, then widened again after visual review so the rail
uses fewer larger geometry-only sidebar slabs instead of dense barcode ticks.
The terminal grid remains reserved to the right of the rail. The roundel still
opens the native LCARS surface menu. The installed binary SHA256 is:
`cd543e25a626e47dcef0f367a83641507ef23dba75c8679a5bfa1930f27eac1f`.
Visual evidence was archived under:
`/home/buanzo/git/tools/output/owt/lcars-screenshots/2026-05-15/20260515T222241Z-lcars-functional-frame-idle/`
and
`/home/buanzo/git/tools/output/owt/lcars-screenshots/2026-05-15/20260515T222518Z-lcars-functional-frame-roundel-menu/`.

The 2026-05-31 prompt-legibility refinement kept the same native chrome path
but narrowed the colored left rail slabs, increased the black gutter, and
added a prompt-clearance gap below the tab band so shell text is not visually
boxed in by a continuous orange column. The installed binary SHA256 is:
`7d5f2097733ff12f5880afec17e7632ab0a6b7ffb7116ae867ff6821031f3974`.

The 2026-05-31 LCARS button reservation fix moved persistent one-button
surfaces back to semantic placement: normal `corner_button`/`single_button`
documents reserve terminal space by default, old unopted overlay coordinates are
normalized away by native projection, and only explicit terminal overlays keep
floating geometry. The follow-up compact reservation pass keeps that reserved
button from consuming a full top-panel band, so maximized shells start just
below the button rather than several rows lower. The installed binary SHA256 is:
`836b91809cd4fc777c078ef08edc2b67d74e1b4f8e56dbf4c13077b981e4070b`.

The follow-up same-day scene-reflow build keeps coordinate resolution inside
native OWT: active scope surfaces are ordered by latest apply, and compact
auxiliary `corner_button`/`single_button` controls automatically move to a free
reserved edge when the current scope owns their requested slot. Genetica can
own the top band while a secondary RENDERS button becomes a bottom strip
instead of causing a same-top conflict. Because OWT was running, this build was
staged as `%LOCALAPPDATA%\OWT\OWT.next.exe` with SHA256:
`394ff72c2a7bd1cc4b1fe2f2a0a663e82df992ae1f5a715df07ee4b7e44e20b3`.

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
with one action optionally bound in the same transaction, and
`patch_interface_layout` as the first layout mutation operation for
dock/origin/orientation/reservation/visibility changes without replacing the
semantic tree. Saved interface documents are stored under the local OWT
profile, not arbitrary caller-supplied paths. The GUI paint path renders
compact/static LCARS surfaces for active applied interface state and can paint
more than one active docked surface by merging terminal reservations across
left/top/right/bottom docks.
`owt_status` reports `native_render_passes` so the native render path can be
verified. The tools-repo bridge now filters native-mode MCP tools to the native
subset and exposes `apply_lcars_panel`, which builds and applies a simple
semantic panel through `OWT.exe`. Native LCARS layout now defaults to docked
top-band mode so terminal text renders below the panel; reserved `left_rail`,
`right_rail`, and `bottom_strip` modes can reflow terminal space around
side/bottom surfaces; `compact_top`, `left_browser`, `right_browser`, and
`alert_strip` profile aliases are accepted as intent hints for the current
static renderer; `structural_console` is now accepted as the first top+left
LCARS frame/profile; `lcars_shell` / `daily_shell` are the current
operator-facing readable-shell aliases for the low-noise docked composition,
with `docked_surface` kept as the renderer-level profile name;
`thelcars_control_panel` is the private, non-distributed TheLCARS.com demo
profile with a dedicated native control-panel composition renderer; and explicit
overlay mode remains available for HUD-style interfaces. The first native action dispatch
path is also present: `dispatch_action` records declared action ids and native
status reports `last_dispatched_action`; button hitboxes and
`Ctrl+Alt+Shift+1..9` both feed that native dispatch state. External
execution intent from HTTP/MCP `dispatch_action`/`request_refresh` enters a
terminal-owned approval state: callers can request execution but cannot grant it.
Renderer clicks are terminal UI input; native OWT can directly open local
file/path targets and can fork structured `run.argv` actions only when the
active interface root allowlists the exact command.
Current LCARS
top/left/right/bottom surfaces share marker/button primitives, avoid marker-over-text
rendering, show active `ACK` feedback for the last dispatched action, and keep
manual surface controls inside OWT: secondary-clicking a tab title opens the
terminal-owned native LCARS elbow/slab surface menu for hide/show,
dock/overlay moves, and loading saved profile-store LCARS documents. Declared
action buttons now double as surface handles: left-click still dispatches on
release, left-drag moves or docks the owning surface, and right-click opens the
surface menu for tiny one-button overlays. Dragging now paints an LCARS
drop-target preview, and one-button surfaces honor left/right/top/bottom
anchors instead of staying visually pinned to the right edge. The custom titlebar can now render
OWT LCARS window chrome around those surfaces without taking over native resize
semantics.
The default top band remains compact with thinner header geometry. The current native
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
`%LOCALAPPDATA%\OWT\OWT.next.exe` plus `%LOCALAPPDATA%\OWT\OWT.next.json`, close
all OWT windows, then rerun the script with `--no-build` to verify the staged
hash and replace `OWT.exe`.

Native OWT keeps operator layout preferences separate from project defaults.
Manual LCARS surface moves are stored as profile-local layout overrides under
the OWT profile and reapplied by interface id/scope when the same semantic
document is loaded again. Project `LCARS.md` files should keep portable default
intent; local placement memory belongs to OWT.

## LCARS Window Chrome Baseline

The current native LCARS demo frame makes the window itself part of the
interface: `OWT.exe` owns a custom top-left roundel, LCARS-styled tab slabs,
integrated title controls, and a reserved left command rail around the terminal
client area. The current rail pass favors TheLCARS-like large sidebar slabs and
a stronger top-left elbow over many small ticks, so the frame reads as LCARS
without competing with shell text. The left window rail is intentionally
geometry-only in this render phase; do not call the terminal text renderer from
the decoration rail path, because that can leave the Windows client area blank
even though the process and native endpoint remain responsive.

The private TheLCARS evaluation path may read ignored local template CSS metrics
from the tools repo and expose them as semantic interface metadata. Native OWT
uses those signals for visible included-theme and palette/frame/bar cues in the
dedicated control-panel profile; it still must not embed TheLCARS HTML, CSS,
JavaScript, fonts, audio, or other template payload into the terminal binary.

Verified Windows install:

```text
OWT.exe sha256 e4b1e4ce44820fa2db799df99bed7a2c3105516ca93f47506e8177ed93bfe55e
native_rendering_ready=true
native_render_passes=6
capture output/owt/lcars-screenshots/2026-05-18/20260518T144547Z-thelcars-window-is-lcars-final/owt-window.png
right-control check output/owt/lcars-screenshots/2026-05-18/20260518T144656Z-lcars-native-chrome-right-buttons/owt-window.png
```

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
local save/load lifecycle: verify human click dispatch, add approved external
action runners, add richer layout profiles, add import/export package handling,
promote branded defaults and launch profiles, and decide the durable Windows
build target GNU-vs-MVC. The embedded control plane direction is described in
`OWT_NATIVE_MCP.md`; the side-effect-free Rust scaffold for that work is
`owt-control/`, which currently owns typed protocol, interface document,
validation, runtime state, and native lifecycle tool-name constants.

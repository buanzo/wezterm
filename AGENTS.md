# OWT Terminal Agent Instructions

This repository is the native OWT terminal fork candidate. It is derived from
upstream WezTerm and currently tracks baseline tag
`20240203-110809-5046fc22` at commit
`5046fc225992db6ba2ef8812743fadfdfe4b184a`.

OWT has two active code homes:

- `/home/buanzo/git/owt-terminal`: this repository, the native terminal binary
  fork. Use it for `OWT.exe`, app identity, icons, launch profiles, terminal
  runtime work, packaging, and eventual in-process UI/runtime features.
- `/home/buanzo/git/tools/rust/owt`: the current prototype/spec/control-plane
  workspace. It still owns the Rust MCP bridge, Unix-socket backend, WezTerm Lua
  prototype, LCARS panel renderers, and `owt.ui` protocol experiments.

Do not blur those boundaries. A native `OWT.exe` that launches WSL Bash is only
the window/shell layer unless the embedded endpoint is present. The current
native endpoint can report status, validate/apply semantic interface state,
paint compact or structural LCARS surfaces from the active interface, render first-pass
semantic primitive classes, and record side-effect-free action dispatch for
declared action ids. It also provides first local interface lifecycle calls:
list, save, and load semantic interface documents from the OWT profile store.
The tools-repo bridge also exposes native `apply_lcars_panel` as the current
high-level agent path for simple semantic panels and filters native-mode MCP
tools to the native subset. Native LCARS layout defaults to docked top-band
mode, with reserved `right_rail` and `bottom_strip` regions now available for
simple semantic panels; `compact_top`, `right_browser`, and `alert_strip`
profile aliases are accepted as current static-renderer intent hints;
`structural_console` is the first top+left LCARS frame profile, reserving left
rail space and treating the terminal as a content bay under native chrome;
explicit overlay mode is reserved for HUD-style panels.
Button-node hitboxes and `Ctrl+Alt+Shift+1..9` action-slot keyboard fallback
are wired to the native dispatch path. The current renderer shares LCARS
marker/button primitives across top/right/bottom surfaces, keeps marker chrome
out from under text, recognizes first-class `frame`, `side_rail`, `bar_run`,
`content_bay`, `data_cascade`, `table`, and `command_grid` primitives, can draw
a structural-console table bay with headers, severity lanes, row/column
overflow, and stable weighted geometry beyond four columns, promotes table or
information-shape profiles such as `fleet_matrix` into the structural path when
no explicit side/bottom layout overrides them, uses a first layout planner to
choose inline or stacked table/data detail bays and show explicit signal/action
overflow counters when geometry is tight, wraps long structural signal rows
within their reserved row budget instead of letting them run into adjacent
LCARS regions, suppresses the structural command bay when no actions are
declared, and reclaims that horizontal space for actionless status/table/detail
interfaces,
uses harder console bars/command paddles instead of generic rounded desktop
pills, and shows active `ACK` feedback for the most recently dispatched action.
The native endpoint also has the first granular
semantic build operation, `update_node`, which adds or replaces one node against
the active interface and can bind one action in the same transaction; external
action execution, pane control, arbitrary
import/export packages, and full scene-graph native UI layouts are not native
yet.

## Working Branches

- `owt/baseline-20240203`: clean upstream baseline. Keep it clean.
- `owt/windows-gnu-bootstrap`: first native Windows GNU bootstrap branch.

Prefer narrow, reviewable commits and keep OWT changes isolated from unrelated
upstream churn.

## Build Notes

Current Windows GNU release build from WSL:

```sh
RUSTFLAGS='-L native=/tmp/owt-openssl-gnu-lib' \
TARGET_CXXFLAGS='-Wa,-mbig-obj' \
cargo +1.75.0 build \
  -p wezterm-gui \
  --target x86_64-pc-windows-gnu \
  --release \
  --features wezterm-ssh/vendored-openssl-ssh2
```

See `OWT_BASELINE.md` for the OpenSSL alias workaround, HarfBuzz object-size
flag, resource-embedding details, Linux package blockers, and install notes.

## Artifact Policy

Never commit generated binaries, `target/`, staged Windows AppData artifacts, or
large render/build outputs. Do not introduce Git LFS. Keep icons/source assets
small enough for normal Git.

## Windows Identity

The Windows app identity currently includes:

- `assets/windows/owt.ico`
- version metadata reporting `OWT` / `OWT - LCARS Agentic Terminal`
- AppUserModelID `org.buanzo.owt`
- resource embedding driven by Cargo `TARGET`, so Linux-hosted Windows
  cross-builds produce a real `.rsrc` section

If any app identity, icon, launch profile, or install path changes, update all
of these in the same work unit when applicable:

- `OWT_BASELINE.md`
- `CONTRIBUTORS.txt`
- `/home/buanzo/git/tools/SCRIPTS_GUIDE.md`
- `/home/buanzo/git/tools/.plans/owt/agentic-control-terminal.md`
- `/home/buanzo/git/tools/.plans/owt/native-windows-owt.md`

## Current Product Direction

The immediate target is a daily-drivable native terminal shell:

- native Windows `OWT.exe`
- default WSL Ubuntu shell into `/home/buanzo/git/tools`
- stable Windows pin/taskbar identity
- branded launch profiles and defaults
- compatibility with the current prototype backend where practical

On Windows, `OWT.exe` auto-selects `wezterm.lua` beside the executable when no
`--config-file` is provided. Keep that behavior intact: pinned taskbar launches
and ordinary Explorer launches pass no arguments, so the sibling portable config
is the installed default profile. That config also sets a Windows-present font
fallback stack (`Cascadia Mono`, `Consolas`, `Courier New`) so launch does not
depend on upstream's default JetBrains Mono being installed.

The installed Windows config forwards `OWT_NATIVE_ENDPOINT`,
`OWT_NATIVE_TOKEN`, `OWT_NATIVE_PROTOCOL`, and `OWT_NATIVE_PID` into WSL when
the embedded endpoint is available. `OWT.exe` also writes a per-process
`native-endpoint.json` locator under the local OWT profile so WSL-side bridge
processes that started before those environment variables were exported can
recover the current endpoint and token. The locator is sensitive runtime state:
do not commit it, paste the token, or treat it as public diagnostic output. If
the endpoint is unavailable, the bridge/config falls back to
`OWT_NATIVE_WINDOW_ONLY=1` and
`OWT_CONTROL_PLANE=native-window-only`. This prevents Codex from claiming that
LCARS panels were drawn in the native window when only a stale WSLg prototype
backend is visible elsewhere on the system.

The longer target is terminal-owned OWT runtime state, semantic UI scene graph,
LCARS theme primitives, and agentic interface lifecycle inside the terminal.
Use `/home/buanzo/git/tools/.plans/owt/agentic-control-terminal.md` as the
current product plan.

Architecture decision: the MCP/control plane belongs inside `OWT.exe`. Do not
design the native product around a permanent external daemon that guesses window
identity and shells out to `wezterm cli`. Keep the current tools-repo
`owt_current_bridge`/`owt_backend` path as a prototype and migration adapter.
Use `OWT_NATIVE_MCP.md` as the native-control-plane direction.

The `owt-control/` crate is the first native-runtime scaffold. It should remain
side-effect free: protocol types, interface document types, validation, runtime
state, and tool-name constants only. GUI wiring, transport listeners, file I/O,
and agent/MCP startup loops belong in later integration layers so endpoint
startup stays deterministic.

The first GUI-side endpoint lives in `wezterm-gui/src/owt_native.rs`. It binds
a Windows-loopback HTTP endpoint, generates a per-process token, exports
`OWT_NATIVE_*` values before Lua config loads, and lets the portable Windows
config pass them into WSL. It currently supports status, semantic interface
validation/application state, side-effect-free declared action dispatch, and
local list/save/load for interface documents in the OWT profile store.
`wezterm-gui/src/termwindow/render/owt_lcars.rs` renders the first compact or
structural LCARS surface for the active interface, registers button-node
hitboxes, and the status response reports `native_render_passes` so agents can
verify the native paint path ran. The renderer now has shared marker/button
primitives, structural frame/content-bay primitives, first structural-console
layout planning, table/information-shape structural promotion, row/column
overflow reporting, and active dispatch feedback, but it is still an early
native scene slice. Structural signal text now has bounded word wrapping plus
ellipsis overflow for the current top/left structural console path; full
layout fit scoring, focus/drilldown navigation, and richer table/cohort
semantics remain pending.
Keep the listener on loopback; the tools-repo bridge can use Windows interop
when WSL cannot reach Windows loopback directly. Do not expand this endpoint
into pane mutation, external action execution, arbitrary import/export
packages, or broader UI semantics without also updating `OWT_NATIVE_MCP.md`,
the tools-repo bridge docs, and the active OWT plans.

## Root Tools Coordination

When a task is about LCARS panels, `owt_current`, the prototype backend, shell
context, or existing project scope interfaces, work in `/home/buanzo/git/tools`
and load:

- `/home/buanzo/git/tools/sub_agents/lcars_user_interface_owt_rules/AGENTS.md`
- `/home/buanzo/git/tools/rust/owt/README.md`
- `/home/buanzo/git/tools/rust/owt/LCARS.md`

When a task is about native terminal source, build, packaging, icon, Windows
pinning, or app identity, work here first and use the tools repo docs only to
keep continuity current.

## Security And Compatibility

Take tokens and operational secrets only from environment variables. Do not
hardcode private paths beyond documented local development/install paths. Ask
the operator before any `sudo` package install. Preserve upstream license
information and avoid changes that make rebasing against WezTerm unnecessarily
hard.

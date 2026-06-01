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
paint compact or structural LCARS surfaces from active interface state, render first-pass
semantic primitive classes, and record side-effect-free action dispatch for
declared action ids. It also provides first local interface lifecycle calls:
list, save, and load semantic interface documents from the OWT profile store.
The tools-repo bridge also exposes native `apply_lcars_panel` as the current
high-level agent path for simple semantic panels and filters native-mode MCP
tools to the native subset. Native LCARS layout defaults to docked top-band
mode, with reserved `left_rail`, `right_rail`, and `bottom_strip` regions now
available for simple semantic panels; `compact_top`, `left_browser`,
`right_browser`, and `alert_strip` profile aliases are accepted as current
static-renderer intent hints;
`structural_console` is the first top+left LCARS frame profile, reserving left
rail space and treating the terminal as a content bay under native chrome;
`docked_surface` is the current low-noise public/default docked LCARS
composition profile, with `sleek_console` retained only as a legacy alias;
`thelcars_control_panel` is the private, non-distributed TheLCARS.com demo
profile with a dedicated native control-panel composition renderer;
explicit overlay mode is reserved for HUD-style panels.
Button-node hitboxes and `Ctrl+Alt+Shift+1..9` action-slot keyboard fallback
are wired to the native dispatch path. Native validation responses should expose
soft `validation_warnings` for actionable buttons not covered by those automatic
slots or by explicit shortcut metadata, and for external/shell-capable actions
that lack a root action allowlist entry. The current renderer shares LCARS
marker/button primitives across top/right/bottom surfaces, keeps marker chrome
out from under text, recognizes first-class `frame`, `side_rail`, `bar_run`,
`content_bay`, `data_cascade`, `table`, and `command_grid` primitives, can draw
a tactical-systems-style `block_composition` surface from labelled data boxes,
a function column, a graphics viewport, ruler/tick affordances, and local
command controls, can reflow that same block-composition document into top,
side-rail, or bottom-strip placement without replacing its semantic nodes, and
can expose LCARS rail/title hitboxes as surface-control affordances for loading
or docking saved interfaces. Surface-control hitboxes also support first-pass
direct drag-release docking to left/right/top/bottom or a center free/undocked
overlay with recorded floating geometry; floating overlays also have a
lower-right resize hitbox that patches persisted floating width/height. It can
also paint more than one active docked
surface at once by merging each surface's terminal reservation, and surface
control hitboxes carry their interface id so menu-driven layout mutations target
the clicked surface. It can also draw a structural-console table bay with headers,
severity lanes, row/column overflow, and stable weighted geometry beyond four
columns, promotes table or
information-shape profiles such as `fleet_matrix`, `incident_summary`,
`queue_triage`, `project_status`, and `artifact_browser` into the structural path when
no explicit side/bottom layout overrides them, uses a first layout planner to
choose inline or stacked table/data detail bays and show explicit signal/action
overflow counters when geometry is tight, renders compact metric/table
provenance labels from first-class `UiNode.provenance` with legacy property
fallbacks, extracts first table row grouping/sort/focus/drilldown metadata,
maps visible child-row action ids to native hitboxes that update table focus
metadata through side-effect-free dispatch, supports `Ctrl+Alt+Shift` plus
Up/Down/Left/Right/PageUp/PageDown for keyboard table focus movement and
row-group jumps without external execution while persisting `focused_group`,
supports `Ctrl+Alt+Shift+Space` for focused-group mode with previous
focus-mode metadata preserved for restore,
supports `Ctrl+Alt+Shift+Enter` for safe focused-row action dispatch, renders
compact group-boundary summaries
for row/drill/focus/provenance/severity state, honors explicit `focused_group`
metadata in those summaries, can enrich those summaries from explicit
`role=cohort_summary` / `role=cohort` nodes targeting the table, renders first compact per-cell
severity/provenance markers from child row `cell_severity`/`cell_provenance`
metadata, projects focused structural table rows into a compact `ROW DETAIL`
readout with group/state/drilldown/provenance/cell-mark context,
supports an opt-in focused-cell `CELL FOCUS` readout and highlight through
`focused_column`/`focused_cell` style table properties plus
`Ctrl+Alt+Shift+<`/`>` and `Home/End` focused-column navigation and first
cell mouse hitboxes,
wraps long structural signal rows
within their reserved row budget instead of letting them run into adjacent
LCARS regions, gives structural chrome-marker labels black backing so rails do
not cut through text, suppresses the structural command bay when no actions are
declared, and reclaims that horizontal space for actionless status/table/detail
interfaces, uses compact/regular/wide structural breakpoints so command banks
do not steal narrow-window content space and wide windows give more width to
table/detail bays,
uses harder console bars/command paddles instead of generic rounded desktop
pills, and shows active `ACK` feedback for the most recently dispatched action.
The native endpoint also has the first live-builder surface: `edit_interface`
applies ordered add/replace/patch/remove/move/clear/bind/highlight operations
against the active interface, and the tools bridge exposes granular helpers for
step-by-step visual reconstruction with selectable feedback modes. `update_node`
remains the older single-node add/replace path and can bind one action in the
same transaction, plus `bind_action` for attaching a declared action to an
existing semantic node,
`patch_interface_layout` for dock/origin/orientation/reservation/visibility
changes without replacing semantic nodes/actions, `request_refresh` for
auditable refresh intent, `dispatch_action` external-execution intent recording
with explicit permission/confirmation flags for HTTP/MCP callers, renderer-owned
click dispatch for native local `open` actions, and structured `run.argv` actions
that native OWT may fork only when a root action allowlist entry matches,
`diff_interface` for semantic snapshot comparison,
and `patch_interface_lifecycle` for pin/hide/restore/expire metadata without
retiring the interface, including rendered `LIFECYCLE` badge projection for
visible pin/TTL state and `FRESHNESS` badge projection for stale
collected-at/refresh-interval metadata, plus `replace_interface` for swapping an active surface
to an inline or saved replacement document while preserving lifecycle state, and
inline inert `export_interface`/`import_interface` package JSON without
implicit execution. Validate/apply/load/import/replace and layout
preview/patch responses report `render_projection` so agents can see the
actual renderer path, structural profile, normalized placement, reservation
intent, and recorded spatial hints (`anchor`, `z_order`, `priority`,
`min_terminal_cells`, `collapse_policy`) plus floating overlay geometry and
information-shape contract metadata (`profile_family`, `table_density`,
cohort/state/severity keys, lifecycle controls, action roles, refresh policy,
drilldown policy, and non-default `palette_profile`) before a screenshot. Layout previews
also expose first-pass same-edge reservation conflict hints; `owt_status`
reports the same projection per interface; external action execution, pane control, archive pack file
I/O, automated screenshot capture, and full scene-graph native UI layouts are
not native yet.
When applying or loading a project/session LCARS interface, native OWT sets the
active tab title from explicit `tab_title`, then root `properties.tab_title`,
then a short final-segment fallback derived from the scope id. Status and
`list_interfaces` also expose a derived `owner` object from the same semantic
scope, so project/session/window ownership is visible without tying OWT to a
specific agent model or conversation id. The native LCARS surface menu should
reuse that owner metadata for target status, saved-profile grouping, and
saved-profile owner filtering rather than creating a separate ownership model.
Use `tab_title: "preserve"` for
interfaces that must not change the tab title.

## Working Branches

- `owt/baseline-20240203`: clean upstream baseline. Keep it clean.
- `owt/windows-gnu-bootstrap`: first native Windows GNU bootstrap branch.

Prefer narrow, reviewable commits and keep OWT changes isolated from unrelated
upstream churn.

## Build Notes

Current Windows GNU release build from WSL:

```sh
packaging/windows/build_and_install_owt.sh
```

That script builds `wezterm-gui.exe`, regenerates the stripped
`target/x86_64-pc-windows-gnu/release/OWT.exe` from it, installs the sibling
portable config, installs only when no OWT process is running, and verifies the
installed hash. If OWT is running, it stages `%LOCALAPPDATA%\OWT\OWT.next.exe`
and asks for a rerun after the window is closed.

Manual equivalent:

```sh
RUSTFLAGS='-L native=/tmp/owt-openssl-gnu-lib' \
TARGET_CXXFLAGS='-Wa,-mbig-obj' \
cargo +1.75.0 build \
  -p wezterm-gui \
  --target x86_64-pc-windows-gnu \
  --release \
  --features wezterm-ssh/vendored-openssl-ssh2
x86_64-w64-mingw32-strip --strip-unneeded \
  -o target/x86_64-pc-windows-gnu/release/OWT.exe \
  target/x86_64-pc-windows-gnu/release/wezterm-gui.exe
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
state, tool-name constants, and CLI helpers such as `owt-validate-interface`
only. GUI wiring, transport listeners, file I/O,
and agent/MCP startup loops belong in later integration layers so endpoint
startup stays deterministic.

The first GUI-side endpoint lives in `wezterm-gui/src/owt_native.rs`. It binds
a Windows-loopback HTTP endpoint, generates a per-process token, exports
`OWT_NATIVE_*` values before Lua config loads, and lets the portable Windows
config pass them into WSL. It currently supports status, semantic interface
validation/application state, side-effect-free declared action dispatch, and
local list/save/load for interface documents in the OWT profile store. Keep
semantic validation hostile to accidental disclosure: obvious
credential-sensitive visible text or property metadata must be rejected before
an interface reaches native runtime state. Missing keyboard fallback coverage is
a warning, not a hard rejection, until the renderer exposes complete explicit
action-slot control.
`wezterm-gui/src/termwindow/render/owt_lcars.rs` renders compact or
structural LCARS surfaces for active interface documents, registers button-node
hitboxes, merges reservations across multiple active docked surfaces, and the
status response reports `native_render_passes` so agents can verify the native
paint path ran. The renderer now has shared marker/button
primitives, structural frame/content-bay primitives, first structural-console
layout planning, table/information-shape structural promotion, row/column
overflow reporting, render_projection projection of profile contract metadata,
and active dispatch feedback, but it is still an early
native scene slice. Structural signal text now has bounded word wrapping plus
ellipsis overflow for the current top/left structural console path. The
planner now has a first viewport-breakpoint slice: compact widths keep the
command bank single-column and capped, while regular/wide widths allow larger
command/detail bays and two-column command banks when action count and space
justify it. Full layout fit scoring, focus/drilldown navigation, and richer
table/cohort semantics remain pending. `patch_interface_layout` is the first
native mutation path for moving, reserving, hiding, and showing/restoring a
surface while keeping the terminal-owned semantic document intact.
Secondary-click on a tab title opens the first terminal-owned LCARS surface
menu; those manual actions are backed by the same native layout patch path
rather than by an OS context menu or a regenerated fixture. That menu also
lists saved LCARS documents from the local OWT profile store and can load one
directly into the visible native runtime. When opened from LCARS chrome instead
of the tab title, the menu targets that clicked interface id; dock placement
must remain geometry-only and must not assign application roles by side. The
native LCARS window chrome path owns the OWT roundel, tab masthead, integrated
title controls, and left rail; keep the decoration rail geometry-only in that
paint phase rather than calling the terminal text renderer from chrome code.
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

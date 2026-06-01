# OWT Terminal Fork

This repository is Buanzo's native OWT terminal fork candidate, derived from
upstream WezTerm. It currently exists to turn the OWT prototype from
`/home/buanzo/git/tools/rust/owt` into a real terminal binary with stable
Windows app identity, LCARS/agentic-control direction, and eventually
terminal-owned UI/runtime state.

The central contract is `InterfaceDocument`, a renderer-agnostic operational UI
IR for state, grouping, facts, declared actions, capability/confirmation
requirements, and patchable updates. LCARS is the current native renderer and
visual language for that IR, not the document model itself. The renderer displays
documents and captures intent; execution authority remains with terminal-owned
approval and the future ActionBroker path.

The current fork baseline and local build notes are in `OWT_BASELINE.md`.
Repository-local agent instructions are in `AGENTS.md`.
The native MCP/control-plane direction is in `OWT_NATIVE_MCP.md`; the first
typed runtime/protocol scaffold lives in `owt-control/`, and `OWT.exe` now has
a local endpoint that `owt_current_bridge` can discover through inherited
`OWT_NATIVE_*` environment variables. The endpoint supports status plus
semantic interface validation/application state, first-pass action dispatch
state, per-interface render/owner status, layout preview/validation, active-surface
retirement, semantic refresh intent, semantic interface diffing, inline inert
interface-pack import/export, granular node/action binding, and recent runtime
events. `owt-control` also ships the side-effect-free
`owt-validate-interface` helper for offline validation of generated
`InterfaceDocument` JSON before a live endpoint is available or restarted. The
tools-repo bridge filters native-mode MCP tools to the working
native subset and exposes `apply_lcars_panel` as the high-level agent entrypoint
for simple semantic panels. The GUI paints active LCARS surfaces from
terminal-owned interface state, defaults to docked layout so terminal text
starts below the band, supports first reserved `left_rail`, `right_rail`, and
`bottom_strip` layouts, can render multiple active docked surfaces at once by
merging their terminal reservations, gives compact `action_strip` surfaces a
small button-strip reservation, reflows compact auxiliary buttons away from an
occupied reserved edge, and registers native hitboxes for button nodes
with interface-scoped ACK state so dispatch feedback does not leak across
surfaces. It also paints transcript-local LCARS chrome from explicit OWT 1701
Marker events and from opt-in `.lcars.md` LCARSMarkdown files without rewriting
scrollback text, visually suppressing recognized LCARSMarkdown control comments
after scan. LCARSMarkdown can mark fenced `lcars-dataview` blocks inline
without opening an overlay. Cat-able generated `.lcars` DataView files can use
chunked OSC 1701 payloads to open a sandboxed in-terminal table overlay with
local search, sort, and paging without entering MCP or trusted
`InterfaceDocument` state. Applying or loading a project/session interface also updates the active
tab title from `tab_title`, root `properties.tab_title`, or a short fallback
derived from the final segment of the scope id. Surface-control hitboxes carry
the clicked interface id into the native LCARS menu so manual dock/hide/show
mutations target the intended surface rather than relying on active-scope
ordering. Agents can call `preview_reflow`/`validate_layout` before
`patch_interface_layout` to inspect predicted properties and unsupported hints
without mutating runtime state; previews also report first-pass same-edge
reservation conflict hints, `layout_scene_plan`, `estimated_reserved_space`,
`overflow_estimate`, `viewport_fit` when viewport cell hints are supplied, and
a 0-100 `fit_score` with status/factors. Native validate/apply/load/import/replace,
layout preview, and layout patch responses include `render_projection` so
agents can see the actual renderer path, structural profile, normalized
placement, reservation intent, and recorded spatial hints such as `anchor`,
`z_order`, `priority`, `min_terminal_cells`, `collapse_policy`, and floating
overlay geometry for explicit overlays before relying on screenshots.
Persistent single-button controls are normalized to reserved placement unless an
explicit terminal-overlay opt-in is present. Compact button reservations consume
only the button-height band instead of the generic top-panel height, preserving
the shell viewport, and compact `action_strip` surfaces reserve only their
button strip instead of a full top panel. Projection now also preserves
information-shape contract metadata such as profile family, table density,
cohort/state/severity keys, lifecycle controls, action roles, refresh policy,
drilldown policy, and non-default LCARS palette profile so agents can verify workbench intent from native status
without rereading the source `.lcarsmd` file; `owt_status` also reports the
same projection per interface. `retire_interface` removes an active native
surface from the runtime. `request_refresh` records refresh intent for a
declared interface/action, including request provenance
(`source`/`request_source`/`requested_by`, `request_id`,
`requested_at_unix`). `dispatch_action` can also record an explicit
external-execution request with the same request provenance, but HTTP/MCP
callers cannot grant execution authority. Native renderer clicks are terminal UI
input: local `open` actions use the platform opener directly, and structured
`run.argv` actions may be forked only when the active document declares a root
action allowlist entry. Refresh requests preserve the selected action
label/kind/target/command metadata for allowlisted external runners.
`list_action_requests` returns the bounded recent queue of action-execution
intents and refresh requests so runners can inspect pending or approved work
without gaining native execution authority. `diff_interface` compares active/runtime,
saved, or inline semantic snapshots without mutating native state and now
returns `changed_facts` for metric/table/progress/data nodes whose operational
values changed.
`patch_interface_lifecycle` records pin/hide/show/restore/expire/TTL metadata
without retiring the semantic document; hidden or expired surfaces are omitted
from active rendering until restored.
`replace_interface` swaps an active target to an inline or saved replacement
document while preserving lifecycle state by default. `export_interface` emits
inert inline pack JSON with manifest hashes, permissions, changelog, and
optional inline sample/screenshot metadata; `import_interface` validates such a
pack and can save or explicitly apply it without executing actions or shell
commands. Import rejects permissions that attempt to grant external execution,
network, native-endpoint, or filesystem authority. `bind_action` attaches a declared action to an existing semantic node
without replacing the interface tree. LCARS surface-control hitboxes now support
first-pass direct drag docking: a click opens the surface menu, while a
drag-release patches the target surface toward left/right/top/bottom docking or
center free/undocked overlay with terminal-owned floating position and size
metadata.
Declared action buttons also act as surface handles: left-click dispatches the
action on release, left-drag moves/docks the owning surface, and right-click
opens the same surface menu so tiny one-button overlays are still controllable.
Drag now shows an LCARS drop-target preview, and single-button surfaces honor
left/right/top/bottom anchors when explicitly docked instead of staying pinned
to the right edge.
Floating overlays also expose a lower-right resize handle that patches
top-left anchored floating width/height without regenerating the semantic
document.
Native semantic validation rejects obvious credential-sensitive
visible text or property metadata before validate/apply/import/update flows
accept an interface, rejects unconfirmed
run/network/destructive/credential-sensitive actions before they enter runtime
state, and native responses now include soft
`validation_warnings` such as `keyboard_fallback_missing` when actionable
buttons are outside the current automatic keyboard slot coverage and
`action_allowlist_missing` when an external/shell-capable action declares a
target or command without a root action allowlist entry, and
`data_provenance_missing` when fact-bearing metric/table/progress/data nodes
lack source/collection metadata. Semantic nodes can carry
first-class fact provenance (`source`, `collected_at`, `host`, `command`,
`confidence`, `error`, and observed/inferred/stale/failed state), and the
native metric/table paths render compact provenance labels while preserving
legacy property fallbacks. Active documents with pin/TTL lifecycle metadata get
an ephemeral semantic `LIFECYCLE` badge at render time, and elapsed TTLs are
treated as expired for active render/status selection. Documents with root
`collected_at_unix` plus `refresh_interval_seconds` metadata get a visible
ephemeral `FRESHNESS` badge when their data is stale. Table rows now support
first grouping/sort/focus/drilldown metadata (`group_by`, `sort_by`,
`sort_direction`, `focused_row`, `focused_group`, child-row action ids, row
groups/focused flags/provenance) with compact native affordances. Visible
child-row action ids become native row hitboxes; safe dispatch records the
action and updates table focus metadata, including `focused_group`, without
executing external commands.
`Ctrl+Alt+Shift` plus Up/Down/Left/Right/PageUp/PageDown moves focused table
rows and row groups, persists `focused_group`, and does not execute commands;
`Ctrl+Alt+Shift+Space` toggles focused-group mode, filters the table view to
that cohort, and preserves prior table focus-mode metadata for restore;
`Ctrl+Alt+Shift+Enter` dispatches the currently focused row's declared action
through the safe native dispatch path.
Row group boundaries display compact summaries with row, drilldown, focus,
provenance, and severity signals for the full group; explicit `focused_group`
metadata marks a summary as focused, and focused-group mode filters visible
rows to that cohort. Explicit semantic `role=cohort_summary` /
`role=cohort` nodes can now enrich or override row-derived group summary
counts, severity, drilldown count, and source-table targeting.
The focused cohort also projects a compact `GROUP DETAIL` readout with row
count, drilldown/provenance density, severity class, cell-mark count, and
representative row keys.
Focused structural table rows also project a compact `ROW DETAIL` readout with
group, state, drilldown action id, key cells, provenance, and cell-level mark
counts before any external action is allowed to run. Focused rows with
cell-level marks now also project a compact `CELL DETAIL` readout that maps
those severity/provenance marks back to their table columns.
Tables can also declare a focused cell through `focused_column`,
`focus_column`, `selected_column`, `focused_cell`, `focus_cell`, or
`selected_cell`; the renderer highlights that cell and projects a compact
`CELL FOCUS` band with value, severity, provenance, group, row key, and
drilldown context. `Ctrl+Alt+Shift+<`/`>` moves the focused column
previous/next, and `Ctrl+Alt+Shift+Home/End` jumps first/last without changing
row or group focus. Visible non-empty cells also register native mouse hitboxes
that set `focused_column` directly without dispatching row actions.
Interfaces may request `palette_profile: bright_classic` for higher contrast
or `palette_profile: science_station` for muted analysis surfaces; aliases
such as `palette`, `lcars_palette`, and `muted_science` are normalized by the
native renderer.
Child row nodes can expose compact per-cell severity/provenance markers through
`cell_severity` and `cell_provenance`. The renderer now consumes the first
semantic node classes (`region`, `group`, `bar`, `elbow`, `badge`, `metric`,
`progress`, `button`, etc.) and provides `Ctrl+Alt+Shift+1..9` as a keyboard
fallback for the first nine visible action slots. Multi-surface renders now
derive a lightweight render-scene summary from active surface placement and show
a compact `SCENE PLAN`/`SCENE CONFLICT` badge when more than one surface is
visible or same-slot reserved surfaces collide. Before painting/status
projection, native OWT orders active scope surfaces by latest apply and reflows
compact auxiliary buttons, such as single render-folder controls, to a free edge
when the current scope already owns their requested reserved slot. External
action execution, pane control, archive pack file I/O, automated screenshot
capture, and full scene-graph layouts remain future native slices.
The endpoint stays on Windows loopback; the WSL bridge uses Windows interop if
direct loopback is unavailable.

The first installed native Windows build lives at:

```text
C:\Users\Usuario\AppData\Local\OWT\OWT.exe
```

It launches a WSL Ubuntu login shell in `/home/buanzo/git/tools` through the
portable config beside the executable. On Windows, `OWT.exe` auto-loads that
sibling `wezterm.lua` when no config file is supplied, which is the taskbar and
Explorer launch path. That installed profile enables the OWT LCARS custom
titlebar chrome: integrated Windows resize/buttons, a top-left OWT roundel,
LCARS tab slabs, and a wider geometry-only left rail with large sidebar slabs
that opens the native surface menu; its saved-profile view uses semantic owner
metadata for grouping labels and an `OWNER` filter control.
The private TheLCARS control-panel profile consumes only semantic metadata
derived from ignored local template CSS/theme scans; it does not embed template
payload in `OWT.exe`.
The current native binary has OWT version metadata, `assets/windows/owt.ico`,
and AppUserModelID `org.buanzo.owt`.

The upstream README follows for project provenance and user-facing WezTerm
references.

# Wez's Terminal

<img height="128" alt="WezTerm Icon" src="https://raw.githubusercontent.com/wez/wezterm/main/assets/icon/wezterm-icon.svg" align="left"> *A GPU-accelerated cross-platform terminal emulator and multiplexer written by <a href="https://github.com/wez">@wez</a> and implemented in <a href="https://www.rust-lang.org/">Rust</a>*

User facing docs and guide at: https://wezfurlong.org/wezterm/

![Screenshot](docs/screenshots/two.png)

*Screenshot of wezterm on macOS, running vim*

## Installation

https://wezfurlong.org/wezterm/installation

## Getting help

This is a spare time project, so please bear with me.  There are a couple of channels for support:

* You can use the [GitHub issue tracker](https://github.com/wez/wezterm/issues) to see if someone else has a similar issue, or to file a new one.
* Start or join a thread in our [GitHub Discussions](https://github.com/wez/wezterm/discussions); if you have general
  questions or want to chat with other wezterm users, you're welcome here!
* There is a [Matrix room via Element.io](https://app.element.io/#/room/#wezterm:matrix.org)
  for (potentially!) real time discussions.

The GitHub Discussions and Element/Gitter rooms are better suited for questions
than bug reports, but don't be afraid to use whichever you are most comfortable
using and we'll work it out.

## Supporting the Project

If you use and like WezTerm, please consider sponsoring it: your support helps
to cover the fees required to maintain the project and to validate the time
spent working on it!

[Read more about sponsoring](https://wezfurlong.org/wezterm/sponsor.html).

* [![Sponsor WezTerm](https://img.shields.io/github/sponsors/wez?label=Sponsor%20WezTerm&logo=github&style=for-the-badge)](https://github.com/sponsors/wez)
* [Patreon](https://patreon.com/WezFurlong)
* [Ko-Fi](https://ko-fi.com/wezfurlong)
* [Liberapay](https://liberapay.com/wez)

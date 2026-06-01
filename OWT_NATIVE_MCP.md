# OWT Native MCP Control Plane

## Decision

The OWT MCP/control plane should live inside `OWT.exe`.

The product should be understood as a terminal-native agentic control plane.
`InterfaceDocument` is the stable operational UI IR. LCARS is the current native
renderer and visual language for that IR, not the document model itself. The
renderer displays documents and captures user intent; ActionBroker-style
execution and terminal-owned approval remain the safety boundary.

The current tools-repo control path is a working prototype:

```text
Codex -> owt_current_bridge -> Unix socket -> owt_backend -> wezterm cli -> WezTerm window
```

That proved the interaction model, but it is not the final architecture. The
native terminal should expose its own local MCP/control endpoint so Codex and
other local agents can talk directly to the terminal process that owns panes,
tabs, runtime state, LCARS panels, actions, and UI events.

Transcript-local semantic hints use a separate narrow stream channel:
**OWT 1701 Markers** are carried by private `OSC 1701` sequences and can
annotate ordinary terminal output with small LCARS transcript events such as
sections, tasks, findings, sources, assessments, warnings, and errors. OSC is
an existing ANSI/VT Operating System Command escape family; OWT only defines the
private selector and payload convention. This does not replace the native
endpoint. Full interface documents, layout mutation, save/load lifecycle, and
action dispatch remain endpoint operations.

LCARSMarkdown is the no-escape companion for readable Markdown reports. A
`.lcars.md` file with `<!-- owt:lcars-md v=1 -->` can be printed with `cat`;
native OWT keeps the text unchanged and derives compact LCARS transcript chrome
from Markdown headings, task lists, blockquotes, admonitions, code fences,
fenced `lcars-dataview` blocks, rules, tables, and optional
`<!-- lcars: ... -->` hints. Recognized control comments are visually suppressed
after scanning; the scrollback text remains unchanged. Explicit OWT 1701
markers are still the right tool when a script needs exact event control.

LCARS DataView is the sandboxed cat-able table path. A generated `.lcars` file
can carry chunked `owt.dataview.*` OSC 1701 payloads and open a transient native
OWT overlay with local search, sort, and paging. Inline `lcars-dataview` fences
inside `.lcars.md` are safe Markdown content and do not auto-open that overlay.
DataView files have no MCP access, no native endpoint access, no command
execution, no registered actions, and no automatic promotion into trusted
`InterfaceDocument` state.

Native LCARS layout mutation is semantic. `patch_interface_layout` moves or
resizes an existing interface by changing placement properties while preserving
the document tree and actions; renderers are responsible for redistributing
rails, bays, command docks, and readout lanes for the new anchor. Visual
reference sites may be used as quality benchmarks only; OWT must not copy or
vendor third-party LCARS assets, CSS, HTML, JS, fonts, sounds, labels, or exact
layouts.

## Why

An embedded control plane fixes the structural problems seen with the first
native Windows `OWT.exe`:

- no stale WSLg backend can accidentally receive commands for the wrong window
- no Unix-socket-only backend blocks native Windows support
- no `wezterm cli` subprocess is needed for hot-path UI actions
- no external process has to infer pane/window identity from environment,
  `/proc`, TTYs, or mux sockets
- the terminal can directly expose its current window, workspace, panes,
  actions, and LCARS scene graph to agents

The sidecar model can remain useful for bootstrapping and compatibility, but
the product target is terminal-owned control.

## Target Shape

```text
Codex / local tool
  -> stdio MCP, named pipe, or loopback local endpoint
OWT.exe
  -> in-process OWT runtime
  -> panes / tabs / command palette / LCARS scene graph / status / actions
```

Recommended first native transport on Windows:

- named pipe scoped to the current user, or
- loopback TCP bound to `127.0.0.1` with a per-session token

Linux can keep a Unix socket transport, but the API should be platform-neutral.

## Implementation Seed

`owt-control/` is the first Rust scaffold for the embedded control plane. It is
deliberately side-effect free and currently contains:

- protocol version and native/prototype tool-name constants
- endpoint status types
- scoped interface document types
- semantic UI node and action types
- interface validation
- runtime state registration for applied interfaces
- side-effect-free `owt-validate-interface` CLI validation of generated
  `InterfaceDocument` JSON

It is not a transport, renderer, or GUI integration yet. That boundary is
intentional so the future MCP endpoint can start quickly and deterministically.

## First Endpoint Slice

The first embedded endpoint is intentionally narrow, but no longer status-only:

- `OWT.exe` starts a local HTTP status endpoint on `127.0.0.1:<ephemeral>`.
- It generates a per-process token using the OS random source.
- It exports `OWT_NATIVE_ENDPOINT`, `OWT_NATIVE_TOKEN`, `OWT_NATIVE_PROTOCOL`,
  and `OWT_NATIVE_PID` before Lua config loads.
- The Windows portable config forwards those variables into WSL through
  `WSLENV`.
- It writes `%LOCALAPPDATA%\\OWT\\native-endpoint.json` with the same endpoint
  metadata so WSL-side bridge/MCP processes that did not inherit fresh
  `OWT_NATIVE_*` variables can recover the current endpoint. The file includes
  the bearer token and is local runtime state only; diagnostics must redact it.
  On normal shutdown, `OWT.exe` removes the locator if it still matches the
  current process token.
- `owt_current_bridge` detects those variables and uses the native endpoint for
  `owt_status`, `build_interface`, `validate_interface`, `apply_interface`,
  `dispatch_action`, `list_interfaces`, `save_interface`, `load_interface`,
  `replace_interface`, `export_interface`, `import_interface`, `edit_interface`,
  `start_interface_build`, `update_node`, `add_interface_node`,
  `replace_interface_node`, `patch_interface_node`, `remove_interface_node`,
  `move_interface_node`, `clear_interface_children`, `highlight_interface_node`,
  `set_interface_feedback_mode`, `bind_action`, `request_refresh`, `list_action_requests`, `diff_interface`,
  `preview_reflow`, `validate_layout`, `patch_interface_layout`, and
  `patch_interface_lifecycle`, and `retire_interface`. In native endpoint mode the bridge advertises only
  the native-capable MCP tools and adds
  `apply_lcars_panel` as a high-level helper that builds a simple semantic
  panel and sends it through `apply_interface`.
  The bridge-side builder accepts root `profile`, `information_shape`,
  `visual_density`, and scalar `properties` metadata so specialized native
  renderers such as `thelcars_control_panel` can receive attribution,
  license-use, and local-template metrics without copying private template
  payloads into the interface. The TheLCARS showcase renderer keeps semantics
  in those nodes while adding subtle LCARS-native depth: raised/pressed command
  slabs, recessed black bays, bevel edges, and builder-highlight glow.
- Applying or loading a project/session interface also updates the active tab
  title. Explicit document `tab_title` wins, root `properties.tab_title` is the
  legacy-compatible override, and otherwise OWT derives a short label from the
  final segment of the scope id. `tab_title: "preserve"` leaves the existing
  tab title untouched.
- Status is interface-aware: `owt_status` reports per-interface render passes,
  interface-scoped last-dispatched actions, per-interface `owner` metadata
  derived from semantic project/session/window scope, per-interface
  `render_projection`, the first-pass `layout_scene_plan` reservation map, and
  recent runtime events. The
  top-level `native_rendering_ready` flag means at least one active interface
  has actually reached the native LCARS render path, not merely that JSON was
  accepted.
- `list_interfaces` returns runtime interface entries with `owner` plus
  `owner_groups` summaries, so agents can group project/session/window panels
  without inventing conversation-specific ownership state.
- Native semantic validation rejects obvious credential-sensitive visible text
  or property metadata before validate/apply/load/import/replace/edit/update flows
  accept an interface document, and rejects unconfirmed
  run/network/destructive/credential-sensitive actions before they enter runtime
  state. Native validate/apply/load/import/replace/edit/update responses also include
  soft `validation_warnings`; `keyboard_fallback_missing`
  means an actionable button is not covered by the current automatic
  `Ctrl+Alt+Shift+1..9` action-slot range and has no explicit shortcut metadata.
  `action_allowlist_missing` means a command-bearing action, or a
  run/network/refresh/destructive/credential-sensitive action target, needs a
  root allowlist property such as `allowed_action_targets` before agents treat
  the interface as permission-ready. `data_provenance_missing` means a
  fact-bearing metric/table/progress/data node lacks at least source and
  collection-time provenance, inherited from an ancestor or declared directly.
- The terminal-owned LCARS surface menu uses the same owner metadata for its
  first owner-aware pass: target status can show the target owner, saved
  profiles sort by owner, and saved entry details include owner plus interface
  id; the saved-profile view exposes an `OWNER` control that filters between
  all saved profiles and each semantic owner group. Manual dock/move commands
  must emit complete visible layout intent, including normalized
  `layout`/`placement`/`reservation` fields, so the menu path stays equivalent
  to native bridge `patch_interface_layout` calls.
- Validate/apply/load/import/replace, `preview_reflow`/`validate_layout`, and
  `patch_interface_layout` responses include `render_projection`: visible
  state, renderer path, normalized layout/origin/reservation/orientation,
  terminal reservation intent, first-pass spatial hints (`anchor`, `z_order`,
  `priority`, `min_terminal_cells`, `collapse_policy`), floating overlay
  geometry, structural profile, requested profile hint, and non-default LCARS
  palette profile. Layout previews
  also include first-pass same-edge conflict hints, a `layout_scene_plan` over
  active surfaces, reserved-space estimates, viewport-fit estimates when
  `viewport_columns`/`viewport_rows` are supplied, automatic action-slot
  assignment with overflow action ids, table overflow estimates, and a fit
  score.
  reservation `conflict_hints`. This lets agents detect
  accepted-but-normalized layout/profile choices before asking for visual
  proof.
- The endpoint accepts:
  - `GET /owt/status`
  - `POST /owt/validate_interface`
  - `POST /owt/apply_interface`
  - `POST /owt/dispatch_action`
  - `GET /owt/interfaces`
  - `POST /owt/save_interface`
  - `POST /owt/load_interface`
  - `POST /owt/replace_interface`
  - `POST /owt/export_interface`
  - `POST /owt/import_interface`
  - `POST /owt/edit_interface`
  - `POST /owt/update_node`
  - `POST /owt/bind_action`
  - `POST /owt/request_refresh`
  - `GET|POST /owt/action_requests`
  - `GET|POST /owt/list_action_requests`
  - `POST /owt/diff_interface`
  - `POST /owt/compare_snapshot`
  - `POST /owt/preview_reflow`
  - `POST /owt/validate_layout`
  - `POST /owt/patch_interface_layout`
  - `POST /owt/patch_interface_lifecycle`
  - `POST /owt/retire_interface`
  - `GET|POST /owt/runtime`
- When a WSL process cannot connect to Windows loopback directly, the bridge
  uses Windows interop to query the same `127.0.0.1` endpoint from the Windows
  side. Do not widen the listener to `0.0.0.0` for this path; that causes
  Windows Firewall Public-network prompts and is broader than the status slice
  requires.
- Secondary-clicking a native tab title opens the terminal-owned LCARS surface
  menu. The menu uses operator-facing terms: hide, show/restore, load saved
  LCARS, dock/reserve, and move to overlay. Drag-release to the center records a
  first-pass free/undocked overlay position and size, rendered as a floating
  in-window widget rather than a separate OS window. Floating overlays expose a
  lower-right resize handle that patches top-left anchored width/height through
  the same semantic layout-property path. It does not expose a separate
  activate/deactivate command; show/restore sets the required runtime
  visibility/enabled flags internally. LCARS surfaces also register
  low-priority surface-control hitboxes and action paddles render hover feedback
  as a physical state change rather than textual log output.

This proves the route `Codex in WSL -> owt_current_bridge -> native OWT.exe`.
It also proves that semantic interface documents can be validated and recorded
inside the terminal-owned runtime. The current GUI integration also paints a
compact static LCARS control band from active `InterfaceDocument` state and
reports `native_render_passes` after the paint path runs. It can now paint
multiple active docked LCARS surfaces in the same window by merging each
surface's reserved terminal space before drawing. First-pass LCARS
button hitboxes now route declared action ids into native dispatch state, and
`Ctrl+Alt+Shift+1..9` dispatches the first native action
slots after normal configured keybindings fail. Documents with more actionable
  buttons than covered automatic slots report `keyboard_fallback_missing` warnings
  until they declare explicit shortcut metadata. `request_refresh` and
  `dispatch_action` both preserve request provenance (`source`/`request_source`/
  `requested_by`, `request_id`, `requested_at_unix`) in native runtime audit
  state, and refresh requests include the selected action label/kind/target/
  command metadata for downstream allowlist checks. HTTP/MCP caller permission
  flags are not authority: external-execution requests remain terminal-owned
  approval records. Renderer clicks are terminal UI input; local `open` actions
  use the platform opener directly, and `run.argv` actions fork only with a
  matching root action allowlist entry. `list_action_requests` exposes the
  bounded recent queue of action-execution intents and refresh requests,
  optionally filtered by interface/scope, so an external runner can inspect
  pending or approved work without gaining native OWT command execution.
  Multi-surface rendering also derives a lightweight
render-scene summary from active placement and paints a compact `SCENE PLAN` or
`SCENE CONFLICT` badge when multiple surfaces are visible or same-slot reserved
surfaces collide. The endpoint also has the
first local interface lifecycle slice: list runtime/saved interfaces, save the
active or supplied interface document into the local OWT profile store, and
load a saved interface by id. It can preview layout mutations without changing
runtime state, report first-pass reserved terminal-space estimate and layout fit
score, report first-pass overflow/action-slot counts plus projected renderer
placement/profile, patch layout intent in place, and retire an active native
surface with optional profile-store deletion by sanitized store id. It can also
  record semantic refresh intent, including terminal-owned approval state for
  external execution, without running external commands implicitly. It can compare
two semantic interface snapshots from runtime ids, saved profile-store ids, or
inline documents through `diff_interface`/`compare_snapshot` without mutating
runtime state. It can patch lifecycle state through
`patch_interface_lifecycle`, including pin/unpin, hide/show,
restore_previous, expire_now, clear_expiration, and TTL/expiry metadata without
retiring the semantic document. It can replace an active target with an inline
or saved semantic document through `replace_interface` while preserving
lifecycle state by default. Active render snapshots now project visible pin/TTL
lifecycle metadata into ephemeral root properties plus a semantic `LIFECYCLE`
badge, and elapsed TTLs are treated as expired for active render/status
selection without deleting the runtime document. Active render snapshots also
project root `collected_at_unix` plus `refresh_interval_seconds` metadata into
an ephemeral `FRESHNESS` badge when the data is stale. It can export active, runtime,
saved, or inline semantic documents as inert inline interface-pack JSON with
manifest hashes, permissions, changelog, and optional inline sample/screenshot
metadata. It can also import such a pack, saving by default and applying only
when explicitly requested; import does not execute declared actions or shell
commands and rejects permission metadata that attempts to grant external
execution, network, native-endpoint, or filesystem authority. It does not yet expose pane control, external
action execution, archive pack file I/O, automated screenshot capture, or full rich layout
profiles. The first structural profile, `structural_console`, can render
semantic `frame`, `side_rail`, `bar_run`, `content_bay`, `data_cascade`,
`table`, and `command_grid` nodes with top+left terminal reflow. `table` nodes
now get a dedicated LCARS table bay when space allows: columns come from
`properties.columns`/`properties.headers`, rows come from newline/semicolon
text or `properties.rows`, and row severity can be inferred from visible cell
text or supplied through child row properties. Tables and metric rows can draw
compact provenance labels from first-class `UiNode.provenance`, falling back to
legacy `properties.provenance`/`properties.source` and `collected_at` fields
when older documents are loaded. Table rows also have first semantic metadata
for grouping, sorting, focus, drilldown, and row-level provenance:
`group_by`, `sort_by`, `sort_direction`, `focused_row`, `focused_group`,
child-row action ids, row `group`, focused flags, and child row provenance.
Visible child-row action ids are also exposed as native row hitboxes; dispatch
is still side-effect-free and updates table focus metadata, including
`focused_group`, rather than executing external commands.
Keyboard row focus uses `Ctrl+Alt+Shift+Up/Down/Left/Right/PageUp/PageDown`
and updates runtime metadata only; Left/Right jump row groups, all row movement
persists `focused_group`, Space toggles focused-group mode with previous
table focus-mode metadata preserved for restore, and Enter dispatches the
focused row action when declared. Group boundary labels summarize
row, drilldown, focus, provenance, and severity state, with explicit
`focused_group` marking a summary as focused; focused-group mode filters
visible rows to that cohort. Explicit semantic `role=cohort_summary` /
`role=cohort` group nodes can target a table through `cohort_summary_node` or
`source_table` and enrich/override row-derived group counts, severity, and
drilldown counts. Child row nodes can also expose
first compact per-cell severity/provenance markers with `cell_severity` and `cell_provenance`
properties.
The focused cohort renders a compact `GROUP DETAIL` readout with row count,
drilldown/provenance density, severity class, cell-marker count, and
representative row keys.
Focused structural table rows render a compact `ROW DETAIL` readout with group,
state, drilldown action id, key cells, provenance, and cell-level mark counts
before any external action execution exists. Focused rows with cell-level marks
also render a compact `CELL DETAIL` readout that names the marked columns and
their severity/provenance values.
Tables can opt into a focused-cell target with `focused_column`,
`focus_column`, `selected_column`, `focused_cell`, `focus_cell`, or
`selected_cell`; native rendering highlights that cell and adds a compact
`CELL FOCUS` detail band with the focused cell value, severity, provenance,
group, row key, and drilldown context. `Ctrl+Alt+Shift+<`/`>` moves focused
columns previous/next, and `Ctrl+Alt+Shift+Home/End` jumps first/last while
canonicalizing focus aliases into `focused_column`. Visible non-empty table
cells also register mouse hitboxes that focus the clicked column without
dispatching row actions or requesting external execution.
Table-only documents and
information-shape profiles such as `fleet_matrix`, `queue_triage`,
`incident_summary`, `project_status`, and `artifact_browser` now promote to the
structural renderer when no explicit side/bottom layout overrides them. Table
profile contracts are also projected through native `render_projection`:
profile family, table density, cohort/state/severity keys, lifecycle controls,
action roles, refresh policy, and drilldown policy are visible in
validate/apply/load/import/replace/layout responses and per-interface
`owt_status` entries when the semantic document declares them. Table
geometry supports more than four columns without spilling past the bay and
reports hidden row/column overflow. The structural renderer also has the first
layout-planner slice: it sizes the command grid, chooses inline vs stacked
table/data-cascade detail bays from available geometry, and shows explicit
hidden signal/action counters instead of silently overpainting. Long structural
signal rows are now bounded to a small wrapped row budget with ellipsis
overflow, which reduces the worst text-crowding failure without pretending
that the full layout engine exists yet. Actionless structural interfaces no
longer reserve or paint the command grid; the planner hides that bay and gives
the width back to status/table/detail content. The same planner now has a first
compact/regular/wide breakpoint pass: narrow windows cap the command bank and
force one action column so the content bay remains readable, while wide windows
can spend more width on detail/table bays and use a two-column command bank.
The native renderer now has a `block_composition`/`semantic_blocks` source path
for real agent-built LCARS UI requests: labelled data blocks, multiple data
boxes, a graphics viewport, and local command controls. This keeps the protocol
semantic while OWT owns the LCARS chrome and later docking/reflow decisions.

## Runtime Ownership

`OWT.exe` should own:

- pane, tab, window, workspace identity
- OWT runtime state store
- status items
- registered actions
- notifications
- LCARS/`owt.ui` panels and scene graph
- hit testing and keyboard fallback for panel actions
- import/export of interface packages

External tools should publish semantic intent, not draw raw pixels or type into
the user's active prompt.

## Intent Boundary

Human phrasing is not part of the terminal protocol. The operator may say
`switch to`, `pasate para`, `open the`, or any other natural-language variant.
Codex or another local agent resolves that language into a structured project
scope and an interface contract by reading the relevant `AGENTS.md`,
`sub_agents`, and project-local `LCARS.md` files.

`OWT.exe` should receive the resolved operation, for example:

- active scope: project/root/path/workspace identity
- interface package or `owt.ui` document to validate/apply
- requested status/actions/panels
- declared action permissions

The native endpoint should not depend on hard-coded human command phrases.

## No Fake LCARS

Native LCARS is not implemented by opening a shell tab, printing a dashboard,
or running a renderer from WSL. Those can be diagnostics, but they do not count
as applying a project interface inside native OWT.

For any resolved project-scope intent to count as native LCARS:

1. The request must reach `OWT.exe` through its embedded local endpoint or a
   bridge connected to that endpoint.
2. The terminal-owned runtime must record the active interface package,
   status/actions, and any visible surfaces.
3. The visible surface or surfaces must be owned by the terminal runtime or by an explicitly
   supported native renderer.
4. If the endpoint or renderer is unavailable, the correct result is an
   explicit unsupported/native-window-only status, not a simulated panel.

The current native Windows build now satisfies the first visible static slice
for `validate_interface`/`apply_interface` and the bridge's
`apply_lcars_panel`: the request reaches `OWT.exe`, the terminal-owned runtime
records the applied interface, and the native GUI paint path renders a compact
LCARS control band for active interface state. Native layout defaults to docked,
so terminal content renders below the LCARS band; overlay remains an explicit
HUD-style mode through semantic node properties. Treat this as the
static-rendering milestone, not the complete agentic UI system. The native
renderer now has first reserved layout regions beyond the default top band:
`right_rail` and `bottom_strip` reflow terminal space around side/bottom LCARS
surfaces, while `overlay` remains explicit HUD mode. It also recognizes
`compact_top`, `right_browser`, and `alert_strip` profile intent aliases for
the current static renderer, plus `structural_console` for the first native
top+left LCARS frame/content-bay mode and `docked_surface` for the current
low-noise public/default docked composition. The older `sleek_console` spelling
is a legacy alias, not a product architecture. It now consumes the first semantic node
classes directly (`region`, `group`, `frame`, `side_rail`, `content_bay`,
`bar`, `bar_run`, `elbow`, `data_cascade`, `command_grid`, `badge`, `metric`,
`progress`, `table`, `button`, etc.) instead of treating all nodes as plain summary text. The
current top/right/bottom surfaces share LCARS marker and action-button
primitives: marker chrome stays out from under operational text, buttons have
notch/rule affordance, hovered buttons shift cap/rule/text state, and the most
recently dispatched action is highlighted as an `ACK` state. Multiple active
interfaces with distinct scopes can coexist as native docked surfaces, so a
left rail and a right rail can both be applied without regenerating a single
combined document.

## Interface Package Lifecycle

OWT should treat agent-created UI as durable interface packages, not one-off
draw commands. A package should include:

- metadata: id, title, version, source, scope, timestamps
- declared permissions and action categories
- semantic UI nodes: panel, region, group, frame, side rail, content bay, text,
  bar, bar run, elbow, data cascade, command grid, button, badge, list, table,
  metric, progress, image, spacer
- action bindings and keyboard fallbacks
- theme/profile selection
- import/export representation
- optional screenshots or preview artifacts
- changelog/provenance for agent-authored updates
- fact provenance on semantic nodes: `source`, `collected_at`, `host`,
  `command`, `confidence`, `error`, and observed/inferred/stale/failed state

The native lifecycle should support:

- build
- validate
- apply
- save
- load
- update node
- bind action
- export
- import
- diff
- retire

## MCP Surface

Initial native tools should mirror the useful prototype names:

- `owt_status`
- `owt_expected_contract`
- `interface.describe_schema`
- `validate_interface`
- `apply_interface`
- `apply_lcars_panel`
- `dispatch_action`
- `get_active_context`
- `list_panes`
- `focus_pane`
- `resize_pane`
- `split_pane`
- `send_input_to_pane`
- `spawn_tab`
- `set_tab_title`
- `get_ui_state`
- `set_status_bar`
- `clear_status_bar`
- `notify_ui`
- `register_ui_action`
- `clear_ui_action`
- `attach_resource_view`
- `toggle_resource_view`
- `clear_resource_view`
- `attach_folder_view`
- `toggle_folder_view`
- `clear_folder_view`
- `attach_lcars_panel`
- `clear_lcars_panel`

Current native endpoint mode intentionally advertises only the implemented
subset: `owt_status`, `owt_expected_contract`, `interface.describe_schema`, `build_interface`,
`validate_interface`, `apply_interface`, `apply_lcars_panel`,
`dispatch_action`, `list_interfaces`, `save_interface`, `load_interface`,
`replace_interface`, `export_interface`, `import_interface`, `edit_interface`,
`start_interface_build`, `update_node`,
`add_interface_node`, `replace_interface_node`, `patch_interface_node`,
`remove_interface_node`, `move_interface_node`, `clear_interface_children`,
`highlight_interface_node`, `set_interface_feedback_mode`, `bind_action`, `request_refresh`,
`list_action_requests`, `diff_interface`, `preview_reflow`,
`validate_layout`, `patch_interface_layout`, `patch_interface_lifecycle`, and
`retire_interface`.
`diff_interface`/`compare_snapshot` reports action, node, root-property, and
fact-level `changed_facts` changes without mutating runtime state.
Prototype-only tools should remain hidden in native
mode until they are truly backed by `OWT.exe`.

## Migration Plan

1. Keep `/home/buanzo/git/tools/rust/owt` as the prototype/spec source.
2. Define a platform-neutral OWT control API inside this fork.
3. Add an embedded endpoint in `OWT.exe`.
4. Make `owt_current_bridge` detect native OWT and connect to the embedded
   endpoint instead of the Unix-socket sidecar.
5. Move LCARS panel state and action dispatch into the terminal runtime.
6. Retire the sidecar backend for daily use once native status/actions/panels
   are verified.

## Compatibility Rule

Until external action execution, pane control, archive pack file I/O, and
automated screenshot capture exist, do not claim that native `OWT.exe` provides the full prototype
LCARS/control surface. The native endpoint now proves process reachability,
applied interface state, compact and first structural OWT-owned LCARS render passes, native
table-bay rendering for semantic operational matrices, table/information-shape
promotion into the structural path, information-shape contract projection in
`render_projection`, row/column overflow reporting,
action-aware command-bay suppression for actionless structural panels,
breakpoint-aware command/detail sizing for compact, regular, and wide
structural consoles,
high-level panel application through the bridge, local save/list/load for
interface documents in the OWT profile store, `edit_interface` plus granular
bridge helpers for live step-by-step node construction/removal/movement/
highlighting with selectable feedback modes, `update_node` for the older atomic
single-node construction path, `bind_action` for attaching a declared action to
an existing semantic node without replacing the interface tree,
`patch_interface_layout` for dock/origin/orientation/
reservation/visibility mutation without replacing semantic nodes/actions,
targeted surface-control hitboxes that pass the clicked interface id into those
layout mutations, and
side-effect-free action dispatch state for
declared button ids, including keyboard fallback for the first nine rendered
action slots, with validation warnings for uncovered actionable buttons.
It also supports inline inert `export_interface`/`import_interface` package
JSON without implicit execution. External action execution, pane control,
archive pack file I/O, automated screenshot capture, and full scene-graph
native UI layouts are not native yet.

# OWT Native MCP Control Plane

## Decision

The OWT MCP/control plane should live inside `OWT.exe`.

The current tools-repo control path is a working prototype:

```text
Codex -> owt_current_bridge -> Unix socket -> owt_backend -> wezterm cli -> WezTerm window
```

That proved the interaction model, but it is not the final architecture. The
native terminal should expose its own local MCP/control endpoint so Codex and
other local agents can talk directly to the terminal process that owns panes,
tabs, runtime state, LCARS panels, actions, and UI events.

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
  `owt_status`, `validate_interface`, `apply_interface`, `dispatch_action`,
  `list_interfaces`, `save_interface`, `load_interface`, and `update_node`. In native endpoint
  mode the bridge advertises only the native-capable MCP tools and adds
  `apply_lcars_panel` as a high-level helper that builds a simple semantic
  panel and sends it through `apply_interface`.
- The endpoint accepts:
  - `GET /owt/status`
  - `POST /owt/validate_interface`
  - `POST /owt/apply_interface`
  - `POST /owt/dispatch_action`
  - `GET /owt/interfaces`
  - `POST /owt/save_interface`
  - `POST /owt/load_interface`
  - `POST /owt/update_node`
  - `GET|POST /owt/runtime`
- When a WSL process cannot connect to Windows loopback directly, the bridge
  uses Windows interop to query the same `127.0.0.1` endpoint from the Windows
  side. Do not widen the listener to `0.0.0.0` for this path; that causes
  Windows Firewall Public-network prompts and is broader than the status slice
  requires.

This proves the route `Codex in WSL -> owt_current_bridge -> native OWT.exe`.
It also proves that semantic interface documents can be validated and recorded
inside the terminal-owned runtime. The current GUI integration also paints a
compact static LCARS control band from the active `InterfaceDocument` and
reports `native_render_passes` after the paint path runs. First-pass LCARS
button hitboxes now route declared action ids into side-effect-free native
dispatch state, and `Ctrl+Alt+Shift+1..9` dispatches the first native action
slots after normal configured keybindings fail. The endpoint also has the
first local interface lifecycle slice: list runtime/saved interfaces, save the
active or supplied interface document into the local OWT profile store, and
load a saved interface by id. It does not yet expose pane control, external
action execution, arbitrary import/export packages, or full rich layout
profiles. The first structural profile, `structural_console`, can render
semantic `frame`, `side_rail`, `bar_run`, `content_bay`, `data_cascade`,
`table`, and `command_grid` nodes with top+left terminal reflow. `table` nodes
now get a dedicated LCARS table bay when space allows: columns come from
`properties.columns`/`properties.headers`, rows come from newline/semicolon
text or `properties.rows`, and row severity can be inferred from visible cell
text or supplied through child row properties. Table-only documents and
information-shape profiles such as `fleet_matrix`, `queue_triage`,
`incident_summary`, `project_status`, and `artifact_browser` now promote to the
structural renderer when no explicit side/bottom layout overrides them. Table
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
3. The visible surface must be owned by the terminal runtime or by an explicitly
   supported native renderer.
4. If the endpoint or renderer is unavailable, the correct result is an
   explicit unsupported/native-window-only status, not a simulated panel.

The current native Windows build now satisfies the first visible static slice
for `validate_interface`/`apply_interface` and the bridge's
`apply_lcars_panel`: the request reaches `OWT.exe`, the terminal-owned runtime
records the applied interface, and the native GUI paint path renders a compact
LCARS control band for the active interface. Native layout defaults to docked,
so terminal content renders below the LCARS band; overlay remains an explicit
HUD-style mode through semantic node properties. Treat this as the
static-rendering milestone, not the complete agentic UI system. The native
renderer now has first reserved layout regions beyond the default top band:
`right_rail` and `bottom_strip` reflow terminal space around side/bottom LCARS
surfaces, while `overlay` remains explicit HUD mode. It also recognizes
`compact_top`, `right_browser`, and `alert_strip` profile intent aliases for
the current static renderer, plus `structural_console` for the first native
top+left LCARS frame/content-bay mode. It now consumes the first semantic node
classes directly (`region`, `group`, `frame`, `side_rail`, `content_bay`,
`bar`, `bar_run`, `elbow`, `data_cascade`, `command_grid`, `badge`, `metric`,
`progress`, `table`, `button`, etc.) instead of treating all nodes as plain summary text. The
current top/right/bottom surfaces share LCARS marker and action-button
primitives: marker chrome stays out from under operational text, buttons have
notch/rule affordance, and the most recently dispatched action is highlighted
as an `ACK` state.

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
subset: `owt_status`, `owt_expected_contract`, `validate_interface`,
`apply_interface`, `apply_lcars_panel`, `dispatch_action`, `list_interfaces`,
`save_interface`, `load_interface`, and `update_node`. Prototype-only tools should remain
hidden in native mode until they are truly backed by `OWT.exe`.

Then add native-only lifecycle tools:

- `build_interface`
- `bind_action`
- `export_interface`
- `import_interface`
- `diff_interface`
- `retire_interface`

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

Until external action execution, pane control, and full package lifecycle
support exist, do not claim that native `OWT.exe` provides the full prototype
LCARS/control surface. The native endpoint now proves process reachability,
applied interface state, compact and first structural OWT-owned LCARS render passes, native
table-bay rendering for semantic operational matrices, table/information-shape
promotion into the structural path, row/column overflow reporting,
action-aware command-bay suppression for actionless structural panels,
breakpoint-aware command/detail sizing for compact, regular, and wide
structural consoles,
high-level panel application through the bridge, local save/list/load for
interface documents in the OWT profile store, `update_node` for atomic
node-level construction, and side-effect-free action dispatch state for
declared button ids, including keyboard fallback for the first nine rendered
action slots.

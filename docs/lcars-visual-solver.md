# LCARS Visual Solver

OWT treats LCARS documents as semantic interface requests, not coordinate maps.
The renderer owns the final visual solution for each viewport.

## Model

- A surface may request a dock edge: top, left, right, bottom, corner, or overlay.
- Docking is an anchor preference. It does not by itself mean the terminal must be resized.
- Reservation is a separate outcome. OWT should reserve terminal space only when a surface would otherwise make the shell unreadable, unclickable, or materially ambiguous.
- Project `LCARS.md` files should describe intent: surface role, action semantics, priority, collapse policy, and persistence behavior.
- The renderer resolves size, label fitting, final edge, overflow, and scene conflicts.

## Presentation Tiers

- `floating_overlay`: a HUD or widget over the terminal.
- `dock_overlay`: an edge-anchored surface that does not reserve terminal space.
- `dock_reserved`: an edge-anchored surface that reserves terminal space.
- `compact_action`: a small shortcut or folder opener.
- `action_group`: a strip or rail of related actions.
- `structural_surface`: a larger panel with tables, state, or multiple content bays.
- `hidden`: a collapsed or invisible surface.

## Reflow Priorities

1. Preserve shell legibility.
2. Keep action labels readable before preserving decorative proportions.
3. Reflow auxiliary surfaces before primary project surfaces.
4. Collapse diagnostics such as scene badges before moving project controls.
5. Reserve terminal space only when overlay placement would obscure meaningful terminal content or leave controls unusable.

## Expected Signals

Native projection and scene-plan responses should expose enough information for agents and tests to understand the solver:

- requested edge and solved edge
- presentation tier
- whether terminal space was actually reserved
- label-fit state
- collapse or reflow reason
- occlusion risk
- whether placement is stable from the previous semantic request

Agents should use these signals instead of guessing exact coordinates.

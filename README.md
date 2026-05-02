# OWT Terminal Fork

This repository is Buanzo's native OWT terminal fork candidate, derived from
upstream WezTerm. It currently exists to turn the OWT prototype from
`/home/buanzo/git/tools/rust/owt` into a real terminal binary with stable
Windows app identity, LCARS/agentic-control direction, and eventually
terminal-owned UI/runtime state.

The current fork baseline and local build notes are in `OWT_BASELINE.md`.
Repository-local agent instructions are in `AGENTS.md`.
The native MCP/control-plane direction is in `OWT_NATIVE_MCP.md`; the first
typed runtime/protocol scaffold lives in `owt-control/`, and `OWT.exe` now has
a local endpoint that `owt_current_bridge` can discover through inherited
`OWT_NATIVE_*` environment variables. The endpoint supports status plus
semantic interface validation/application state and first-pass action dispatch
state. The tools-repo bridge filters native-mode MCP tools to the working
native subset and exposes `apply_lcars_panel` as the high-level agent entrypoint
for simple semantic panels. The GUI paints a compact LCARS control band from
the active interface, defaults to docked layout so terminal text starts below
the band, supports first reserved `right_rail` and `bottom_strip` layouts, and
registers native hitboxes for button nodes. The renderer now consumes the first
semantic node classes (`region`, `group`, `bar`, `elbow`, `badge`, `metric`,
`progress`, `button`, etc.) and provides `Ctrl+Alt+Shift+1..9` as a keyboard
fallback for the first visible action slots. External action execution, pane
control, package persistence, and full scene-graph layouts remain future native
slices.
The endpoint stays on Windows loopback; the WSL bridge uses Windows interop if
direct loopback is unavailable.

The first installed native Windows build lives at:

```text
C:\Users\Usuario\AppData\Local\OWT\OWT.exe
```

It launches a WSL Ubuntu login shell in `/home/buanzo/git/tools` through the
portable config beside the executable. On Windows, `OWT.exe` auto-loads that
sibling `wezterm.lua` when no config file is supplied, which is the taskbar and
Explorer launch path. The current native binary has OWT version metadata,
`assets/windows/owt.ico`, and AppUserModelID `org.buanzo.owt`.

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

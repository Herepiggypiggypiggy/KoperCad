# Visual test user

Tests that only read code miss what a first-time user sees. The visual test
user lets an AI assistant (or a script) use the real KoperCAD window the way a
person does: look at it, read its labels, click, drag, scroll and type.

Where to find it: `kopercad --control` (app), `kopercad-cli test-user` (MCP
server), `scripts/test-user-mcp.sh` (launcher, also what `.mcp.json` runs).

For scripted runs, screenshots at fixed sizes and audit reports see `docs/testing/qa-suite.md`
(`scripts/qa.sh`), which is built on this harness.

## Parts

1. **Control mode** (`crates/novo-app/src/control.rs`). `kopercad --control[=PORT]`
   opens the normal window and listens on `127.0.0.1` (loopback only, never a
   network address). One JSON request per line in, one reply per line out. Input
   is injected through egui's own input path, so it is the same code a real
   mouse and keyboard reach, on every platform. Screenshots are egui's
   viewport screenshot (what the GPU drew, PNG). `ui_tree` is egui's AccessKit
   tree: role, label, value, enabled and checked state, rect.
2. **MCP server** (`crates/novo-cli/src/testuser.rs`). `kopercad-cli test-user`
   speaks MCP over stdio. It starts the app with an empty profile (the
   first-run look), and on Linux without a display it starts its own Xvfb,
   openbox and software Vulkan (lavapipe) if installed.
3. **`.mcp.json`** at the repository root registers the server for Claude Code
   sessions started in this repository.

## Tools

`screenshot`, `ui_tree [filter]`, `click_text text [role] [count]`,
`click x y [button] [count] [modifiers]`, `move`, `drag from to`,
`mouse_down` / `mouse_up` (hold a button between steps), `scroll x y dy`,
`key "ctrl+s"`, `text`, `wait`, `resize w h`, `info`, `scene` (text dump of
the page, devices, wires, nets and cabinets), `answer_dialog path|paths` and
`dialogs` (native file dialogs are answered by the harness, queue the answer
first), `restart [project] [fresh]` (kills the app, `fresh=false` keeps the
profile to test crash recovery), `check_layout` (cheap layout check of the screen),
`finding` and `report` (file findings, write the audit report; see qa-suite.md), `quit`. Coordinates are window points. The schematic
canvas and the 3D view are painted, not widgets, so they are not in `ui_tree`:
use screenshots and coordinates there. Icon-only buttons have an empty label
in the tree (a finding in itself); hover them and take a screenshot to see the
tooltip.

## Security of the control port

`--control` is a development tool and stays out of tester-facing docs. It binds
127.0.0.1 only. At start the app makes a random 128-bit token and writes
`port` and `token` (two lines) to `KOPERCAD_CONTROL_PORT_FILE`. The first line
of every connection must be `{"cmd":"hello","token":...}`; anything else
(an HTTP request from a web page, wrong JSON, silence for 5 s) closes the
connection. Screenshots go only to the folder in `KOPERCAD_CONTROL_SHOTS`
(the test user sets it; else a folder in the temp dir) and the client sends a
bare file name. With `--connect PORT` pass `--token` or set
`KOPERCAD_CONTROL_TOKEN`. The MCP script builds with cargo if the binaries are
missing. The empty profile moves settings and Documents; on Windows the
sandbox env is best effort, so projects can still land in the real Documents.

## Running it

Linux (needs `apt install xvfb openbox xdotool mesa-vulkan-drivers
libxkbcommon-x11-0`; xdotool is only for `scripts/screenshot.sh`):

```
cargo build -p novo-app -p novo-cli
scripts/test-user-mcp.sh --size 1366x768
```

Options: `--size WxH`, `--project FILE`, `--shots DIR`, `--real-profile`,
`--app PATH`, `--connect PORT`.

## Windows, on Tadjiro's PC (through Remote Control)

Nothing here needs admin rights; the app and the tool are the normal per-user
files from a build (`kopercad.exe`, `kopercad-cli.exe`).

1. A Remote Control session on the PC runs `kopercad-cli.exe test-user
   --size 1366x768` (the MCP server starts `kopercad.exe --control` itself
   with an empty profile) or, to test the real profile,
   `kopercad.exe --control=47800` and `kopercad-cli.exe test-user --connect 47800
   --real-profile`.
2. The MCP server must run where the session's Claude can reach it: register
   it in that session with `claude mcp add kopercad-test-user -- C:\path\kopercad-cli.exe
   test-user` (per-user config, no elevation). The cloud sessions cannot reach
   the PC's loopback port, which is intended.
3. This gives real GPU, real DirectX 12 / Vulkan drivers, DPI and scaling:
   set the display scale in Windows (125%, 150%) before starting; `info`
   reports `pixels_per_point` and `screenshot` reports pixels versus points.
4. Windows caveats: the empty profile sets `APPDATA` and `LOCALAPPDATA` for
   the child, but `Documents` comes from the known-folder lookup and stays
   the user's real one, so new projects land there. Use a throw-away
   Windows user for a fully clean first run.

## What is verified

Linux only so far (Xvfb, lavapipe software rendering, debug build): start,
screenshot, ui_tree, click_text, click, key, text, resize, quit through the MCP
protocol. Not verified: Windows (any step), real GPUs, scaling other than 1.0,
IME and non-US keyboard layouts (text is injected as characters, key names are
egui key names), drag on the 3D view, file dialogs (native dialogs are outside
the window and cannot be seen or clicked; use `restart project=FILE` or the
in-app flows).

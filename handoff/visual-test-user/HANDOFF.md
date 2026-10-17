# Visual test user: handoff

Branch `claude/project-thread-akr2en`, PR #942 (kopercad). Thread: "Visual test-user harness".

State: control mode and MCP server work on Linux (Xvfb, openbox, lavapipe, debug build). Verified through the MCP protocol: screenshot, ui_tree, click_text, click, key, text, resize, quit; placed a breaker, contactor coil and motor on a new project. Not verified: Windows, real GPU, DPI above 1.0, native dialogs, drag on canvas/3D.

Resume:
```
apt install xvfb openbox mesa-vulkan-drivers libxkbcommon-x11-0
cargo build -p novo-app -p novo-cli
scripts/test-user-mcp.sh --size 1366x768      # MCP over stdio
```
Quick manual driver used in the first session: a Python JSON-RPC client over that stdio (initialize, then tools/call). Screenshots land in `$TMP/kopercad-test-user/shot-NNN.png`.

Code: `crates/novo-app/src/control.rs` (egui Plugin: input_hook injects events, output_hook captures the AccessKit tree, ViewportCommand::Screenshot), `crates/novo-cli/src/testuser.rs` (MCP, launches app, Xvfb).

Traps: egui screenshot arrives as Event::Screenshot one frame later; the app clamps the window to the screen (`fit_window_to_screen`), so Xvfb is made 40x80 larger than --size. Ctrl+S on an untitled project opens a native dialog the harness cannot see.
Plan alignment: scenario ids A to J are those of `/mnt/project-files/release/v0.1-plan.md`.

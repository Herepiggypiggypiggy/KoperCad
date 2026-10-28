# Display geometry: handoff

Where: `display.rs` (`track` is called from `NovoApp::ui`), `dialogs.rs` (`remember_owner`, `take_native_dialog_ran`), `diag.rs` (`Display:` line in the problem report).

What is proven: the pure size rules (unit tests, including the 3378 x 32711 case) and the Windows build (`cargo clippy --target x86_64-pc-windows-gnu`).
What is not proven: nothing ran on the mirrored portrait monitor. KC-04 (panels ignore the mouse) is explained by a hypothesis: the swap chain was larger than the client area, so the picture was scaled and the pointer mapped to the wrong place. The size correction removes that state; if panels still ignore the mouse, the log line `Display: window ... monitor ...` and the report line show the numbers to look at next.
The only `unsafe` is in `dialogs.rs::owner` (Windows only, `WindowHandle::borrow_raw` of the live main window, used on the window thread during a modal dialog).

# Intel start-up crash: handoff

Branch `claude/intel-start-crash` (from main 78a1dab). One PR covers findings 1 and 21. Related: PR #1568 (multi-monitor, same startup/graphics area: display.rs, app.rs, lib.rs). Rebase on main after #1568 merges; expected overlap is only lib.rs (a `mod` line).

What it does
- `gpu::order(entries, wish)`: key = named adapter, then real GPU before software (Cpu/Other), then not-avoided before avoided, then kind rank, then backend (Windows only: Dx12, Vulkan, Gl). WARP is a DX12 adapter but the kind still decides first.
- `startup.rs`: `begin()` in `main` (after `diag::init`) creates `start-pending.txt` in the log folder, holds an exclusive `File::try_lock`, and rewrites it with step and adapter (`startup::adapter` is called by `gpu::select` before `is_surface_supported` and `request_device`). `first_frames_done()` (3 frames, via `on_end_pass` in lib.rs) or `ended()` (main, after `run()` returns) remove it. A file left behind with no lock holder is a start that died: log line, avoid list entry (`graphics-avoid.txt`, newest 8), and a Yes/No question before the window opens (Yes = safe graphics for this run). Not asked under `--control` (QA harness). A second copy starting at the same moment sees the lock and tracks nothing.
- A normal start that reaches the first frames on an avoided adapter removes it from the list.
- Problem report has a line "Graphics adapters tried last ..." when the list is not empty.

Tests: `cargo test -p novo-app --lib -- startup gpu packaging`.
Only real Windows can prove: that DX12 first fixes the Intel UHD laptop (31.0.101.2125); that the lock behaves as in the unit test (Windows uses LockFileEx); that the question appears and "Yes" starts with WARP; that nothing is written outside %LOCALAPPDATA%\KoperCAD\logs (portable: data\local\KoperCAD\logs). Not tested: a real access violation. Windows SEH handling is not in the program (unsafe is denied workspace-wide).

Traps
- `File::try_lock` needs Rust 1.89 or newer.
- Packaging: the zip has no `kopercad.portable` marker on purpose (run-latest.ps1 replaces the folder each update).
- Dropping `novoelectric.exe`: a launcher older than the rename needs `setup-new-pc.ps1` again (it no longer needs gh).
- Release note for rc11 not added: `docs/release-notes/unreleased.md` is being changed by #1569. Tell the Release thread: "DirectX 12 before Vulkan on Windows; a start that never draws its window is noticed and safe graphics offered; experimental zip has run.cmd/run-safe-graphics.cmd/readmes".

# Intel start-up crash and experimental packaging: roadmap

Source: laptop test report 2026-10-28 (cooling tower cell), findings 1 (Critical) and 21.

Scope
- Finding 1: `kopercad.exe` exits with 0xC0000005 right after "Graphics adapters found" on Intel UHD (driver 31.0.101.2125, Windows 11); no window, no report. `--safe-graphics` and `WGPU_BACKEND=dx12` start fine.
- Finding 21: the experimental zip lacked launchers and readmes, shipped three 74 MB exes, and the launcher scripts asked for a `gh` sign-in although the repo is public.

Done (PR claude/intel-start-crash)
- gpu.rs: on Windows DirectX 12 is tried before Vulkan for the same kind of adapter; an adapter that stopped a start is tried last (but before the software renderer).
- startup.rs: "start pending" marker with an exclusive lock, step and adapter trail, question at the next start, avoid list, self-forgiving.
- Packaging: run.cmd, run-safe-graphics.cmd, README-EXPERIMENTAL.txt, TESTER-README.md in the experimental zip; no novoelectric.exe copy; canvas-demo.exe a separate release file; run-safe-graphics.cmd on the latest-build branch.
- Scripts: run-latest.ps1, setup-new-pc.ps1, build-local.ps1 use public URLs, no gh.

Next / open
- Real Windows proof (see HANDOFF.md).
- No in-process handler for the native crash itself (`unsafe` is denied); the trail is the log line and marker written before each driver call.
- If DX12-first is not enough on some machine: the avoid list covers the second start; consider a `WGPU_BACKEND` hint in the question.

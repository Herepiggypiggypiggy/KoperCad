# EPLAN-BRIDGE handoff

Branch `claude/eplan-bridge-bxvoxq`, PR from main (see PROGRESS.md for the number and head SHA). Reviewer R2 `cse_01SEokxUZ1MKDwyQvLm4DPys`, lead A `cse_01FiLvRBKEcNNHbQEGhYyL1S`, findings thread `cse_01VQS5EbJ8bqSb6DPZ72Kuyw`.

State: engine, script, CLI and app entry done and tested (interop, workflow, cli, app tests, fmt, clippy). The `.cs` script has NOT been run in a real EPLAN: it uses the documented scripting pattern (`[Start]`, `[DeclareAction]`, `CommandLineInterpreter.Execute("export", ctx)` with TYPE=PXFPROJECT and EXPORTFILE, `PathMap.SubstitutePath`, `Decider`).

Resume: `cargo test -p novo-interop --lib export_script`, `cargo test -p novo-cli --test cli eplan`.

Traps
- `ScriptVersion` in the `.cs` must equal `SCRIPT_VERSION` in Rust (a test checks it). Raise both when the script or manifest changes.
- The script file must stay ASCII and free of network calls (tested).
- Never copy anything from the private crane repo; this lane was written from the findings note only.
- Disk: a full workspace `cargo test` fills the cloud disk; clean `target` first.

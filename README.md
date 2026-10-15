# KoperCAD

By KoperVonk. Lightweight, local-first electrical CAD: schematics, terminal and cable
connection screens, cross references, cabinet layout and circuit simulation.
Native Rust, fully offline, scriptable from Python.

```
cargo run -p novo-app
```

To get the newest experimental build on a Windows PC, paste
`scripts/setup-new-pc.ps1` into a PowerShell window once (no admin rights
needed). It puts a "KoperCAD (latest)" shortcut on the desktop.

See `docs/ROADMAP.md` for the plan and `CLAUDE.md` for how the code is organised.

## Name

The product is KoperCAD, made by KoperVonk. It was called NovoElectric until
2026-10-15. Internal names that people never see keep the old name for now: the
repository, the `novo-*` crates, `novoelectric.exe`, the `novoelectric` Python
module and the data folders (`%LOCALAPPDATA%\NovoElectric`,
`Documents\NovoElectric`, `%APPDATA%\NovoElectric`). Moving the data folders
needs a migration from the old location first.

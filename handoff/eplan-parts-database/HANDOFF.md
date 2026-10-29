# Handoff: EPLAN parts database import

Where the user finds it: File > Import parts > Import parts from a CAD parts export, pick an `.adb` or `.eod` file (or any file whose bytes hold articles).

- Reader: `crates/novo-interop/src/eplan/eod.rs`. Scans for zlib streams, reads tagged fields `F1..FE` (see the module doc table). Non-article streams are ignored. Same part under two codes of one maker is reported, not added twice.
- Aliases: `crates/novo-parts/src/makers.rs`. Built-in table has only generic public maker names and common codes. The user's own codes are saved in `maker-aliases.json` beside `parts.json`.
- App: `crates/novo-app/src/app/eplan_parts.rs` (worker thread `start_eplan_parts_job`, "Makers in this file" section in the preview).
- Parts keep `provenance.source = "import:eplan"`, so they never go into shared `.kcpl` bundles.
- Tests use invented fixtures only. Never commit a real EPLAN database or the shop CSV.
- Format knowledge comes from notes of a read-only look at a customer database; it has not been run against the real file in CI.

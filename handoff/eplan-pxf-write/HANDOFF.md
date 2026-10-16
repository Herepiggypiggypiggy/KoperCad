# EPLAN-PXF-WRITE handoff
Branch claude/eplan-pxf-read-ocop20 (same thread as EPLAN-PXF-READ), PR from main. Reviewer R2 cse_01SEokxUZ1MKDwyQvLm4DPys, lead A cse_01FiLvRBKEcNNHbQEGhYyL1S.
Code: crates/novo-interop/src/eplan/pxf/{write,dialect}.rs, tests crates/novo-interop/tests/pxf_write.rs, app command `export.eplan` (app/interop.rs), CLI `export eplan`.
Version-specific values only in `dialect.rs`. Never invent EPLAN 2027 values.
Knowledge source: /mnt/project-files/interop/crane-configurator-findings.md only; never copy customer data.
Resume: cargo test -p novo-interop --test pxf_write; cargo test -p novo-cli --test cli eplan.
Trap: a full workspace build fills the cloud disk; `cargo clean -p <crate>` and CARGO_INCREMENTAL=0.

# Busbar heat: handoff

Branch claude/project-thread-yeat1y. Code: crates/novo-thermal/src/busbar.rs (engine, 15 tests), crates/novo-app/src/app/calculations_busbar.rs (tab), hook in calculations.rs (TABS has 4 entries; the new tab has no lesson anchor).

Resume: `cargo test -p novo-thermal --lib busbar`, `cargo test -p novo-app --lib calculations`.

Honest status: MODEL. Exact in tests: uniform bar with alpha(T) source, fin equation with cooled ends, end heat flow, lumped heat-up curve, skin factor series, joint I2R, energy balance. NOT validated: film and radiation to the cabinet (surroundings at air temperature), proximity (typed factor), skin of thin flat bars (edges ignored), no measured bar. The ampere limit is a planning number for a chosen temperature, not an IEC 61439 verdict.
Traps: Picard iteration freezes source and film; no steady state is reported as Error::Runaway above the melting point. Fixed-end air loss is counted in the end flow.

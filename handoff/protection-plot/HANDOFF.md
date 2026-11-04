# Handoff: protection coordination plot

Branch claude/project-thread-961q0t. Find it in the app: Ctrl+K "Protection" (or View > Protection), tab Curves. Tick devices; "Also draw" toggles cable damage curves (dashed) and motor starts (blocks). Left list shows OK / Check / Fails per device.

Reads: cable section and insulation from the single-line diagram (thinnest run behind each device, `protected_by`), motor current from its box, `starting_current_ratio` (or `locked_rotor_ratio`) and `starting_time_s` on the motor (typical 7x and 5 s, marked assumed).

Honest limits: device curves are standard band limits, not maker curves; cable curve is adiabatic k2S2, verdict only from 5 s to 0.1 s; inrush peak not drawn; no diversity or several motors starting together.

Tests: `cargo test -p novo-checks --lib protection::plot`, `cargo test -p novo-app --lib protection`.
Ideas only from GElectrical (GPL-3); no code taken.

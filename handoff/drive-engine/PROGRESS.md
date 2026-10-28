2026-10-28 09:43 UTC Thread started from lane L1 brief.
2026-10-28 09:50 UTC Engine built (ramps, V/f, bus, I2t, terminals, state machine, sizing); tests, clippy, fmt green; PR 1557 opened (head 13a2f8fd).
2026-10-28 09:57 UTC R1 NOT READY on 13a2f8fd (labels, guards); fixed in 9613ab65.
2026-10-28 10:05 UTC R2 NOT READY: non-finite reference ran at max speed (now trips ReferenceLost), reset restarted with run held (now run inhibit until the run command is released), bus and resistor input guards, S-ramp fraction 0 exactly linear. Pushed as the next head.

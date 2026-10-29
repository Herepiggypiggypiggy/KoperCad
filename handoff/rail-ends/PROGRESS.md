# Rail and duct ends: progress

- 2026-10-28: engine, UI, 14 engine tests and 4 app tests; QA scenario 1590 written, not run here.
- 2026-10-28: follow-ups to #1608: T-junction test, `no_crossing_reason` (hover on the disabled Trim buttons and the message), dead line removed, long-duct speed test. `clear_duct_spot` (#1604) is not in main yet, so its cost is not covered here.
- 2026-10-29: `clear_duct_spot` shortens ducts over 1 m by a hundredth of their length per step (bounded search; 99 m duct with no room: 0.04 s). The wall-clock assertion in the railedit speed test is 20 s (guards only against runaway scans).

# rel-contactor: coil on a power feed warning

Second PR from main (the search PR #963 keeps ROADMAP/HANDOFF/PROGRESS; merge both, then fold this note into them).
New check `ProblemKind::CoilOnPowerFeed` (`novo-checks/src/more_rules.rs`, `coil_on_power_feed`): a contactor coil wired directly to a pole of a three-phase breaker/fuse/contacts (3 or more pass-through groups) or a 3-phase motor gets a warning that names the fix (3-pole power contacts, search `contactor`). Seen in the Issues panel and the issue pill, waivable like other checks.
Limits: only a coil wired directly to the power device is found, not one several wires away.
Test: `a_coil_in_a_three_phase_feed_points_to_the_power_contacts`.

## Aux contacts (follow-up PR)
`coil_on_power_feed` also covers one-pole auxiliary contacts (symbol keys under `/contacts/`): same rule, own message ("one-pole contact in a three-phase feed ... place the 3-pole device"). One warning per device and kind. Test `a_one_pole_contact_in_a_three_phase_feed_is_flagged`.

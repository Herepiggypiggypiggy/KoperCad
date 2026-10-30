# QA evidence for PR 1801

Code head 0b189af0dc44b758e083e58cb2ebc9e88807e5f9 (second merge of origin/main dff6734, plus a fix to the
add-on engine's own test that the "controllers add-on" CI job caught).
Run: `scripts/qa.sh --only ontroller`, 19 runs at 1366x768 and 1920x1080, program built from that source
(report.md says "source at git 0b189af0dc"). 0 failed, 4 S4 "may overlap" notes (1584, 1590, 5630 twice: docks
and dialogs stack on purpose).

Scenarios this PR adds: 5630 (add-on panel tabs, both sizes), 5631 (signal filter), 5632 (all controllers).

An earlier run on the first merge (main 0fffafb) had 1581, 1581b, 1583, 1583b, 1584 and 1584b red; 1583 and 1584 were
red on main too (known, separate fix thread) and 1581 failed on the same kind of click-point drift. They pass on
this head, which contains main's later fixes. Not re-run on main itself.

CI note: the engine test (`cargo test --manifest-path addons/controllers/engine/Cargo.toml`, its own workspace) was not
in my local set and failed on the first push; fixed here and run locally with engine clippy, fmt and the signing round trip.

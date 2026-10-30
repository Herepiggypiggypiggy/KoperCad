# Evidence, PR #1794

HEAD.txt: 781449f0c2c2575f0356334aeb504cd6473d9c04 (merge of main 965c88ee at a601202d0a, then QA scenario edits). The commit that adds this folder comes after it and changes no code.

## What was run
- `cargo fmt`, `cargo clippy -p novo-controller -p novo-machine -p novo-app -p novo-tests --all-targets -D warnings`: clean at a601202d0a.
- Tests at a601202d0a: novo-controller and novo-machine all pass; `novo-app --lib controller_` 42 pass; radio_machine_fixture 3 pass.
- Earlier full `novo-app --lib` at cb056bb6: 2081 pass, 1 fail (`parts_identity::storing_into_a_cache_of_wanted_files...`). Re-run alone: passes (3.76 s). A timing flake under load, unrelated.
- QA at 1366x768 and 1920x1080 (see report.md). 17300, 17300b, 1581, 1581b, 1580, 1582, 1779, 5620 pass.

## Not green, and why
- 1583/1583b (`brake open`) and 1584/1584b (`click_text "Wired pendant..."`) fail. This PR does not edit them. Main's controller card now has an instance row (radio tabs, "+ Add", Pop out) above the signals, which moved the hard-coded coordinates and the picker. I fixed 1581/1581b (they pass now) but did not get 1583/1584 working. I did not run main alone to prove they fail there too, so "caused by main's card change" is likely, not proven.
- Parts scenarios 1581-assign-all, 1582-assign-all, 1584-parts-window: `click_text Place #2` fails; unrelated to controllers (caught by the `--only 158` filter).
- Menu rows: this PR adds none (ceilings File 22, Edit 19, Draw 12, total 186 untouched).
- Run windows keep their "Model, not measurement" footer; no safety claim added.

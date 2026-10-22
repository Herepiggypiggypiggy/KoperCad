# Learning suite: handoff

Code: `crates/novo-app/src/app/learn_suite.rs` (window and cards), state in `learn.rs` (`only_path`, `only_track`, `level`, `window_drawn`), words in `assets/lessons/ui.toml`. QA scenario `qa/scenarios/21-learning-suite.qa` (dark and light, 1366x768 and 1920x1080).
Selecting what to show: neither `only_path` nor `only_track` is Home. Search or a level chip switches the main area to a flat result list inside the current section.
Resume with `cargo test -p novo-app learn` and `scripts/qa.sh --only learning-suite`.
Trap: lesson ids and paths.toml are shared with the content thread; only append.

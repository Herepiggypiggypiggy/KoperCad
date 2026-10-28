# Heat preview and lesson claims: handoff

Branch: `claude/project-thread-p08qjl` (one PR from main). The PR number and head are in PROGRESS.md.

State: preview model rewritten, claims tests added, lesson text corrected, QA scenario 530 added.

Where things are
- Preview model: `crates/novo-heatview/src/synthetic.rs` (`SynthScene::temperature`, constants at the top).
- Where the view reads it: `crates/novo-app/src/app/heat.rs` (`build`, `device_temp`, `drawn_temps`).
- Tests: `heat_preview_tests.rs` (preview against the physics and the 2D solver), `lesson_claims_tests.rs`
  (what lessons quote against the engines). Add a claim there whenever a lesson quotes an engine figure.
- QA: `qa/scenarios/530-learn-heat-claims.qa`; the scene dump (`scene_dump.rs`) has a `heat` entry per cabinet.

Traps
- Lesson steps are limited to 500 characters of text (novo-learn lint): split a step rather than shorten.
- The preview is an estimate fitted to one cabinet; the legend says so. Never present it as a solver result.
- The wire-routing sample feeds seven lessons: a change to routing or rails changes their numbers; run
  `cargo test -p novo-app --lib lesson_claims_tests` and fix the text, EN and NL.
- Another thread owns lesson navigation (suite, panel highlight, Apply fixes): this PR touches only the
  text of heat-and-sizing, cut-lengths, panel-labels and wire-routing, plus a split step in two of them.

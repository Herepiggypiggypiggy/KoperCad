# Controller: handoff

State (2026-10-28 UTC): lane C1 engine merged in #1558; follow-up fixes PR open. Engine written and tested in `crates/novo-controller` (profile + schema + validator, control kinds,
signals with step semantics, interlocks, link-loss/self-test effects, instances, bindings and checks, sequence record/replay). Follow-up PR open
from branch `claude/project-thread-z6unbc`; no UI yet by design (C3 owns the panel, C0b the add-on host).
Docs: `docs/design/controller-profile.md`. Fixture profile: `crates/novo-controller/tests/data/`.
Resume: `cargo test -p novo-controller`, `cargo clippy -p novo-controller --all-targets -- -D warnings`,
`cargo build -p novo-controller --target wasm32-unknown-unknown` (needs `rustup target add wasm32-unknown-unknown`).
Rules: no vendor profiles or customer files in the repository; shipped profiles are generic. No dependency on novo-core or novo-sim.
Traps: `requires`/`drops_when` are receiver-side latch semantics (validator warns on other kinds); selector remap is signals with
`requires = ["sel=pos"]`; E-stop leaves `pressed` only through `reset`.
People: C4 import thread cmsg_01TV2QyH4AYw9X2aTmgUsuhC8mQQyNbZNcy3uo9rm8bA8D (builds `Profile` values); reviewers and leads per coordination/threads.md.

2026-10-28 C4 import wizard: `novo_controller::import` (table parser, NL/EN word lists in `data/`, `propose` with warnings, TOML in the C1 profile format, device pins, generic library) and the wizard window `crates/novo-app/src/app/controller_import.rs` (File > Import > Controller profile from I/O table). Design: `docs/design/controller-import.md`. Resume: `cargo test -p novo-controller -p novo-app controller_import`. A test parses every library profile with `Profile::parse` and `validate`.

2026-10-28 C2 controller in the live circuit: files `crates/novo-app/src/app/controller_live.rs` (state, drive, storage, checks), `controller_card.rs` (card, keys), `controller_live_tests.rs`; `crates/novo-sim/src/pins.rs`; fixture `crates/novo-tests/src/radio_hoist.rs` and `qa/fixtures/radio-hoist.novo` (regenerate: `KOPERCAD_WRITE_FIXTURES=1 cargo test -p novo-tests --test radio_hoist_fixture`). Design: `docs/design/controller-live.md` (where the user finds it, binding semantics, honest limits, add-on seam). Resume: `cargo test -p novo-app --lib controller_`, `cargo test -p novo-sim --test pins`, `scripts/qa.sh --only controller`. Traps: tests must call `LiveView::reset_poles` and one `solved` frame before acting on the controller (the first frame reads it from the project; contacts rest from `rest_state`); a signal name in a binding that the profile does not have is an engine finding, not a panic; a new live card needs a registered lesson anchor (debug builds panic otherwise; run `UPDATE_ANCHORS_DOC=1 cargo test -p novo-app anchors_doc`).
Open: the Check menu does not list the binding findings yet; the Live card still says "no push button to press" when only a controller drives the circuit; machine module paths wait for L5b; the receiver badge row overlaps the symbol's label.
2026-10-29 C4: the importer also builds `Profile` values (`ProposedProfile::to_profile`); the wizard validates with the engine. Bare directions use the crane convention. Word lists untouched by real pages yet.

2026-10-29 C4 word lists tuned on real page patterns (see `docs/design/controller-import.md`, "Receiver pages in the field"). Still unseen: a wired pendant page and a second maker's receiver terminal table. A user can add words in %APPDATA%\KoperCAD\controllers\words.txt, including `pair.<motion>:` lines.


2026-10-30 C3 widgets: the Controller card is drawn from `novo_controller::panel::plan` by `crates/novo-app/src/app/controller_widgets.rs` (two-step stacks, joystick pad, rotary, mushroom, latch, hold buttons, Sticky, Key map). Design: `docs/design/controller-live.md` ("The panel widgets"). Resume: `cargo test -p novo-controller panel`, `cargo test -p novo-app --lib controller_`, `scripts/qa.sh --only controller` (1584 shows the pendant and the joystick). Traps: widget tests find the pad/stack by `ui.min_rect()` of the last widget; a headless egui frame needs `textures_delta.clear()`; QA click_text names changed (the mushroom is painted, click it by coordinates, not by text). Open: add-on `PanelLayout` has only heading/label/button/lamp/row (no rocker or joystick kinds yet), tabs for several instances, gamepad, his local profiles per `plans/controller-io-facts.md` (never in this repo).

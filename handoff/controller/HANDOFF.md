# Controller: handoff

State (2026-10-28 UTC): lane C1 engine written and tested in `crates/novo-controller` (profile + schema + validator, control kinds,
signals with step semantics, interlocks, link-loss/self-test effects, instances, bindings and checks, sequence record/replay). One PR open
from branch `claude/project-thread-z6unbc`; no UI yet by design (C3 owns the panel, C0b the add-on host).
Docs: `docs/design/controller-profile.md`. Fixture profile: `crates/novo-controller/tests/data/`.
Resume: `cargo test -p novo-controller`, `cargo clippy -p novo-controller --all-targets -- -D warnings`,
`cargo build -p novo-controller --target wasm32-unknown-unknown` (needs `rustup target add wasm32-unknown-unknown`).
Rules: no vendor profiles or customer files in the repository; shipped profiles are generic. No dependency on novo-core or novo-sim.
Traps: `requires`/`drops_when` are receiver-side latch semantics (validator warns on other kinds); selector remap is signals with
`requires = ["sel=pos"]`; E-stop leaves `pressed` only through `reset`.
People: C4 import thread cmsg_01TV2QyH4AYw9X2aTmgUsuhC8mQQyNbZNcy3uo9rm8bA8D (builds `Profile` values); reviewers and leads per coordination/threads.md.

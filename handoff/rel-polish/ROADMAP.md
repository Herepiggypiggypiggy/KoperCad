# REL-POLISH roadmap

Scope: release v0.1.0 blockers B8, B9, B12 (`release/v0.1-plan.md`).

- B9 text readable at fit zoom: done in slice 1 (declutter floor reaches wire numbers).
- B8 part suggestions by device type and control voltage: already on main (`novo-parts/src/fit.rs`, tests `crates/novo-parts/tests/fit.rs`); verified, no change.
- B12 startup safety: slice 2 (adapter choice with software fallback, window floor for 200% scaling, top-bar width tests at DPI widths). Parts window list and top-bar overflow at 1366 verified on main.
- Open: the real Windows check at 125/150/200% (harness thread).

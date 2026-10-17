# Drawing gaps: handoff

Branch `claude/project-thread-cmtxjk`, one PR per item from main. Linux headless only; nobody ran the Windows exe.

Item 1: verified on main that bound safety functions (`safety.functions`) were stored, but the free calculator of the Safety window (risk graph, duty, subsystems) was session-only. Now stored as JSON text under `safety.calculator` in project info properties (`crates/novo-app/src/app/safety_calc.rs`), removed again while it equals the defaults. Additive: older files lack the key and open with defaults; older builds ignore the key. Unreadable values fall back to defaults. Label "model, not exact" unchanged.

Resume: see ROADMAP.md for the next item.

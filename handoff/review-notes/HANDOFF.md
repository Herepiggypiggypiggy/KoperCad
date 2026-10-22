# Review notes: handoff

Branch: claude/project-thread-0mqs8q (kopercad). PR #1413, head 05ffd5c, routed to Lead B.
Where the code is: crates/novo-revisions/src/notes.rs (engine, tests/notes.rs), crates/novo-app/src/app/review_notes.rs (+ _tests), crates/novo-pdf/src/lib.rs (PdfMarker, extra_appendices), crates/novo-canvas MarkKind::Note, crates/novo-editor view.rs (menu_world, context rows), handover.rs (file 10).
Storage: sidecar key `review_notes` via ProjectBackend::review_notes_json (copied by save as).
Decisions: notes are not model records (no undo, no revision bump); a symbol of a device stands for the device; PDF prints open notes only, handover lists all; delete asks a second click; author = settings.initials.
Resume: `cargo test -p novo-revisions --test notes`, `cargo test -p novo-app --lib review_notes`; QA: scripts/qa.sh --only review-notes (needs xvfb openbox mesa-vulkan-drivers libxkbcommon-x11-0).
Reviewer routing: odd PR number to Lead B, even to Lead A.

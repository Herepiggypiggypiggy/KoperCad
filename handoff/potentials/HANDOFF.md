# Handoff: Potentials, PE and interruption points

Branches: `claude/project-thread-kyzp0k` (PR A, novo-terminals), `potentials-interruption-numbering` (PR B, novo-xref). Both from main f6be257.
Reviewer: main reviewer cse_016C9nTT8cN7voW8t1NQUG4W; integration lead cse_01FiLvRBKEcNNHbQEGhYyL1S merges. Roster: coordination/threads.md.

Decisions
- No new model fields: PE is `TerminalKind::ProtectiveEarth`; earthed net = PE/PEN potential defined on it or a pin of `iec/earth/earth` or `iec/earth/protective_earth` (chassis excluded).
- `mark_earthed` changes only plain feed-through terminals, so a fuse/neutral terminal keeps its kind.
- Interruption names are whole numbers, lowest unused; an unnamed partner still takes the other's name on link.

Resume: `cargo test -p novo-terminals -p novo-xref`. Read ROADMAP.md for next steps.

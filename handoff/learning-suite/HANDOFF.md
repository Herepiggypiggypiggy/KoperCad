# Learning suite: handoff

Code: `crates/novo-app/src/app/learn_suite.rs` (window and cards), `learn_ui.rs` (the lesson card), state in `learn.rs` (`only_path`, `only_track`, `level`, `window_drawn`, `hidden`), words in `assets/lessons/ui.toml`. QA scenarios `qa/scenarios/21-learning-suite.qa` (dark and light) and `280` to `285` (navigation), at 1366x768 and 1920x1080.
Selecting what to show: neither `only_path` nor `only_track` is Home. Search or a level chip switches the main area to a flat result list inside the current section.

Navigation model (2026-10-28, branch `claude/project-thread-qzx9qs`):
- Learn window: the rail (chips when narrow) is the only way between Home, a path and a topic. A path or topic page names itself with a small label ("Learning path", "Topic"); there is no "Back to Home" button beside Home any more.
- Lesson card: header = grip, title (cut first), "n of N" (never cut), All lessons (list icon, opens the window at the lesson's topic), cross (hides the card, the lesson waits; Learn or Help > Continue brings it back). Row: Back, Show me how, Next or Do it for me. Below: Restart lesson. The finished card has All lessons and Restart lesson only; the header cross is the one close.
- Icon-only buttons get a name through `named_icon` so the QA tool and screen readers see them.
- `panel.rs`: bare cards hang from a corner until dragged; the pivot is now switched to top left after the drag, not in the middle of it (cards used to jump by their own size), and bare panels do not select text so a drag from the text moves the card.

Lesson rings and greyed controls (2026-10-28):
- A panel reports its anchors with `cabinet_bar::note_rect_in(ui, ..)` (cut to the panel's clip rect); `note_rect(ctx, ..)` is for whole windows. The ring follows the real rectangle every frame.
- In the Cabinet the card parks bottom left (Inspector on the right). Dialogs are kept right of a left card.
- A step with `expect = clicked` on a control that greys out once its work is done needs a state check beside it: `{ any = [ { clicked = ".." }, { <state check> } ] }` (example: Apply in `enclosure-door-and-depth`, `enclosure_depth`). The card shows `greyed_note` when the awaited control is greyed. QA `284-learn-door-real-clicks.qa`.
- Trap: never share one `CARGO_TARGET_DIR` between a worktree and the main checkout of this repo; the artifacts collide and the embedded lesson list or novo-learn comes out stale (empty catalog, missing fields).
- A step that points at a control inside a short scrolling list (example: Apply in the Heat what-if list) needs a reveal flag set from `learn_reveal` and `scroll_to_me` on the control once it is drawn (`Wizard::reveal_apply`). Do not scroll by hand in QA walks; the walk must pass the way a user plays it.

Resume with `cargo test -p novo-app learn_nav panel::` and `scripts/qa.sh --only learn-nav` / `--only learn-panels`.
Trap: lesson ids and paths.toml are shared with the content thread; only append. A label like "All lessons" may be icon-prefixed: scenarios match without the icon glyph.

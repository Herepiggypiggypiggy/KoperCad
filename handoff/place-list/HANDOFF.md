# Handoff: place from a pasted list

Branch `claude/project-thread-m077aw`, one PR from main. Run: `cargo test -p novo-parts -p novo-app --lib place_list`.

- Reading and matching: `crates/novo-parts/src/place_list.rs` (pure). Line = optional tag, words, quantity
  (`x5`, `5x`, leading number, Excel quantity cell). Article number first, then symbol alias/name/function word
  with the other words narrowing the catalog; a part is chosen only when exactly one fits.
- Placement: `place_lines` in `crates/novo-app/src/app/place_list.rs`, grid beside the drawing or at the pointer,
  typed tags count up (F1 x3 -> F1..F3); a used tag falls back to automatic numbering and is reported.
- The preview is a dry run of the same function on a project copy.
- Registry rows: commands.rs, menus.rs PALETTE_ONLY, language.rs, presentation.rs, workbench.rs APP_COMMANDS.
- QA scenario `qa/scenarios/9810-place-from-list.qa` was written without a screen (no openbox or Vulkan in the
  cloud box), so it has not been run.

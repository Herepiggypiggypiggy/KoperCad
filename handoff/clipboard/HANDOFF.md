# Clipboard handoff
Branch claude/copy-cut-paste-hccg7u, PR from main. Engine: crates/novo-workflow/src/clipboard.rs (tests/clipboard.rs). Ghost: Tool::Paste in novo-editor tool.rs. App: crates/novo-app/src/app/clipboard.rs (tests clipboard_tests.rs).
egui-winit turns Ctrl+C/X/V into Copy/Cut/Paste events, not key presses: the keymap chords never fire, the app reads the events (Shift = keep tags, Alt = in place).
After PR #599 merges: merge main and delete edit.copy/edit.cut/edit.paste*/view.override_marks from the known-gaps list in a_command_with_a_key_is_never_comes_later.
Reviewers: R3 cse_01SL6nnpsSpN8Tjt3tfo8K5V, R2 cse_01SEokxUZ1MKDwyQvLm4DPys, lead A cse_01FiLvRBKEcNNHbQEGhYyL1S.

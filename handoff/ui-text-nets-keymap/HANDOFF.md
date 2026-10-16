# UI gaps: free text, net trace, keymap file
Audit source: audit/ui-coverage-drawing.md Part A, gaps 6, 9, 11, 14.
Text: Tool::Text in novo-editor tool.rs, view.rs rename box targets Note and NewNote; command draw.text, key T.
Net trace: crates/novo-app/src/app/net_trace.rs (net.select, net.trace), rows in the editor right-click menu.
Keymap: userkeymap.rs ensure_file and TEMPLATE, command help.keymap_file; the shortcut sheet names how to rebind.
Tests: app/ui_gaps_tests.rs, net_trace.rs, userkeymap.rs.
Skipped: autosave interval setting (a saved project already writes through; only untitled recovery files use it), custom symbol folder (needs a catalog source, not just a path).

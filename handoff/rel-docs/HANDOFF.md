# REL-DOCS handoff

Branch `claude/project-thread-d7xekx` (repo kopercad), one PR from main. Coordinator brief: release plan lane REL-DOCS.

State: all four documents written; see ROADMAP.md for the list. Nothing half-done.

Resume: `git fetch origin main && git checkout claude/project-thread-d7xekx`; `python3 tools/third-party-notices.py` (needs `cargo` and network once for `cargo fetch`) rewrites the notices.

Things to re-check before the RC (they depend on other lanes):
- Help > Report a problem is NOT promised (reviewer asked); the READMEs send Help > About info and any crash-*.txt. When REL-DIAG (B2) lands, update TESTER-README section 4 and RELEASE_NOTES to use the button.
- Workspace keys: only Ctrl+1 and Ctrl+2 exist; Ctrl+3 to Ctrl+5 are still drawer tabs in `actions.rs`. The README says so.
- Licence line ("keep this beta to yourself") follows the plan's proposed default; Tadjiro decides. Do not add LICENSE.
- README links `handoff/release-0-1/` which arrives with #927.
- Portable README.txt in #928 is shorter; the tester README should replace it.

Traps: the generated notices file is 570 KB; the 12 crates without a licence file are listed with a note, covered by the standard MIT/Apache texts.

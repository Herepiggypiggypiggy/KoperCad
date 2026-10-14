# Main reviewer C: roadmap

## Scope
Independent review of the cloud workers' new-crate PRs (WS-12 cables, WS-13 reports, WS-28 CLI/Python), plus small fixer and docs PRs the coordinator routes here. Review only.

## Done (2026-10-14)
- Reviewed to READY: #54, #55, #56, #58, #60, #62, #63, #65, #68 and #71.
- #64 got CHANGES NEEDED after it was already merged; #71 fixed it and is merged.
- Two real bugs caught before merge: #56 deleting the recovery file, and #58's precheck aborting.

## Next
Nothing is waiting on C. Future new-crate PRs need two correctness reviewers.

## Non-blocking follow-ups nobody has done yet (hand to the owning thread)
- WS-12 cables (`crates/novo-cables`):
  - `shield_connection` treats a shield pointing at a terminal that has since been deleted as connected. `check` should flag it.
  - Spare marks are keyed by core name (`cable.spare`) and are lost on a core rename. Wants `CableCore::spare` in core (WS-02).
  - `choose_type` for L+PE without N gives a numbered 3G.
- WS-13 reports:
  - xlsx shows "Created: 2000-01-01". Consider the project's last-saved time instead.
  - #68: generated sheet names `20.1`, `20.2` can collide with a real page `20.1`.
  - Generated pages that grow conflict with DESIGN.md's pinned flow (a note for revisions, WS-18).
- WS-28:
  - Python `place(..., device=k)` defaults `main=False`, so a scripted coil gets "No main function" from `novo check`. Default `main` from the symbol class (novo-editor `classify`).
  - novo-cli pulls in egui through `novo-editor/egui` just for `art`. #70 merged the light `art` feature; switch novo-cli to it.
  - Now that #68 adds `terminals`/`cables`, the report-id docstrings ("toc, bom, devices or wires") are stale.
- WS-06 editor (#62):
  - Verb-first `no KM1` / `no M1` / `no F3` / `no X2` gets no parent, because these tags are symbol aliases. Consider preferring the device reading when a device with that tag exists.
  - `pt100` reads as device -PT100.
  - `edit::place` (scripts) does not refuse a second main symbol; only the editor does.
- WS-08 workflow: `split_verb` slices the original query at the byte length of the lowercased verb, which is only safe for ASCII.
- merge-main.sh: the header comment (lines 4-6) still describes the old "keep both, drop duplicates" rule.
- MAIN-HANDOFF: drop Claude session ids from public-facing files if the repo goes public (they are not secrets).

## Open questions
None pending from me.

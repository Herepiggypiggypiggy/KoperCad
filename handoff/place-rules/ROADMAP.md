# Place rules (R7): roadmap

Scope: R7 of `docs/PRO-PANEL-GAP-ANALYSIS.md`: rules-based Place all in `novo-layout::rules`. Design: `docs/design/cabinet-layout.md` section 5b.

## Done
- Rule set (zone, own or shared rail, gaps, hot rule) with text file parse and write; `place_all_rules` returning the existing `Plan`; `rules::model::specs`. 18 tests in `crates/novo-layout/tests/rules.rs`.

## Next
1. (done in the app PR) Place all uses the rules; `place-rules.txt` in the user's profile; "Place rules" section in the Plate card.
2. Read the same rules from the CLI (`kopercad-cli place`), if wanted.
3. Rotation to fit, stacking of small devices, a `max_per_rail` rule.

## Open questions
- Default zones are a common panel practice, not a standard; the owner should adjust them after trying a real panel.

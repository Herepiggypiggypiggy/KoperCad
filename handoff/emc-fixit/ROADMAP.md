# EMC fix-it: roadmap

Scope: each EMC finding offers the fixes the program can do, previewed on a copy of the project before anything changes, applied as one undo step. EMC stays a rule-based design aid, never compliance.

Done
- `novo_emc::project::preview(project, fix)`: applies the fix to a clone, re-runs the rules, returns findings before/after, cleared, added, and score changes.
- Applicable fixes: declare class, add shield, shield ends all round, add filter, plate finish bare (`BarePlate`), device bonded (`BondDevice`, new), shorten pigtail (`ShortenPigtail`, only ends longer than the limit).
- Check > EMC check: "Preview: <fix>" per finding, card with Apply (one undo step) and Cancel.
- Nits: the 3D/plate EMC panel no longer says "stand-in until the rule engine is connected"; a macro never lands on a generated page (goes to the first drawing page).

- Follow-up 1: the previewed fix is drawn before/after in the 3D and plate EMC views (banner with Before/After, Apply, Cancel).

Next
- Fixes that move things (separate runs, move device, other duct) stay by hand until routing takes a duct preference.
- Cable card in the schematic panel could offer Preview as well (it applies directly today).

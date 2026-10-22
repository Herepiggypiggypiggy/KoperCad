# Heat-aware Place all: roadmap

Scope: Place all (novo-layout rules planner) uses loss and temperature limits: spreads hot devices, keeps sensitive ones off and below the heat, respects rails, ducts and the text work.

Done (PR from `claude/project-thread-g1cq89`, based on main):
- `novo_layout::heat`: zone override, rows-with-sources-first, swap and slide search, coupling score.
- Rule file `heat on|off`, Rules window check box, message after Place all, inspector reason.
- Rule check (`check.rs`) uses the same zone override.
- Validation example and tests with the 2D model (`heat_place_all`).

Next, in order:
1. Calibrate the coupling kernel against the 2D model with more layouts (CV-200 gain is 1.7 K mean margin, T1 unchanged).
2. Optional "Heat check" button after the preview: run the 2D model on the plan and the rules-only plan, show over-limit counts (reuse PR #1365 machinery once merged; not stacked here).
3. Use real losses and limits once the Real part data lane lands (CV-200 file carries assumptions).
4. Cooler layout (#1365) and this step share the model run; unify.

Open questions: whether a hot-zone default other than middle fits drives better; vent and fan position sweep is a separate lane.

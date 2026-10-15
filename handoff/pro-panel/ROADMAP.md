# Pro Panel research (thread): roadmap

Scope: research EPLAN Pro Panel capabilities from public sources, compare with KoperCAD, rank what to build. Output: `docs/PRO-PANEL-GAP-ANALYSIS.md`. Owner decision: 2D first, 3D later.

## Done
- Gap analysis doc (capabilities C1 to C16, gap matrix, data needs, ranked plan R0 to R9, skip list, proposed threads).

- R7 rules-based Place all: merged as #436 (engine in `novo_layout::rules`, design `docs/design/cabinet-layout.md` 5b).
- R8 assembly checklist: merged as #451 (`novo_reports::AssemblyChecklist`, design `docs/design/assembly-checklist.md`), six steps including the drill table from #397.

## Next
1. R7 in the app: Place all uses the rules, a rules file in the user's profile, a small editor on the Cabinet canvas. After #409, #419 and #406 merge; agree with the Layout thread (cse_01UXbCeQW22y6tv74F2Xvbvm).
2. R8 in the app: palette command `export.assembly` writing the PDF beside the project; needs the stand-alone report PDF (#404). Add `row_mm` for sign-off rows.
3. Get #379 (this doc) merged as docs only; the integration lead merges.
4. Remaining plan items R0 to R6 and R9 belong to the follow-up threads in doc section 8.

## Open questions
- Door-swing simulation in Pro Panel could not be confirmed from public pages; owner may describe the real feature.
- Which wire and drilling machines the owner's workshop uses (decides the first machine format).

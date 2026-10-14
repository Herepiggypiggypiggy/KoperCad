# Design reviewer: roadmap

Scope: second reviewer for UI, visual and workflow PRs in NovoElectric. Reviews each against docs/DESIGN.md (draft: /mnt/project-files/design/DESIGN-draft.md) for coherence, elegance and flow, plus anything copied from EPLAN or other CAD without a reason. Also checks correctness and the no-admin rule. Verdicts go on the PR (COMMENT reviews, since the owner account authors every PR) and to the integration lead.

My PRs (set by the integration lead): #24 (DESIGN.md), #20 (theme), #38 (novo-editor, UI side), and later the concept widgets PR. The main reviewer keeps core, engine, storage and test PRs.

## Done
- #20 reviewed at b326a85: changes needed (4 required items).

## Next, in order
2. #38 at a4da520: finish the review (correctness agent plus fmt, clippy and tests), then post the verdict.
3. Re-review #20 when the layout thread sends a new SHA.
4. Concept widgets PR, when it opens.

## Open questions
- WORKFLOW.md (on main) and DESIGN.md disagree (Tab, W vs drag-connect, status bar vs dock). PRs built to WORKFLOW.md before DESIGN.md merges: gaps go to follow-up PRs, they don't block.

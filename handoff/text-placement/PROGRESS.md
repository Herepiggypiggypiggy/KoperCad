2026-10-22 09:35 UTC Studied current text placement (model.rs, note_place.rs, declutter.rs, art.rs); wrote docs/TEXT-PLACEMENT.md.
2026-10-22 09:32 UTC Plan PR #1351 merged (READY from Reviewer A on e4b1bdc).
2026-10-22 09:38 UTC Stage 0: text_audit module in novo-canvas, scene dump `text_audit`, tests/text_baseline.rs (baseline 0/1/1 on motor starter).
2026-10-22 10:00 UTC Stage 0 review fixes: frame text pairs independent of ids, rotated labels tested against ink and wires in their own frame (baseline still 0/1/1).
2026-10-22 10:35 UTC Stage 0b: Label::inside, own-item tag ink counts, tight and outside-sheet counts (motor starter baseline 0/1/1, tight 0, outside 0). Stress kit #1359 by the Text collision stress test thread will call audit().
2026-10-22 10:50 UTC Stage 2a committed locally on branch local-2a (not pushed until #1361 merges): tag_anchor_after in model.rs + test.
2026-10-22 11:25 UTC Stage 2a: tag anchor follows mirror/half turn; stress matrix 4,287 -> 3,354 conflicts (own-symbol causes together 1,091 -> 18; 18 device tags remain over their own symbol, the rest of the 1,091 were terminal, feed and interruption names); one page grew: terminals row mirrored 100 -> 118. QA pictures /mnt/project-files/qa/text-placement-2a.

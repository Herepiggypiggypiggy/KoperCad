# Text collision stress test: roadmap

Scope: find every text-on-text and text-on-drawing collision, group by cause, keep a regression kit; work with the Text placement thread (docs/TEXT-PLACEMENT.md), which owns the engine.

Done: headless finder (novo-tests text_conflicts), stress matrix (90 pages), baseline test, scenarios 60/61/62, PDF and DXF checkers, findings T-1..T-12 in /mnt/project-files/audit/text-conflicts/findings.md.
Next: PLC channel/address pages, bench packet and label sheet text, wire numbers from the numbering engine, short wire segments, 3D labels with heat callouts, seeded fuzz (tester B), re-test each placement stage and lower the baseline.
Open: is a PE terminal dropped left of a strip meant to start a new strip (T-3)?

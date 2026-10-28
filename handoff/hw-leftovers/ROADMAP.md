# Hardware test leftovers (reports, export, cabinet, S4)

Scope: findings of the 2026-10-28 Windows hardware test (`qa/hardware-2026-10-28/` in the project files) that no other lane owns. Other lanes: #1620 file and saving, #1613 parts import, #1616 calc and numbering, #1610 Ask Claude / Build / Live, Windows UI and performance lane.

Done (PRs from main, no stacking): #1624 reports and export (FEC-12, PIR-05..07), #1626 cabinet and heat (CH-02..05, TCN-04 partly), #1628 S4 polish (FEC-14, FEC-16, LSC-09, 11, 13, 15, PIR-04).

Next: answer review findings; when #1616 merges, look at SB-06/SB-07 and LSC-10 if still open; TCN-04 second step (fold the two wrapped button rows of the Terminals drawer); LSC-08 fixed-height status block in Live.

Open question: a `--stamp` option for the CLI's PLCopen export (the 1970 date is deterministic on purpose).

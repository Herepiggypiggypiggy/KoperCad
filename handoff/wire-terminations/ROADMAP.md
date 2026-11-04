# Wire terminations, crimp list, sleeve text

Source: research/panelpro gap.md ranks 2 (ideas only, own implementation).

Done in the first PR:
- Per-end termination (ferrule, ring, fork, spade, pin, bare) stored as wire properties `termination_a` / `termination_b` (`ring M5`).
- Optional `sleeve_text` per wire, printed on sleeve/flag labels and cut-piece labels; the wire number stays in schedules.
- Crimp list: Bench > Ferrules tab ("Lugs to crimp"), "Lugs to crimp" table in the ferrule PDF and bench packet, "Crimp list" section in the Wire production report, three new job CSV columns.
- Inspector > "Wire ends" (kind list, size, sleeve text), help behind "?".

Next (not done):
- Stud/tab size defaults from the part's connection point data (`pin.<m>.*`), so a ring size is proposed from the clamp.
- Lug part numbers / colour codes per cross-section (a lug standard like the ferrule standard).
- Check: lug size vs stated clamp accepts (WireRange family).
- Sign-off wiring sheet (separate gap row).

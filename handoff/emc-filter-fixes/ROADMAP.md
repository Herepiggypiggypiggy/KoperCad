# EMC filter fixes and small UI findings: roadmap

Source: laptop test report, cooling tower cell (findings 8, 17, 26, 27).

Done: EMC-07 counts only a wired filter; the shield fix states the ends in one step; "1 wire" wording; interruption point names declutter; close button on both inspectors.

Open: finding 26 "F6 tag label jumps from left to right after wiring" is not reproduced. Tags move by design (`place_tags` in novo-canvas) when a wire covers their place and move back when it is free; a repro project is needed.

Not done: a fix that draws and wires the filter by itself (needs a series-insert command in the editor).

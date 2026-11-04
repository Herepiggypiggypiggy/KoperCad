# Evidence PR 1874
Where the user finds it: Check > Calculations > Busbars tab (Ctrl+K "calculations", then Busbars).
Shown: inputs, "(model, quasi-static)" badge, the line "Straight parallel bars, no steel nearby, no joints or bends. Planning numbers."; detail (solid cables, not modelled list) behind the "?".
Speed: the solve runs on a worker thread; worst case (3 phases of 4 stacked bars, 1 kHz, 432 filaments) takes 0.4 s in a release build and about 4 s in debug, off the UI thread. A drag does not start new solves.
Not tested: Windows, real busbars against a measurement, the contents of the clipboard after Copy as text (the click is covered), aluminium in the GUI, full workspace tests (CI).

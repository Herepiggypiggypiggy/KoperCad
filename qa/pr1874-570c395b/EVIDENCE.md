# Evidence PR 1874
Where the user finds it: Check > Calculations > Busbars tab (Ctrl+K "calculations", then Busbars).
Shown: inputs and the "(model, quasi-static)" badge; the model limits, solid cables and the loss ratio sit behind "?".
Changes since the last evidence: the Frequency sweep runs on a worker thread (button disabled, "Sweeping..."); a worker panic shows "The calculation failed: ..." with Try again and is never respawned on its own (unit tests: panic comes back as text, a finished solve and a sweep arrive without blocking); the explanation lines moved behind "?".
Speed: worst case (3 phases of 4 stacked bars, 1 kHz, 432 filaments) 0.4 s release, about 4 s debug, off the UI thread. Filament cap is 450.
Not tested: Windows, real busbars against a measurement, an actual worker panic in the GUI (only in unit tests), clipboard contents after Copy as text, full workspace tests (CI).

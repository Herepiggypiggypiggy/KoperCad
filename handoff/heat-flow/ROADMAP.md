# Heat flow readability: roadmap

Goal (Tadjiro, 2026-10-28): airflow and heat flow are too busy and give no sense of "a lot or a little".

1. Done in this PR: Flow control (Off, Arrows, Streamlines, Speed map) with Strength in the 3D Heat strip; one absolute m/s scale shared by all modes; words (still, gentle, moderate, brisk, fast); through-flow row with per-opening m3/h and a verdict; probe readout with the speed word; the flat Cabinet heat view uses the same scale, calm line count and a small key.
2. Next: heat flux (W/m2) legend bar for the Heat flow layer and a total W readout; Flow chip with modes in the flat Cabinet view; fold into the 3D panel rework when #1599 and the foldable panel land.

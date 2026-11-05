# PR 1949 evidence (head a720a8ea68be30eb87139b64d1b5959d79e3061c)

Built from this head (`HEAD.txt`); the reports say `source at git a720a8ea68`, no older-binary warning. Linux, Xvfb, `scripts/qa.sh --only <n>`.

## Where the user finds it
Cabinet or 3D Heat view > Heat > **Heat-up**; the line "Wall and plate mass: 90 % after ... (model, not measurement)" in the strip. The hover (one paragraph) now also says whether the wall sheet and plate values are assumed, from the catalogue or measured, and the catalogue weight check when a weight is known.

## Pictures
`strip-1366x768.png` (line fits uncut), `hover-1366x768.png`, `hover-1920x1080.png`.

## Scenarios (this head)
18590 (1366x768) and 18591 (1920x1080): pass (expect "Wall and plate mass", layout and blank checks, hover shot).
The numbers in the hover differ from the #1859 pictures (1 h 16 min instead of 1 h 02 min): the fixture's cabinet now carries more of the enclosure record after the main merge; the wording is what this PR changes.

## Hover claim
"Mass changes how fast the final temperature is reached, not the final temperature" is pinned by `mass_does_not_change_the_final_temperature` (same network with and without wall and plate mass, steady state equal to 1e-6 K). The 2 % tolerance belongs only to `thicker_walls_take_longer` (a thicker sheet changes the film split of the shell).

## Not tested
Windows; 3D Heat view picture; any measured cabinet. Engine checked against its own analytic unit tests only.

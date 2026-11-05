# PR 1859 evidence (head 2c4a49f37b9e251268ae4b028e4169acbd96bc67)

Built from this head (`HEAD.txt`); every report says `source at git 2c4a49f37b`, no older-binary warning. Linux, Xvfb, `scripts/qa.sh --only <n>`, both sizes unless noted.

## Where the user finds it
Cabinet view (and 3D) > Heat > **Heat-up** button. Next to the slider strip a line appears on its own row: "Wall and plate mass: 90 % after 1 h 02 min (model, not measurement)". Hover it for the details, including the limits of the lumped model (one air temperature, devices as one lump, no rails/ducts/wires/gasket/sun) and where the "% too short" figure comes from (the same model with and without wall and plate mass; model against model, not a measurement). Help > About the numbers, Heat row, carries the same statement.

## Pictures (fit uncut at 1366)
| File | Shows |
| --- | --- |
| strip-1366x768.png, strip-1920x1080.png | Heat-up strip with the new line |
| hover-1366x768.png, hover-1920x1080.png | the hover text |

## Scenarios
New: 18590 (1366x768), 18591 (1920x1080): Heat-up in Cabinet, `expect "Wall and plate mass"`, layout and blank checks: **pass**.

Existing, on this head: 110 (18/18 pass), 112 (8/8 pass), 135 (4/4 pass), 1814 (2/2 pass), 1815 (4/4 pass), 18150 (2/2 pass). 110 and 112 report only S4 "controls may overlap" notes in 3D at 1280x688 and one in 18110 at 1366; these are 3D docks, not the strip.

Red, all red on main 60beb575c3 as well (same scenario, same step):
| Scenario | Failure | Main 60beb57 |
| --- | --- | --- |
| 1602-3d-heat-cloud-and-slice-solids (inside the 160 run) | `Slice: darker squares` not on screen, both sizes | same, both sizes |
| 380-3d-cooling-advice-2d-vs-3d-comparison | `Model comparison` and `not a measurement` not on screen, both sizes | same, both sizes |
| 250-3d-heat-strip-room-short-window | `click_text Quick` fails because the automatic Heat run has started; it flips between 1280x688 and 1366x768 between runs (main failed 1280x688, this head failed 1366x768 in two runs and 1280x688 in one) | timing: red at 1280x688 |
250-heat-scale-hot-cabinet passes at both sizes.

## Not tested
- The engine is checked only against its own unit tests (analytic cases: lumped exponential, 2-node eigenvalues, first Fourier mode and series solution of a slab, sinusoidal ambient, duty-cycle periodic state, IEC k b A total, heat balance). No measured cabinet. "Not validated" stays on the line.
- The animation frames and curve still come from the 2D run without wall mass.
- Not run on Windows.

# Crane simulation realism: plan

Goal (Tadjiro, 2026-11-04): the crane in the simulation (hoist, bridge, crab driven from the circuit)
should look and move far more like a real crane. A real crane project STEP file was uploaded as the
reference for shapes. Ambition stays high: real shapes first, then behaviour.

## Reference

A customer STEP file (private, kept on the project mount, never committed) was inspected for proportions
and typical details. Generic proportions only, nothing copied: no geometry, names or numbers. The
private notes are in the project mount's `handoff/crane-sim-realism/PLAN-private.md`.

Reader check: the STEP reader reads such a file correctly (checked numerically). Lane R2 is dropped.

## Lanes, one small PR each, no stacking

| Lane | Content | Where the user finds it |
| --- | --- | --- |
| S1 | Generated realistic crane parts in `novo-view3d` (`crane_parts.rs`): runway beam, crane rail, columns, box girder with stiffeners, walkway and handrail, flanged wheels that turn with travel, end trucks with axle boxes, geared motors, buffers, control box, trolley frame and rails, grooved drum that turns with the hook, two falls of rope, hook block with turning sheaves, forged hook, two-leg sling | Simulate > Hoist in 3D |
| S2 | Look: paint and material per part group (metal, painted, rubber), per-part colours, shadows and ambient occlusion tuned for the crane scene, floor with markings, building shell option | Hoist in 3D, View menu |
| S3 | Behaviour in the picture: bridge skew and wheel slip warning, rope slack and stretch, brake and motor state lamps on the crane, limit switch cams that really trip, anti-collision zones | Hoist in 3D |
| S4 | Crane kinds: double girder, monorail (KBK), jib crane, tandem cranes; crane size presets from the setup | Crane setup in Hoist in 3D |
| R1 | Per-product meshes from a STEP file (`part3d::build::placed_parts`, added with S1 work) | internal |
| R3 | User model slot: "Use my crane model" picks a STEP file at run time (private, never copied into the project unless the user embeds it), with part roles (wheel, drum, hook block) mapped by name | Hoist in 3D, crane setup |

## Honesty

The picture is generic. Dimensions are the numbers the simulation uses; the details are typical of a
workshop crane. Nothing here is a structural or clearance check.

## Evidence rules

Evidence built from the exact head (HEAD.txt equal to the sha, report saying "source at git <sha>"),
1366x768 and 1920x1080, `check_layout`, regressions baselined on clean main, pictures on the project
mount under `/mnt/project-files/qa/pr<N>-<sha8>/` and only `HEAD.txt` and `report.md` committed.

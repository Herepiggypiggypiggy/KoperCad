# Crane simulation realism: plan

Goal (Tadjiro, 2026-11-04): the crane in the simulation (hoist, bridge, crab driven from the circuit)
should look and move far more like a real crane. A real crane project STEP file was uploaded as the
reference for shapes. Ambition stays high: real shapes first, then behaviour.

## What the reference file is (private, never committed)

`Vandenberg-ATD-RevC.STEP` (85 MB, SolidWorks 2020, about 128 placed solids, reads in 8 s with the
existing reader). It is a crane bridge: about 11.5 m span, two box girders (6 mm plates, 1 m deep,
stiffeners every 1.5 m), a walkway about 0.5 m wide with a 33.7 / 48.3 mm tube handrail, four end
truck wheel boxes with flanged wheels and geared motors (about 880 x 455 x 360 mm), buffers and
brackets. Only generic proportions and typical details were taken from it; no geometry, names or
numbers are copied. The file stays on the project mount and Tadjiro's PC.

Reader finding: big plates with many holes triangulate badly (huge overlapping triangles at coarse
tolerance). Loading the file at runtime needs a better planar-face triangulator first (lane R2).

## Lanes, one small PR each, no stacking

| Lane | Content | Where the user finds it |
| --- | --- | --- |
| S1 | Generated realistic crane parts in `novo-view3d` (`crane_parts.rs`): runway beam, crane rail, columns, box girder with stiffeners, walkway and handrail, flanged wheels that turn with travel, end trucks with axle boxes, geared motors, buffers, control box, trolley frame and rails, grooved drum that turns with the hook, two falls of rope, hook block with turning sheaves, forged hook, two-leg sling | Simulate > Hoist in 3D |
| S2 | Look: paint and material per part group (metal, painted, rubber), per-part colours, shadows and ambient occlusion tuned for the crane scene, floor with markings, building shell option | Hoist in 3D, View menu |
| S3 | Behaviour in the picture: bridge skew and wheel slip warning, rope slack and stretch, brake and motor state lamps on the crane, limit switch cams that really trip, anti-collision zones | Hoist in 3D |
| S4 | Crane kinds: double girder, monorail (KBK), jib crane, tandem cranes; crane size presets from the setup | Crane setup in Hoist in 3D |
| R1 | Per-product meshes from a STEP file (`part3d::build::placed_parts`, added with S1 work) | internal |
| R2 | Planar faces with holes and non-convex outlines triangulated properly in the STEP reader (needed for plates with lightening holes) | Parts > Add CAD files, 3D views |
| R3 | User model slot: "Use my crane model" picks a STEP file at run time (private, never copied into the project unless the user embeds it), with part roles (wheel, drum, hook block) mapped by name | Hoist in 3D, crane setup |

## Honesty

The picture is generic. Dimensions are the numbers the simulation uses; the details are typical of a
workshop crane. Nothing here is a structural or clearance check.

## Evidence rules

Evidence built from the exact head (HEAD.txt equal to the sha, report saying "source at git <sha>"),
1366x768 and 1920x1080, `check_layout`, regressions baselined on clean main, pictures on the project
mount under `/mnt/project-files/qa/pr<N>-<sha8>/` and only `HEAD.txt` and `report.md` committed.

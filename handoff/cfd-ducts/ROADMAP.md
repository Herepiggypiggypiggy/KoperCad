# CFD and cable ducts: roadmap

1. Duct height from the project, not 70 % of the air depth (done, this PR).
2. Slice, arrows, streamlines and probe treat a slotted duct as air (done).
3. Duct study: none, slotted, closed, real and old height (done, tests/duct_compare.rs).
4. Done in handoff/heat-wires (I²R of the wires in the ducts, 2D and 3D).
5. Next: slot share and open area as a maker value on the duct (Rail has width and height only).
6. Next: validate the porous drag against a measured slotted-duct pressure loss (planning values today).

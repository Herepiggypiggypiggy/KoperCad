# CFD and cable ducts: roadmap

1. Duct height from the project, not 70 % of the air depth (done, this PR).
2. Slice, arrows, streamlines and probe treat a slotted duct as air (done).
3. Duct study: none, slotted, closed, real and old height (done, tests/duct_compare.rs).
4. Next: heat of the wires inside a duct (needs per-wire current and routed length per duct; not in the model yet).
5. Next: slot share and open area as a maker value on the duct (Rail has width and height only).
6. Next: validate the porous drag against a measured slotted-duct pressure loss (planning values today).

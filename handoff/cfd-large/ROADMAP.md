# CFD for large cabinets: roadmap

Scope: whole-cabinet 3D CFD for real machine cabinets (CV-200: 115 devices, 9 rails, 11 ducts, 1.48 M cells at Coarse, never converged). Engine in `crates/novo-cfd/src/large/`, wired into `CabinetSolver`.

## Done
- Reduction: cold devices lumped into row blocks, planes clustered and snapped (hot devices too, shift reported), ducts and rails widened to porous strips with volume kept, pitch chosen from the cell budget with a volume-growth limit.
- Results expanded to every real device (air of the block, housing from own loss and film, wide bands), hot spots rebuilt, notes and accuracy limits extended. `ENCLOSURE_VALIDATED` stays false.
- `LargeMode` (Auto / Off / Force) on `CabinetSolver`; Auto reduces when the full grid is over 1.5 times the level budget.
- `large::convergence` verdict and `large::reduced_summary`; the Heat legend shows both.
- Tests `crates/novo-cfd/tests/large.rs`, QA scenario `qa/scenarios/40-large-cabinet-heat.qa`, example `crates/novo-cfd/examples/large_cabinet.rs`.

## Next (in order)
1. Heat-aware Place all and cooler layout should call the reduced model for a whole-cabinet check.
2. Lump with flow: keep a gap through a long row when a duct or fan jet crosses it.
3. Better mesh planner for the full scenario: planes less than 10 mm apart cascade cells through the whole domain (the existing planner gives 77 k cells for 5 bodies at a 3 000 target).
4. Validate against a measurement or a trusted reference (still none); then think about `ENCLOSURE_VALIDATED`.
5. Pitch per region (not one global pitch) once the solver supports non-tensor meshes.

## Open questions
- Is a lumped member's housing estimate good enough to flag "Over"? (48 of 115 devices came out Over at Medium on the CV-200 because the air round the block is above their 60 C limit.)

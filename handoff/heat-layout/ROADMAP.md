# Heat-optimised layout: roadmap

Scope: place devices so the cabinet runs cooler, with numbers from the app's own heat models, and say plainly what is model and what is validated (`ENCLOSURE_VALIDATED` stays false).

## Done
- PR 1: rail-aware placement search (`novo-thermal/src/placement.rs`, `placement_project.rs`), lumping helper for the 3D mesh (`lump.rs`), Cabinet entry "Cooler layout" (Heat strip button, Ctrl+K "Suggest a cooler layout", plate right-click menu), window with before/after, arrows on the plate, one undo step.
- Real machine (CV-200) heat read and layout comparison: `audit/real-machine/heat-layout-findings.md`.

## Next (in order)
1. CFD-C large cabinets: the 3D mesh cannot take a real machine cabinet (115 devices, 9 rails, 11 ducts: 1.48 M cells at Coarse, stops at the 180 s cap after 10 iterations). Merge cold devices into row blocks (use `lump.rs`), treat rails and ducts as porous or thin features, honour the cell budget.
2. Heat-aware Place all: let `novo-layout` rules use `Spec.loss_w` and the plume estimate of `placement.rs::exposure` (hot device not under a sensitive one, at the side or the top).
3. Cooling advice card in the Heat view: the fan, vent and loss advice of `novo-thermal/src/advice.rs` has no UI of its own (only shown inside Cooler layout when a device stays over).
4. Vent and fan position sweep in the 2D model (advice uses one fixed position).
5. Real losses and limits: the CV-200 parts carry no loss, size or limit (Real part data lane).

## Open questions
- Wire price (0.5 cost per metre), the largest move (900 mm) and the thin-margin band (8 K) are judgement values, not measured. The search knows no wiring: a move that lengthens many wires is not priced by connections, only by distance.

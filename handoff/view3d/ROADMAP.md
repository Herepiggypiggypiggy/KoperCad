# 3D view (WS-29): roadmap

Scope: architecture and renderer of the 3D cabinet view, crate `novo-view3d`. Design: `docs/design/3d-view.md` (section 5 is the contract the sibling threads build against).

## Done (all on main)
- Design, crate, renderer, egui widget, overlays, door animation, layers, routed wires, the enclosure's own shell and door, 3D toolbar, wire colours with stripes, frame statistics panel, dimensions, frame governor connected.

## Next, in order
1. Real numbers from the owner's Windows laptop: View > 3D > frame statistics, steps in the design doc ("Measuring on your own machine"). Then tune the tiers or the culling with those, not with llvmpipe.
2. GPU time with timestamp queries (needs the device feature; eframe's device does not ask for it yet).
3. Colour line conductors (power black, AC control red) once the model stores the circuit class. PE, neutral, 0 V and positive DC are coloured by potential already.
4. Maker meshes (geometry thread) when STEP import exists.
5. Distance by distance culling before the instance copy, if 3000 devices and 1000 wires on a real GPU ask for it (0.86 ms of renderer CPU on llvmpipe).

## Open questions
- Whether maker meshes are wanted (geometry thread decides with screenshots).

# Crane simulation example: roadmap

Scope: one shipped example project, "Overhead crane simulation (example)", that runs from the
circuit (radio, receiver, relays, contactors, hoist, bridge, crab, Hoist in 3D).

Done
- Example built on the plain crane example (`example_crane::draw(tx, sim = true)`), receiver
  blocks, interruption-point rails, motor properties, radio bindings, 3D setup.
- Registered: File > Templates and examples, Start centre, command field (`file.example_crane_sim`).
- Tests in `example_crane_sim_tests.rs` (radio lifts the hook, bridge and crab move, E-stop stops).

Next, in order
1. Travel brakes on the bridge and the crab drives (`trav_brake_*`) with a brake relay drawn.
2. Second hoist unit and a closed-loop drive hoist variant as a second sim example.
3. QA scenario with screenshots of Hoist in 3D (needs the QA harness on a machine with a display).

Open questions
- Should the plain crane example also move to interruption-point buses? (It cannot be run today.)

# Crane model picker and motion: roadmap

Scope: "Use my crane model" chain (lane R3 of crane sim realism). The person points Hoist in 3D at a STEP file of their own crane; the picture shows it; part roles drive its motion.

1. Part 1 (merged, #1965): `kopercad-cli parts step-parts` splits a STEP file into solids and guesses a role from each name (`novo-interop` `part3d::build::placed_parts`, `part3d::crane_roles`).
2. Part 2 (this thread): the picker. Simulate > Hoist in 3D > "Crane model..." reads the file on a worker thread and shows it in place of the generic crane; "Parts" lists the solids with their role, the file's up axis and a turn; roles are editable; the project keeps the path, orientation and edited roles (`crane3d.model*` info properties, one undo step "Crane picture").
3. Part 3: roles drive motion: wheels turn with travel, the drum turns with the hook, hook block and hook follow the rope, girder and trolley parts travel with bridge and trolley.

Later: colours and names from the file's own style data, a ZIP of STEP files, part motion limits from the model's rails, S4 crane kinds.

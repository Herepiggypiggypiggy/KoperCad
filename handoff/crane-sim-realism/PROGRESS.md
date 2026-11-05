2026-11-04 12:36 UTC thread started; plan from coordinator read.
2026-11-04 13:05 UTC a customer STEP file (private) inspected for generic proportions. Reader reads it; reader is fine (lane R2 dropped).
2026-11-04 13:40 UTC S1 generated crane parts written, view3d tests green.
2026-11-04 16:35 UTC S1 merged (#1933); S2 look started.

2026-11-05 S2 floor line merged (#1940); S3 state lamps (brake, drive) on the control box started.

2026-11-05 S3 lamps merged (#1952); S3b slack rope heap beside the block for a closed-loop winch.

2026-11-05 R3 part 1: `kopercad-cli parts step-parts <file>` splits a STEP model into parts and guesses roles from names (novo-interop part3d::build::placed_parts, part3d::crane_roles).

2026-11-05 R3 part 2 (PR 1971): Hoist in 3D 'Use my crane model...' picker; hook and hoist pieces swapped into the picture (novo-interop part3d::crane_model, novo-view3d crane_user, novo-app crane_user_model). Next part 3: roles drive wheels/drum/sheave motion, remember the choice.

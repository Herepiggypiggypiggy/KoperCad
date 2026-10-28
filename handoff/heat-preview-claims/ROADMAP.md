# Heat preview and lesson claims: roadmap

Scope: the Heat view's preview (the picture before Simulate) must rank devices the way the physics
and the 2D solver do, and every number or ranking a lesson quotes from an engine must be checked by a
test, so lesson text cannot drift from the engines unnoticed.

Done
- Preview kernel rebuilt (`novo-heatview/src/synthetic.rs`): device rise = loss / (0.07 + 47 x area W/K),
  air rise 0.17 K per W of total loss with mild stratification, no fictional inlet cooling, strongest
  neighbour counts in full and the others a fifth. Fitted to the 2D model on the lesson cabinet.
- Device readings come from the model at the device centre, not the hottest cell of a ring.
- `lesson_claims_tests` (novo-app): heat-and-sizing, plc-panel, cooling lessons, wire-routing,
  cut-lengths, panel-labels, door and depth, plate drawing, cabinet-layout.
- Stale lesson numbers fixed (EN and NL): wire lengths, cut list counts, label counts.
- Scene dump reports the Heat readings; QA scenario 530 walks the heat lesson at both sizes.

Next
- Claims tests for the remaining lessons that quote engine numbers: wire-numbers (toast counts),
  terminals, plc-io, work-cards (STEP n of 42), workshop-pack (@example), kitting, protection.
- Calibrate the preview against the 3D solver (Simulate) once a validated cabinet exists.

Open questions
- The preview constants rest on one cabinet (the lesson one) and the 2D model; a measured cabinet
  would replace both.

# Handoff: wheel Points wedge

Branch claude/project-thread-nkjdxi on kopercad. PR title is marked "after build": the lead holds the merge until the current build is out.

Code: crates/novo-app/src/app/wheel.rs (CATEGORIES, OTHER for the centre), test in app/gate1_tests.rs `a_held_place_key_opens_the_wheel_and_keys_pick_from_it`.

Points 14 and 16: reviewed in code. 14 is `settle` (dwell 0.18 s before the outer ring follows a new wedge, unit tested). 16 is the release handling in `place_wheel`: a release over a wedge only locks it, a release over an item picks, a key release after a flick picks only when no wedge was locked. Both unit/gate tested; only a hands-on test in a window can confirm the feel.

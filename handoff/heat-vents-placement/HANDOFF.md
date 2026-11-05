# Heat window over the Vents card at 1280x688

Regression seen after #1841 (docked strips): with the Cooling what-if window open at the left edge, the Vents and fans card started beside it (x about 508 at 1280 wide) and the Heat window at the right edge (330 wide, left of the docked strip) landed on top of it, covering "Add the filter fan and a grille".

Fix (`view3d_vents.rs`, `whatif_placement`): the card starts beside the what-if window only when it then still fits left of the Heat window; otherwise it starts under the what-if window at the left edge. The Heat width is now `pub(super)` for that check. Scenario `qa/scenarios/1604-3d-vents-beside-whatif.qa` runs it at 1280x688, 1366x768 and 1920x1080; unit test `beside_the_whatif_window_the_panel_never_meets_the_heat_window`.

Where to find it: 3D > Heat > Cooling what-if, then the Vents and fans card (3D > Heat > Vents).

Not done: a person-moved Heat window is not considered (only its default place). Scenario 160 itself was not edited and passes at all three sizes.

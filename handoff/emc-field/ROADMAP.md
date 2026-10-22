# EMC magnetic field estimate: roadmap

Scope: the low-frequency magnetic field estimate of `docs/EMC.md` (phase E4, first slice): `crates/novo-emc/src/field.rs`, rules EMC-19 and EMC-20, the field slice in the EMC views, the Magnetic field estimate section of Check > EMC check. Doc: `docs/EMC-FIELD.md`.

## Done
- Biot-Savart of straight segments with complex currents (phases), plate image, flux by the loop integral of the vector potential, Faraday; 10 closed-form unit tests plus 11 scenario tests.
- Currents from `emc.current_a` or the rated current at an end; phases from pins, DC potential or cable core order.
- EMC-19 field at a sensitive device, EMC-20 voltage induced in an analog, fieldbus or safety wire (line frequency and current step); findings with fixes (move device, twist, separate).
- Exposure map layer shows the field slice (log ramp, planning value in the middle, microtesla ends in the Legend); marker kind "Magnetic field".
- Check > EMC check: collapsible section with settings, editable currents (one undo step), sensitive devices and wires.
- QA scenarios 150 to 152, fixture `qa/fixtures/emc-field.novo`.

## Next
1. Common-mode current path for a motor cable (needs a cable capacitance model and the shield termination); the dominant real-world coupling, not covered.
2. Field contours and hot spots through `novo-heatview` (the map is a plain grid now), a section plane in 3D instead of a sheet at the wire height.
3. A what-if slider: move a wire or the PLC and see the number change live.
4. Riser wires and device heights (the engine has no heights yet).
5. Typed currents on devices from the part catalogue (drive output current, motor nominal current) instead of the generic rated current.

## Open questions
- Planning values (12.6 uT, 10 mV) are house values; replace by a maker's immunity data when a part states one.
- IEC 61000-4-8 levels are quoted from memory and need a person with the licensed text.

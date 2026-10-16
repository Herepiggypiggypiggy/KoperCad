# Example project: cabinet-ready

Scope: the built-in example (File > New / Open example) works end to end for Cabinet, 3D and heat.

Done: plate, interior, setback, right-hinged door, rails, ducts, placed devices; catalogue parts carry sizes and planning loss values; test `the_example_cabinet_is_complete_and_simulates`.

Next / open:
- Door-mounted devices (-S0, -S1) still sit on the plate with a 30 x 30 mm planning footprint; the layout engine has no "mounted on the door" exclusion (Mounting::DoorCutout exists in novo-parts only). Needs WS-15.
- The new-project template and lesson projects are separate fixtures and were not changed.
- Loss and max-ambient values in `novo-parts/catalog/starter.json` are marked `estimated`; check against maker data sheets.

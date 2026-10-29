# Route all through ducts: roadmap

- Done: wire ends on the device edge per pin (marking order) when the part has no terminal data, flagged as estimated.
- Done: a duct on the terminal's own side beats one that needs the wire to run along the device row.
- Done: wires no duct reaches are not drawn in the Cabinet (ends marked red), listed in the notes and in Issues.
- Done: duct slots (pitch or closed), set in Cabinet > Rails and ducts > Slots.
- Done: the router knows device bodies; stubs go round them; no way round is reported (Issues: Wire through device).
- Done: DIN rails with devices are routing channels (`Route::rails`), not ducts.
- Done (follow-up PR): rail channels drop a side that lies within 5 mm of a neighbouring row's devices; slot snap clamps to the last slot on the duct; duct beats rail channel at equal length (test).
- Open: duct slots are a pitch model, not maker data; the room between devices (a gap under 20 mm) is not used to pass; device depth is not in the stub path.
- Open: terminals of a part with `pin.<m>.x_mm` data on a turned device take no side (Facing::Any), so they wrap less well.

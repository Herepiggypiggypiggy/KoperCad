# Route all through ducts: roadmap

- Done: wire ends on the device edge per pin (marking order) when the part has no terminal data, flagged as estimated.
- Done: a duct on the terminal's own side beats one that needs the wire to run along the device row; turn-back detour capped at 2 x 40 mm.
- Done: wires no duct reaches are not drawn in the Cabinet (ends marked red), listed in the notes and in Issues.
- Open: ducts have no slots or openings in the model, wires enter a duct at the nearest point of its centre line.
- Open: the router does not know device bodies, so a stub to a duct beyond a neighbour can still pass it.
- Open: DIN rails are not routing edges.

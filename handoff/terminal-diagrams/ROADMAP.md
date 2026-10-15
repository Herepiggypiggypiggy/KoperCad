# Terminal diagrams: roadmap

Owner: thread "Terminal diagrams" (sibling: "Cable and terminal strip plans", cable schedule).

Goal: per terminal strip a diagram (terminal number, jumpers and bridges, wires in and out, destinations, cross references) and per connector a contact list, as live reports on screen, PDF (novo-pdf) and CSV.

Done:
- PR 1: strip rows carry wire in/out, bridged-to and location (plug list now lives in #416 of the cable thread).

Done: drawn strip diagram (strip_diagram.rs).

Next:
- Drawn strip diagram page (terminal blocks with jumper bars, wires left and right) for the PDF.
- PDF and CSV for terminal diagrams; drawer view for plugs in novo-app.
- Mating half pairing for plug/socket (same contact, two devices).

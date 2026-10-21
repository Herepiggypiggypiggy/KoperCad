# Bench packet: roadmap

Scope: printed bench output as one coherent packet for a workshop that cuts wires and drills by hand.

Done (this PR)
- `novo-reports/src/packet.rs`: circuit sheets (one tick sheet per schematic page: devices to fit, wires with ends, size, colour, cut mm, ferrules, four tests, signature) and the cover (facts, contents with page numbers, read-first notes).
- `novo-pdf`: `report_outline` gives the page each section starts on, so the cover's page numbers come from the same layout as the bookmarks.
- App: `bench_packet.rs`; Reports > Bench output > "Bench packet (PDF)", command `export.bench_packet` (Reports > Export), Workshop pack writes `0 Bench packet.pdf` and `2 Labels.pdf` (the loose cut list, ferrules and assembly files are now inside the packet and removed on rerun).

Next
1. Heading rows or a page break per circuit group (power, control, PE) inside a sheet.
2. Terminal strip plan as the drawn strip diagram (`strip_diagram`) instead of the table.
3. Print a real packet on a printer and check the 1:1 files with a rule (never done here).
4. Per-circuit device mounting position (plate x, y) on the Fit rows.

Open questions: circuit = schematic page today; a protection-device based circuit would need the potentials graph.

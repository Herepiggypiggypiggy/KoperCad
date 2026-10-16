# EPLAN-PXF-WRITE roadmap
Scope: write a KoperCAD project as PXF for EPLAN to import. Design: docs/design/eplan-pxf-write.md. Target EPLAN 2024; 2027 unknown (see design).
- [x] Slice 1: pages, devices and terminals on symbols, cables and cores; File > Export entry; kopercad-cli export eplan; round trip with the reader
- [ ] Slice 2: wires and nodes, texts, interruption points (need the O31/O42 facts from a survey of a real file)
- [ ] Slice 3: `.emp` page macros out
- [ ] First import into a real EPLAN 2024; every EPLAN complaint becomes a test
- [ ] EPLAN 2027 `Dialect` once a 2027 export has been surveyed

# EPLAN symbol art roadmap
Scope: crane import draws every placement with EPLAN's own symbol drawing and pin positions (lane 1 of faithful crane import).
Done: reader takes the drawing (lines, rectangles, arcs, circles, polygons, fixed texts) of every library variant; drawing travels on the placed symbol as property `art.drawing` (`novo_symbols::drawn`); canvas, PDF and every `LibraryArt` user draw it; pins and origin come from the file; placement matrix applied; report says art/pins counts.
Next: line types (dashes) and fills of rectangles (attributes A411/A412, A1653..A1656 not decoded); text alignment and rotation of fixed texts; pin-designation texts beside pins (EPLAN shows them from placeholder texts); same-drawing sharing to shrink files; v68b run.
Open: a project-level store of drawings so 5000 placements do not each carry a copy (about 0.5 kB each today).

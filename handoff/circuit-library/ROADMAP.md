# Circuit library: roadmap

Scope: KoperCAD's own generic standard circuits as plain TOML data, grouped in
chapters, placeable from the Place macro window and the Macros browser, with a
mounting view per circuit. Design and format: `docs/CIRCUIT-LIBRARY.md`.

Done: format, build-time checks, 14 circuits in 7 chapters, mounting data,
Place window headings and mounting view, CLI `macros install-circuits`.

Next:
1. More circuits: pendant two-step station, safety relay variant, bridge crane
   cross travel pair, anti-condensation heater, cabinet fan with thermostat.
2. Coordinate with the place-palette thread (PR #1681) so chapters show there.
3. Crane template thread: reuse circuits as building blocks.
4. Mounting view into the cabinet layout (place the plate devices as a start).

Open: none that block.

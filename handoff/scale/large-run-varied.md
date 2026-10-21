| Step | Time (ms) | Notes |
| --- | ---: | --- |
| generate | 125.5 | 600 pages, 2676 devices, 8513 wires, 6418 symbols, 23385 records |
| save full | 149.5 | 7.0 MB on disk |
| to_json | 33.5 | 9.2 MB |
| from_json | 73.5 |  |
| open file | 119.4 |  |
| netlist (cold) | 20.9 | 4736 nets |
| netlist (again, cached?) | 21.6 |  |
| search tag | 5.3 | 1 hits for =A1+CAB15-F451 |
| search word | 5.1 | 0 hits |
| cross references of one device | 0.0 |  |
| page scene (one page, placeholder art) | 1.6 | 30 items; model side only, not a frame |
| page scene + tessellate (one page) | 0.5 | 2364 indices; CPU triangles, no GPU |
| page scene + tessellate (every page, symbol art) | 90.3 | 600 pages, 15885 items; slowest page 0.88 ms (page 3); CPU only |
| edit one symbol | 0.1 |  |
| undo edit | 0.0 |  |
| autosave one edit | 12.3 |  |
| number wires (plan, fill) | 47.2 | 8513 wires |
| number wires (plan again) | 46.2 |  |
| number wires (apply) | 28.3 |  |
| design checks | 999.8 | 6204 problems |
| wire cut jobs | 115.8 | 8159 jobs |
| report toc | 1.4 | 629 rows |
| report bom | 17.0 | 44 rows |
| report devices | 23.4 | 2676 rows |
| report wires | 110.4 | 6665 rows |
| report cutlist | 136.2 | 8165 rows |
| report wirecut | 86.4 | 8342 rows |
| report terminals | 77.9 | 1684 rows |
| report cables | 51.4 | 885 rows |
| report io | 18.4 | 0 rows |
| report cableschedule | 52.5 | 708 rows |
| report plugs | 0.1 | 0 rows |
| report labels | 87.5 | 20855 rows |
| autopilot plan | 225.9 | 14931 proposals |
| xref index | 12.0 |  |
| pdf export (all pages) | 144.0 | 7.2 MB, 600 pages |
| route all wires (whole project) | 14.9 | 29 cabinets routed |
| 3d scene + wires (one cabinet) | 24.7 | 79 items, 56 wires |
| 3d scenes (all cabinets, routed) | 623.3 | 29 cabinets |

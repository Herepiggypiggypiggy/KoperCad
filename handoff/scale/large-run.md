| Step | Time (ms) | Notes |
| --- | ---: | --- |
| generate | 94.1 | 600 pages, 2130 devices, 8100 wires, 6000 symbols, 21015 records |
| save full | 133.7 | 6.6 MB on disk |
| to_json | 31.3 | 8.6 MB |
| from_json | 58.7 |  |
| open file | 96.8 |  |
| netlist (cold) | 17.6 | 4800 nets |
| netlist (again, cached?) | 16.7 |  |
| search tag | 2.3 | 1 hits for =A1+CAB16-K151 |
| search word | 2.0 | 0 hits |
| cross references of one device | 0.0 |  |
| page scene (one page) | 0.3 | 30 items |
| page scene + tessellate (one page) | 0.2 | 2364 indices |
| page scene (all pages) | 42.9 | 600 pages, 15300 items |
| edit one symbol | 0.0 |  |
| undo edit | 0.0 |  |
| autosave one edit | 11.2 |  |
| number wires (plan, fill) | 49.7 | 8100 wires |
| number wires (plan again) | 56.5 |  |
| number wires (apply) | 27.7 |  |
| design checks | 857.4 | 2762 problems |
| wire cut jobs | 66.7 | 7500 jobs |
| report toc | 1.0 | 630 rows |
| report bom | 7.9 | 9 rows |
| report devices | 11.2 | 2130 rows |
| report wires | 70.1 | 5700 rows |
| report cutlist | 103.5 | 7502 rows |
| report wirecut | 111.0 | 7506 rows |
| report terminals | 66.9 | 1200 rows |
| report cables | 66.2 | 1500 rows |
| report io | 39.0 | 0 rows |
| report cableschedule | 103.8 | 1200 rows |
| report plugs | 0.2 | 0 rows |
| report labels | 105.7 | 18630 rows |
| autopilot plan | 216.7 | 12600 proposals |
| xref index | 9.6 |  |
| pdf export (all pages) | 188.1 | 7.1 MB, 600 pages |
| route all wires (whole project) | 13.5 | 30 cabinets routed |
| 3d scene + wires (one cabinet) | 28.5 | 70 items, 80 wires |
| 3d scenes (all cabinets, routed) | 787.6 | 30 cabinets |

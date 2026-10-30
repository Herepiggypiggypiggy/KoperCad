# TraceParts lane L2: hand-off flow and STEP footprint (2026-10-30)

Plan: `handoff/traceparts/PLAN.md` in the project files. No key, no scraping, no sign-in by KoperCAD.

## Built
- `novo-parts/src/tp_handoff.rs` (pure, no network):
  - `search_url(mpn)`: link to TraceParts' search for the part number. The `KeyWords` form is NOT verified with a real session (traceparts.com is not reachable from the cloud sandbox); `HOME_URL` is the fallback.
  - `TracePartsWatcher`: polls Downloads for finished STEP files and ZIPs newer than the click. TraceParts names files by its own ids, so the part number need not be in the name (`Arrival::names_the_part` says whether it is). The caller must judge content with `cadmatch` (`CadIdentity::read`, `decide_drop`) before taking a file.
  - `file_into_cache` / `read_model`: takes the STEP from the file or ZIP (in memory, no path extraction, size capped) into `cache_dir()` = `%LOCALAPPDATA%\KoperCAD\cache\traceparts\<mpn>\<mpn>-<hash>.step` plus `.source.json` (part number, download name, sha256, source). Same bytes are kept once.
- `novo-interop/src/part3d/mounting.rs`: `scan_mounting` returns the footprint (outline width x height), depth and the fixing holes in the back face (2.5-14 mm), from the bottom-left of the outline; `MountingScan::on_plate(origin, quarter_turns)` puts them on a plate for a drilling plan.

## Audit result (STEP reading)
- `measure_step` bbox is from tessellated solids, fine (construction geometry excluded).
- `pins.rs` deliberately dropped openings facing the mounting plate, so fixing holes were never read. Now `mounting.rs` reads them.
- Mounting side is chosen by the larger flat face (`mounting_side_is_top`): a part whose back has many holes can be taken the wrong way up; unchanged, noted.
- Holes are reported as through holes; blind/tapped cannot be told from the model.

## For L3 (Parts window, not done here)
Button "Find CAD on TraceParts": open `search_url`, start `TracePartsWatcher::new(downloads, now, mpn)`, poll from the UI timer, on each arrival run `CadIdentity::read` + `decide_drop`, then `file_into_cache(cache_dir(), mpn, path)` and link the file to the part. L1's API client should download into the same `cache_dir()` layout.
Not wired into the app yet, so no user-visible entry from this PR.

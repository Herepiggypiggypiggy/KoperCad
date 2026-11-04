# PR 1862 evidence (code head 27a53cb144da60342335516af4d422df4c0a6e12)

Pictures and the kept drill plan PDFs are in this folder (shots/, files/). Binary built at the code head by scripts/qa.sh; report.md says "source at git 27a53cb1cd".

## Where the user finds each feature
- Rittal download: Parts window, select a Rittal part, Import. Scenario 1862-rittal-parts-window, 1366x768 and 1920x1080 (shots part-selected, waiting-with-steps).
- Maker plate: Cabinet, Plate setup, Inspector, "Maker plate", Use. Scenarios 1862-rittal-plate-drill-plan (1920x1080) and -1366 (1366x768).
- Drill plan with the plate's holes: Cabinet, Export (More at 1366), Drill plan (hand, PDF). The kept PDF says "The plate comes with 5 hole(s) from the maker" and draws light rings.

## What the download does
Import opens https://rittal.partcommunity.com/3d-cad-models/?catalog=rittal&part=<order> in the system browser (the link form is unverified). The user signs in and downloads STEP. KoperCAD only watches the user's Downloads folder, imports the ZIP into the per-user parts library (%LOCALAPPDATA%\KoperCAD, nothing outside the user profile, no admin) and reads the STEP. It never signs in, scripts the portal or stores a password. Offline: the page does not open; the waiting state times out after the normal wait limit and nothing is imported. A ZIP already on disk can be added with Add CAD files.

## Not tested by the QA run
- A real browser download through the watcher (the harness cannot sign in to PARTcommunity).
- The scenarios use a hand-made catalog (qa/fixtures/rittal-parts.json), not Rittal data.
- Unit tests use hand-made geometry. Separately, the reader was run by hand on three real Rittal downloads from the owner (private, not in the repo): see handoff/rittal-parts in the project mount.
- Windows not run (Linux QA only).

## Limits of the plate holes (also printed on the drill plan and in docs/RITTAL.md)
- Check one hole against the real plate before drilling: positions follow the maker's model, which may be turned or mirrored.
- Countersunk and blind holes are listed with their front diameter.

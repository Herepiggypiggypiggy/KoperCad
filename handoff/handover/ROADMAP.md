# Handover package: roadmap

Scope: one command builds the customer folder (round 7 idea 36, /mnt/project-files/audit/creative/round7.md).

## Done (first PR)
- Engine `novo_reports::handover`: contents page with the "exact or estimated" table (`truth_rows`), revision table.
- App `app/handover.rs`: `export.handover`, File > Export reports > Export handover package (folder). Folder `<project> handover <date>`.
- Files: 0 Contents, 1 Drawing set, 2 Revisions, 3 Parts list, 4 Wire list, 5 Terminal plan, 6 Cable schedule, 7 Commissioning record, 8 Safety functions, 9 Lists.xlsx. A file with nothing to say is left out and the contents page names why.

- Review round 1: standings Calculated/Stated (no "Exact" for wire lengths, checks, issued state), basis counts, stale files removed on rerun, folder name cleaned, drawing set via print setup + one PDF per plot set, Workshop/ subfolder.

## Next, in order
1. Optional extras with tick boxes: 3D picture, heat verdict page (needs a finished Heat run), change report since the previous issued revision.
3. A dialog before writing: tick what goes in, pick the folder, show the exact/estimate table first.
4. As-built step: offer to Issue revision and lock all pages from the same dialog (reuse Revisions issue, `issue_revision_locked`).
5. Zip option for e-mail.

## Open questions
- Should the folder name use the latest revision index instead of the date?

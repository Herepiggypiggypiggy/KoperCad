# Workshop and field mode (gap 12, docs/AMBITION-REVIEW.md brief 12)

Status: planned, not started (paused 2026-10-15 at the 80% usage limit).

Complete means: assembly instructions from the layout, tablet commissioning with photos and signatures,
as-built redlines that become revisions, a shop-floor print pack.

## Pieces, in build order
1. **Assembly instructions** (novo-reports, next to `assembly.rs` from #451): a numbered build sequence per
   cabinet (cut rails/ducts, drill, mount rails, ducts, devices, wire, label, check) with tools, drill sizes
   and torque values per step. Tests: step order on the Gate 1 fixture.
2. **Shop-floor print pack**: one PDF bundling plate drawing, assembly instructions, wire list, label list and BOM,
   one cabinet at a time (build on #513 export).
3. **Field results** (novo-commissioning): `Photo` and `Signature` records on checks and punch items (file hash
   plus name, stored in the side tables), export of a field bundle (JSON + photos) and re-import with merge by
   target and check, latest timestamp wins. Tests: export/import round trip.
4. **Tablet layout** (novo-app, Commissioning workspace): large touch targets, one target per screen, swipe next,
   photo attach, signature pad. Builds on #494 (Large touch switch) and #503.
5. **As-built redlines** (novo-revisions + novo-app): mark deviations on a page during commissioning
   (redline note anchored to an object); "Apply as-built" turns accepted redlines into model commands and issues
   a revision. Tests: redline to revision round trip.

## Where the user finds it
Commissioning workspace (tablet layout, redlines) and Reports > Assembly (instructions, print pack).

## Validation
Step order tests on fixtures; export and re-import of field results; walk-through in a tablet-sized window.

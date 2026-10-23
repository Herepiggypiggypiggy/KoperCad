# Drill and fan animations

Restrained, informational motion; Preferences > Reduce motion turns it all off. The test harness (`--control`) shows still pictures unless `KOPERCAD_MOTION=1`.

- PR 1 (branch claude/fan-motion): fans turn (capped rate, grows with datasheet flow) and faint dashes cross openings in the airflow direction. Code: `novo-view3d/src/motion.rs`, `app/motion.rs` (clock), `heat_openings::overlays_at`. User finds it: 3D > Heat with a fan or filter fan opening.
- Next: drill cue when placing drilled objects (cabinet and 3D); QA scenarios 301+.

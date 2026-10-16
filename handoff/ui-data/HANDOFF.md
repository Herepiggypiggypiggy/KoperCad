# Handoff: UI integration of data features

Table of data feature versus where the user finds it: FEATURES.md. Menus come from the Workspaces `menus.rs` PLACEMENTS table, not from this thread.
Done: DXF background move/scale panel (`app/background_panel.rs`, #497). Add its `view.background_place` row to PLACEMENTS when menus.rs lands (View or File > DXF background).
Open: part property-sheet panel after the Parts window (#429); 3D model import entry (#449, held); revision compare window belongs to Revision tracking.
Resume: `cargo test -p novo-app background`.

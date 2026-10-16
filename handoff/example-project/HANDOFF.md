# Handoff: example project

Branch claude/project-thread-yv2sbr, PR from main (see PROGRESS.md for SHA).
Code: `crates/novo-app/src/app/example.rs` (`give_parts_their_data`, `add_cabinet`), `example_tests.rs`, `novo-layout/src/model.rs::device_size` (device own width/height beat parts, used by the -X1 strip), `novo-parts/catalog/starter.json` (power_loss_w, max_ambient_c, pushbutton sizes).
The outline filing already creates the +CAB structure; add_cabinet reuses it and sets enclosure = true.
Place all on the example now says "Every device is already on the plate".
Protection tests changed: catalogue ratings now give the example 3 protective devices.

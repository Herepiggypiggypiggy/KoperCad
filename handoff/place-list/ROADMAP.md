# Place from a pasted list (workflow-gap row 6)

Scope: Ctrl+K `place list` opens a window; pasted lines become devices, previewed first.

Done: parser and resolver (`novo-parts::place_list`), window and one-undo placement
(`novo-app/src/app/place_list.rs`), command `place.list`, QA scenario 9810.

Next, in order:
1. Put a "Place from list" entry in the Q place palette footer (today Ctrl+K only).
2. Excel file picker next to the paste box (cells already work when pasted).
3. Wires between placed devices are not drawn; this only places.
4. Real maker part lists from Tadjiro's shop to tune the word matching.

# Macros: roadmap

Scope: reusable circuits saved to a per-user library, placed with automatic tags and wire numbers,
with preview, browser, rename/delete and file import/export. Design: `docs/design/macros.md`.

Split agreed with the Macro library thread (session_01TWr8q1k7WUQn2qomcUP6B6): it owns the .kcmacro file and
library folder (module `macros` in novo-parts); this thread owns capture, placement, renumbering, wire
numbers, parameters, preview, parts and cabinet carry-over.

Next, in order (each a small draft PR):
1. Design note (PR #396).
2. Placement helper in novo-workflow: wire numbers, preview summary.
3. Parts and cabinet offsets.
4. App: save macro, browser drawer, ghost preview (thin).
5. Terminals inside macros (WS-02 review), update from library.

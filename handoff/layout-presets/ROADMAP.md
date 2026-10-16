# Layout presets: roadmap

Scope: named workspace layouts (Draw, Cabinet, 3D and heat, Review, Reports), kept per user, per preset and per window size. Floating-panel position and size belong to the panels thread (panel.rs), not here.

Done
- Slots and size classes (compact < 1400 pt, regular < 1700, wide), settings keys `<slot>/<class>`, legacy keys as fallback.
- Commands `layout.draw|cabinet|3d|review|reports|reset|reset_all`, View > Layouts.
- Store on switch, on size-class change and on save.

Next
- Show the current preset in the status bar / switcher tooltip.
- Remember floating panel rectangles per slot (needs panel.rs API, ask the panels thread).
- Height-based class for very short windows.
- Keys for presets (Workflow thread owns the keymap).

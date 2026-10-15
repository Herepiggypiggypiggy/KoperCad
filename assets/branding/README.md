# Branding assets

| File | Use |
| --- | --- |
| `icon.svg` | Logo and app icon source. Edit this one. |
| `icon-<size>.png` | Rendered app icon, 16 to 256 px. |
| `lockup.svg`, `lockup.png` | Proposed KoperCAD lockup (mark, wordmark, "by KoperVonk"). Mock-up only: not used by the app yet; the PNG is rendered from the SVG with Inter. |
| `icon.ico` | Windows icon (all sizes in one file) for the exe, installer and shortcuts. |

The mark is "Nova": novo means new, and a nova is a new star. Four tapered
rays meet at a copper junction dot, the dot that joins wires on a schematic.
Colours come from `crates/novo-theme/src/tokens.rs` (copper `ACCENT`
`#C88A5E`, `TEXT` `#E3E1DC`, tile `#151517`).

Alternatives considered in round two (Phase, Ground) are in
`assets/branding/alternatives/`.

After changing `icon.svg`, re-render the PNG and ICO files from the
repository root:

```
cargo run --release --manifest-path tools/brand-render/Cargo.toml -- assets/branding/icon.svg 16,24,32,48,64,256
```

`novo_theme::brand::paint_mark` draws the same mark with egui shapes; keep
its coordinates in step with `icon.svg`.

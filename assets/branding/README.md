# Branding assets

| File | Use |
| --- | --- |
| `icon.svg` | Logo and app icon source. Edit this one. |
| `icon-<size>.png` | Rendered app icon, 16 to 256 px. |
| `icon.ico` | Windows icon (all sizes in one file) for the exe, installer and shortcuts. |

The mark is an "N" drawn as an open contact: two posts and an amber blade
that pivots on the left post and stops short of the right one. Colours come
from `crates/novo-theme/src/tokens.rs` (`BRAND` amber `#FFB81C`, `TEXT`
`#E4E7EC`, tile `#171A1F`).

After changing `icon.svg`, re-render the PNG and ICO files from the
repository root:

```
cargo run --release --manifest-path tools/brand-render/Cargo.toml -- assets/branding/icon.svg 16,24,32,48,64,256
```

`novo_theme::brand::paint_mark` draws the same mark with egui shapes; keep
its coordinates in step with `icon.svg`.

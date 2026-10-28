# Handoff
- `novo_parts::user_catalog_path` now asks `novo_storage::profile::root()` first (`<profile>/roaming/KoperCAD/parts.json`). Portable installs set the root in `apply_profile_flag`, so they follow too.
- `recipes/apply.rs` `read_price`: splits `/` or ` per ` basis, uses `tablepreview::price_basis`, warns on `1.500`, refuses <=0 and >1e8, refuses non-piece units (price per metre) because `Price` stores pieces only.
- Tests: `tests/profile.rs`, `tests/price_units.rs`.

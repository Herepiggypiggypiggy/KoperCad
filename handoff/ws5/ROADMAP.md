# WS-05 symbols: roadmap

## Scope

The `novo-symbols` crate covers:
- IEC 60617 schematic symbols stored as TOML data: connection points, anchors, artwork, function type and behavior slot.
- Loading and validating those symbols.
- SVG previews.
- Palette data: aliases and favourites.

It does not depend on `novo-core`. The format reference is `crates/novo-symbols/FORMAT.md`.

## Done

| PR | What it delivered |
| --- | --- |
| #2 | Format v1 and 126 symbols |
| #12 | Validator hardening; `bridges()` and `pass_through()` |
| #15 | `PassThrough` newtype; changeovers split into common-break and common-make |
| #18 | 148 symbols; conductor poles may join 2 or more connections (bridge bars); private `PassThrough` field |
| #25 | `aliases`, `favorite`, `Library::by_alias` and `favorites()`, `LoadError::DuplicateAlias`; terminal end plate and end bracket (150 symbols); contact `center_off` for the I-0-II switch; clamp consistency test |
| #37 | Review follow-ups to #25: case-insensitive `by_alias`, `accessory.terminal.*` root, stronger clamp test, docs. Merged. |

## Next, in order

1. **Small alias requests** go in a new PR from main.
   - WS-08 asked, at low priority, for `arrow` as an alias on `connectivity/interruption_point`, which already has `ip`.
2. **Human visual check of the delay arcs** on the timer contacts (`contacts/*_delay_*`, `coils/timer_*`) against IEC 60617 S00243 to S00246. Tadjiro said the gallery "looks good", but the arcs were never checked one by one.
3. **When WS-19 (novo-sim) starts:** settle the simulation meaning of the behavior parameters with them. WS-19 owns those meanings.
   - Parameters: `center_off`, `delay`, `latching`, breaker `trip`, `passive.type`.
   - The validator in `src/validate.rs` (BEHAVIOR_SCHEMA) must accept any new keys WS-19 adds.
4. **Later phases:**
   - An NFPA standards pack under `symbols/nfpa/` (CLAUDE.md says IEC first, NFPA later).
   - Pneumatic and hydraulic symbols if the roadmap asks for them.
   - Cabinet-layout macros, which are not schematic symbols.
   - A per-user symbol library folder loaded with `Library::load_dir_lenient`, layered over the built-in set by the editor (WS-06). Favourites stay per user and are never written into installed files (no admin rights).
5. **Possible format v2 items**, none requested yet:
   - Variants: one symbol with several pole counts.
   - Per-connection potential type (AC/DC).
   - Localised names.

## Open questions

- Should user and company libraries be able to override built-in aliases? The editor decides this; FORMAT.md says so.

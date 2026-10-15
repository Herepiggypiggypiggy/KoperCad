# Heat model and cooling: roadmap

Scope: R3 of `docs/PRO-PANEL-GAP-ANALYSIS.md`. Enclosure heat balance is owned by the
Enclosure sizing helper (`novo_layout::sizing`, PR #402); this thread owns per-device heat,
the plate hot-spot map and legend, and `docs/design/thermal.md`.

## Done
- Design doc; `novo-layout::thermal` (sources, heat map, legend, ranking, top-third share, loss bridge).

## Next
1. Heat overlay scene and legend in `novo-canvas` (bands plus marks, not colour alone).
2. (done) App "Heat" toggle and legend strip on the Cabinet canvas. Next: palette command.
3. Result strip from `sizing::thermal` after #402 merges; agree files with the sizing thread.
4. Vertical gradient / fan or cooler position if the mean is not enough.

## Open
- Part keys `power_loss_w`, `load_factor` (Part power loss thread).
- Bottom-face factor: sizing uses 0.5, some handbook forms use 0. To verify.

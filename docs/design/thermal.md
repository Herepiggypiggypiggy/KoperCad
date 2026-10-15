# Heat model and cooling (R3)

How KoperCAD estimates whether a cabinet gets too hot, and how much cooling
it needs. Plan item R3 of `docs/PRO-PANEL-GAP-ANALYSIS.md`. The engine is
`novo-layout::thermal` (pure functions, no UI); the Cabinet canvas draws the
hot-spot map from it. Where this file and `docs/DESIGN.md` disagree, DESIGN
wins.

## 1. What the designer asks

"Do I need a fan or an air conditioner, and where will it be hot?" The answer
is a handful of numbers and one picture, with the assumptions visible. It is a
**planning value**: the enclosure or cooling-unit maker's own sizing tool or
datasheet is the authority, and the UI says so next to every result.

## 2. Split of work and standard followed

- **Enclosure heat balance** lives in `novo_layout::sizing` (Enclosure sizing
  helper thread, PR #402): `effective_surface` (IEC 60890 surface factors per
  face), `total_loss`, `thermal` (dissipation, rise, inside temperature,
  cooling needed, fan airflow) and the material constants. This document does
  not duplicate those formulas; section 4.1 only records what to verify.
- **This thread adds** per-device heat on the plate: sources, the loss-density
  map, legend, hot-device ranking and the share of heat high on the plate
  (`novo_layout::thermal`), and the Cabinet canvas overlay.
- **Not claimed:** the full IEC 60890 type-test extrapolation (temperature rise
  at 0.5 and 1.0 of the enclosure height from the `k` and `x` curves). The
  tables were not available, so they are not reproduced from memory. The
  balance gives a **mean interior temperature**; the top of a cabinet is
  warmer, which is why the map and the top-third share sit next to it.

## 3. Inputs

| Input | Where it lives | Default |
| --- | --- | --- |
| `power_loss_w` per part (W) | part property (Part power loss thread) | missing = **unknown**, never 0 |
| `load_factor` per part, 0 to 1 | part property | 1 |
| Enclosure size, installation, material, temperatures | enclosure data, see `sizing` | from the sizing helper |

Effective loss of a device is `power_loss_w * load_factor`, the same rule as
`sizing::total_loss`. The load factor is the designer's statement of how much
of the rated loss is present on average; the engine does not scale by I squared
because the load current is not known here. A device with no loss data is
listed as unknown and left out of the map; results that depend on the total are
shown as "at least" values.

## 4. Formulas

### 4.1 Enclosure balance (in `sizing`, summarised here to verify)

`Ae = sum(area * b)` with `b`: top 1.4 (0.7 covered), front, rear and sides 0.9
(0.5 covered), bottom factor per `sizing` (0.5 there; some handbook closed
forms, such as `1.8*H*(W+D) + 1.4*W*D`, leave the floor out, a difference of
`0.5*W*D`). Wall loss `k * Ae * (T_max - T_ambient)`, rise `Pv / (k * Ae)`,
cooling `max(0, Pv - wall loss)`, fan flow about `3.1 * cooling / dT` m3/h at
sea level (`3600 / (rho * cp)` with `rho = 1.146 kg/m3`). The bottom-face
factor and `k` are planning values to check against IEC 60890 and a maker
handbook before the UI shows numbers.

### 4.2 Hot-spot map

Aim: show where heat concentrates on the mounting plate, not a temperature.

1. Each placed device with known loss spreads its effective loss evenly over
   its footprint: loss density in W/dm2.
2. The plate is cut into square cells (default 10 mm). A cell's value is the
   sum over devices of density times the fraction of the cell the footprint
   covers.
3. A box blur of radius 40 mm (default) spreads each cell's heat to its
   neighbours, conserving the total, so two hot devices next to each other
   read hotter than the same two far apart.
   Cells are at least 5 mm and the grid is coarsened to at most 40 000 cells.
   Heat of a device that reaches past the plate is not mapped.
4. Cells fall into five bands with fixed limits in W/dm2: below 0.5, up to 2,
   up to 5, up to 10, above 10 (defaults, adjustable). The legend lists the
   limits. Fixed limits mean two plates are comparable and an empty plate
   never reads "hot".

Hot devices are ranked by share of the total loss, and `top_third_share`
reports how much of the placed loss sits in the upper third of the plate
(hot air collects at the top, so a high share is a layout problem R7 can act on).

## 5. Where it shows

- Cabinet canvas: a "Heat" overlay toggle (header button, and the palette
  command `cabinet.layer.heat`, also found by "heat" or "warmte"; it is a
  per-session view option, not saved with the project) draws the map under the devices,
  with a legend of the bands and the balance numbers from `sizing` in a small
  strip. Colour is never the only carrier: the legend names each band and
  devices above the top band get a mark.
- Issues: one Problem in `novo-checks` ("needs about N W of cooling, or a fan
  of about V m3/h"), owned with the sizing helper.
- Always next to a result: "Planning value. Check with the enclosure maker."

## 6. Plan

1. This document and `novo_layout::thermal` (sources, map, legend, ranking,
   model bridge for losses) (PR #406).
2. Heat overlay scene and legend in `novo-canvas`, then the app toggle.
3. Join with `sizing::thermal` for the result strip once #402 merges.
4. Fan or cooler position and a vertical gradient inside, if the planning
   value needs more than the mean.

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

## 2. Standard followed, and what is not claimed

- **Effective cooling surface** follows the surface-factor method of
  IEC 60890 (Annex on effective area): each outer face of the enclosure counts
  with a factor `b` that depends on how freely air reaches it.
- **Heat balance** is the steady-state balance used in maker handbooks
  (Rittal climate-control handbook, listed in the gap analysis): heat leaves
  through the walls in proportion to the effective area and the temperature
  difference.
- **Not claimed:** the full IEC 60890 type-test extrapolation (temperature
  rise at 0.5 and 1.0 of the enclosure height from the `k` and `x` curves).
  The curve tables were not available to the implementing thread, so they are
  not reproduced from memory. The result is a **mean interior temperature**
  estimate; the top of a cabinet is warmer than that value, which is why the
  hot-spot map and the "top third" share are shown next to it.

If a later thread adds the IEC 60890 curves, they replace section 4.2's
`delta_t` and nothing else changes.

## 3. Inputs

| Input | Where it lives | Default |
| --- | --- | --- |
| Loss `power_loss_w` per part (W) | part property (from the Part power loss thread) | missing = **unknown**, never 0 |
| `load_factor` per part, 0 to 1 | part property | 1 (full loss) |
| Interior size `width_mm`, `height_mm`, `depth_mm` | enclosure `Structure.properties` | outer size guessed from the plate (+100 mm each way, 250 mm deep), flagged **guessed** |
| `installation` | enclosure property | `free_standing` |
| `top_covered` | enclosure property | false |
| `material` | enclosure property | `sheet_steel` |
| `ambient_c`, `max_internal_c` | enclosure property | 35 and 40 degrees C |
| `altitude_m` | enclosure property | 0 |

Effective loss of a device is `power_loss_w * load_factor`. The load factor is
the designer's statement of how much of the rated loss is present on average
(a motor starter idles most of the day); the engine does not scale by I squared
on its own, because that needs the load current, which is not known here.
Enclosure dimensions are exterior dimensions (width W, height H, depth D) as
the maker lists them.

## 4. Formulas

All lengths in metres inside the formulas, areas in m2, temperatures in
degrees C, power in W.

### 4.1 Effective surface `Ae`

`Ae = sum(A_face * b_face)`, with:

| Face | Area | `b`, free | `b`, covered (against a wall, neighbour or ceiling) |
| --- | --- | --- | --- |
| Top | `W * D` | 1.4 | 0.7 |
| Front | `W * H` | 0.9 | n/a (the door is always free) |
| Rear | `W * H` | 0.9 | 0.5 |
| Left, right | `D * H` | 0.9 | 0.5 |
| Bottom (floor) | `W * D` | 0 | 0 |

Installations are presets of which faces are covered:

| `installation` | Covered faces |
| --- | --- |
| `free_standing` | none |
| `wall` | rear |
| `row_end` | one side |
| `row_end_wall` | rear, one side |
| `row_middle` | both sides |
| `row_middle_wall` | rear, both sides |

`top_covered` additionally covers the top (cabinet under a ceiling or a
canopy). Check: a free-standing cabinet gives `1.8*H*(W+D) + 1.4*W*D` and a
wall-mounted one `1.4*W*(H+D) + 1.8*D*H`, the well-known closed forms.

### 4.2 Heat balance

- Total loss `Pv = sum(power_loss_w * load_factor)` over devices in the
  enclosure.
- Wall heat transfer coefficient `k` by material, in W/m2K (planning values,
  surface emission included): painted sheet steel 5.5, stainless steel 4.5,
  aluminium 12, plastic 3.5.
- Heat the walls shed at the permitted interior temperature:
  `P_wall = k * Ae * (T_max - T_ambient)`. Negative when the interior must be
  colder than the room (then everything has to be cooled).
- Interior temperature rise without cooling (mean): `dT = Pv / (k * Ae)`;
  interior temperature `T_ambient + dT`.
- **Required cooling power** `Pk = max(0, Pv - P_wall)`.
- **Filter-fan airflow**, only when `T_max > T_ambient`:
  `V = 3600 * Pk / (rho * cp * (T_max - T_ambient))` in m3/h, with
  `cp = 1005 J/kgK` and air density `rho = 1.146 kg/m3 * exp(-altitude/8434)`
  (density at 35 degrees C, sea level, thinning with height). This gives the
  usual `3.1 m3K/Wh` factor at sea level. A fan can never cool below ambient;
  when `T_max <= T_ambient` no airflow is given and the verdict says a cooling
  unit is needed.
- Verdict: `Ok` (`Pk = 0`), `NeedsCooling`, or `Incomplete` when any device in
  the enclosure has no loss data or the size is guessed. An incomplete result
  shows the partial numbers marked "at least" and lists the devices without
  data. It is never presented as a final answer.

### 4.3 Hot-spot map

Aim: show where heat concentrates on the mounting plate, not a temperature.

1. Each placed device with known loss spreads its effective loss evenly over
   its footprint: loss density in W/dm2.
2. The plate is cut into square cells (default 10 mm). A cell's value is the
   sum over devices of density times the fraction of the cell the footprint
   covers.
3. A box blur of radius 40 mm (default) spreads each cell's heat to its
   neighbours, conserving the total, so two hot devices next to each other
   read hotter than the same two far apart.
4. Cells fall into five bands with fixed limits in W/dm2: below 0.5, up to 2,
   up to 5, up to 10, above 10 (defaults, adjustable). The legend lists the
   limits. Fixed limits mean two plates are comparable and an empty plate
   never reads "hot".

Hot devices are ranked by share of the total loss, and `top_third_share`
reports how much of the placed loss sits in the upper third of the plate
(hot air collects at the top, so a high share is a layout problem R7 can act on).

## 5. Where it shows

- Cabinet canvas: a "Heat" overlay toggle draws the map under the devices,
  with a legend of the bands and the numbers of section 4 in a small strip.
  Colour is never the only carrier: the legend names each band and devices
  above the top band get a mark.
- Issues: one Problem in `novo-checks` ("needs about N W of cooling, or a fan
  of about V m3/h"), later, after the engine merges.
- Always next to a result: "Planning value. Check with the enclosure maker."

## 6. Plan

1. This document and `novo-layout::thermal` with the model bridge (this PR).
2. Heat overlay and legend on the Cabinet canvas (`novo-canvas` colour bands
   scene, then the app toggle).
3. Enclosure inputs in the inspector; the Problem in `novo-checks`.
4. IEC 60890 curves, if the tables become available.

Dependencies: the `power_loss_w` and `load_factor` part keys come from the Part
power loss and clearances thread; until they land the keys are read by name from
part properties, so a hand-typed property already works. The enclosure inputs are
plain properties on the enclosure `Structure`, so no `novo-core` change is needed.

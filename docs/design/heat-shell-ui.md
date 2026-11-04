# Heat shell UI (lane H7)

Status: design, written against the H1 contract proposal in `handoff/heat-research/ROADMAP.md`. No engine change here. Today the shell is one IEC 60890 number per face (`ThermalBc::Overall`, `Params::wall_k_scale`); wall temperatures do not exist yet.

## Rules

- On-screen text: values, units and names only. Every "model vs exact", "assumed" and "why" sentence sits behind a `?` (hover). Almost nothing else is explained on screen.
- Colours from `crate::theme::ui_palette()` and the `novo_theme::heat` scale, never `tokens::*`.
- A value is badged **model** until H6 has a passing exact or measured case for that term. The badge is a small dot with `?`, not a sentence. IEC 60890 totals carry **IEC**. Nothing says "validated" while `ENCLOSURE_VALIDATED` is false.
- Setup edits go through commands (undo/redo), stored on the enclosure in the project, like the other Cabinet properties.

## Where the user finds it

1. **Setup**: Cabinet workspace, select the enclosure, Properties, section "Shell". (Sizing window keeps its catalogue material column; Shell is the detailed edit.)
2. **Wall, door, plate temperatures**: Heat layer, Cabinet 2D (outline tint on the six faces and a plate rear swatch) and 3D view (face colour from the same scale as the air, outer and inner toggle in the Heat key). Click a face for a small card.
3. **Heat path split**: Heat strip, one stacked bar under the existing balance row. Click a segment to highlight the faces or openings.

## 1. Shell section (Properties)

One row per concern, no paragraphs:

| Row | Control | Notes |
| --- | --- | --- |
| Material | Steel, Stainless, Aluminium, Plastic (existing `Material`) | Sets k for the IEC path today; later sets layer k and rho*cp |
| Wall | thickness mm (drag value) | Existing `Enclosure::wall_mm`; interior size follows |
| Paint | RAL 7035 / galvanised / stainless brushed / custom emissivity | Sets `emissivity_in/out`; `?` lists planning values |
| Insulation | none / mm of foam | Adds a layer; off by default |
| Door | solid / window (area, glass) / gasket | `DoorSpec` |
| Plate | thickness mm, setback mm | `plate_setback_mm`; setback default 0 reproduces today |
| Mounting | floor free, wall, wall in row, recess | existing `Installation`, plus later H3 modes |
| Outside | ambient C, sun W/m2, wind m/s | `Outside`; sun and wind collapsed under "More" |

Rows that H1 or H3 have not delivered are hidden, not greyed with a sentence. A row appears when the engine reports the capability flag (see below).

## 2. Temperatures in the Heat layer

- Per face: inner and outer surface mean, min, max. The card shows two numbers and a delta to ambient. The `?` on the card states "model" and the lane that produces the value.
- Plate: front (device side) from the existing `plate_temp`, rear from `plate_rear_c`.
- Door: own card (not the front wall), also shows window temperature when a window exists.
- Colour scale is shared with the air scale in the key so a hot door and hot air compare by eye. Faces with no value (engine without H1) are drawn untinted, no placeholder text.
- Hot outer surface (touch limit) gets the existing warning colour; the 70 C / 55 C numbers are a planning hint, behind `?`.

## 3. Heat path split

One bar from `Balance` plus H1/H2/H3 extras. Segments, in fixed order and colour: through walls by convection, through walls by radiation, through the door, through vents and fans, stored. Today only walls (by face) and openings exist, so the bar starts as walls vs openings and gains segments as the engine reports them. Sum equals device loss; the residual is shown as a thin hatched end segment when above 1 %.

## 4. View-model (UI side)

`novo-app` reads one plain struct, built from `ThermalResult`, so the UI compiles before H1 lands:

```
ShellView { faces: [Option<FaceTemps>; 6], door: Option<FaceTemps>, plate_rear_c: Option<f64>,
            split: Vec<(PathKind, f64 /*W*/)>, caps: ShellCaps, status: ModelStatus }
FaceTemps { inner_c, outer_c, outer_min_c, outer_max_c }
ShellCaps { layers, emissivity, door_spec, setback, outside }   // bools from the engine
ModelStatus { Model, Iec, Exact }                                // per value, drives the badge
```

Until H1, the builder returns faces `None`, `split` from `Balance`, and `caps` all false, so only the split bar and the existing mounting/material rows show. Tests: the builder fills the split so the sum matches `loss_w`; absent data produces no tint and no text; badge for every term is `Model` unless listed in an `EXACT_TERMS` table that H6 edits.

## 5. Needs from H1 (contract)

- `FaceTemps` per face and for the door, with min/max over the face, in `ThermalResult` (2D) and in the CFD result.
- A capability flag per row above, or `Option` fields that are `None` when not modelled.
- Heat flow per path kind (convection, radiation, conduction through wall, openings) per face, summing to loss.
- Stable enum for face order (match the existing `front/rear/left/right/top/bottom` balance fields).
- Defaults reproduce today's totals within 2 %, so the Shell section can ship before the layered model without moving any result.

## 6. Steps

1. This design plus the `ShellView` builder and split bar from today's `Balance` (no engine change, QA scenario).
2. Shell section for material, wall thickness, mounting wired to existing fields.
3. Face temperature tint and cards once H1 lands.
4. Paint, insulation, door, setback, outside rows as H1/H3 deliver them.

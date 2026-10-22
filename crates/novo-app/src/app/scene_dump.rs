//! The open project as text for the visual test user (`scene` in control
//! mode): what is on the page, what is selected, the nets, and the cabinet
//! layouts, so canvas content can be checked without reading pixels.
//! Read only; nothing here changes the model.

use serde_json::{Value, json};

use super::NovoApp;

impl NovoApp {
    pub(crate) fn scene_json(&self) -> Value {
        let Some(doc) = self.document() else {
            return json!({"project": null, "workspace": format!("{:?}", self.workspace)});
        };
        let p = doc.project();
        let page = self.current_page();
        let tag_of = |sym: &novo_core::SymbolInstance| match &sym.role {
            novo_core::SymbolRole::Device { device, .. } => {
                p.device(*device).map(|d| d.tag.to_string())
            }
            novo_core::SymbolRole::Terminal { terminals } => {
                terminals.first().and_then(|t| p.terminal_designation(*t))
            }
            novo_core::SymbolRole::Interruption { name } => Some(format!("ip {name}")),
            novo_core::SymbolRole::PotentialFeed { potential } => p
                .potential(*potential)
                .map(|pot| format!("feed {}", pot.name)),
            novo_core::SymbolRole::Unassigned => None,
        };
        let mut symbols: Vec<Value> = p
            .symbols()
            .filter(|s| Some(s.page) == page)
            .map(|s| {
                json!({
                    "id": s.id.raw(),
                    "symbol": s.symbol.0,
                    "tag": tag_of(s),
                    "at_mm": [s.placement.origin.x, s.placement.origin.y],
                    "rotation": format!("{:?}", s.placement.rotation),
                    "mirrored": s.placement.mirrored,
                    "pins": s.pins.len(),
                })
            })
            .collect();
        symbols.sort_by_key(|v| v["id"].as_u64());
        let selected: Vec<u64> = self
            .canvas
            .schematic_ref()
            .map(|v| v.selection().iter().map(|id| id.raw()).collect())
            .unwrap_or_default();
        // Text conflicts of the open page (docs/TEXT-PLACEMENT.md, stage 0).
        let text_audit = self
            .canvas
            .schematic_ref()
            .and_then(|v| v.scene())
            .map(|s| {
                let a = novo_canvas::text_audit::audit(s.scene());
                json!({
                    "labels": a.labels,
                    "text_on_text": a.text_on_text.len(),
                    "text_on_symbol": a.text_on_ink.len(),
                    "text_on_wire": a.text_on_wire.len(),
                    "texts_too_close": a.tight.len(),
                    "text_outside_sheet": a.outside_sheet.len(),
                    "conflicts": a.describe(),
                })
            });
        let mut cabinets = Vec::new();
        for s in p.structures().filter(|s| s.plate.is_some()) {
            let Some(layout) = novo_layout::model::load(p, s.id) else {
                continue;
            };
            let tag = |id: u64| {
                p.devices()
                    .find(|d| d.id.raw() == id)
                    .map_or_else(|| id.to_string(), |d| d.tag.to_string())
            };
            let report = layout.report();
            cabinets.push(json!({
                "location": s.tag.to_string(),
                "enclosure": s.enclosure,
                "plate_mm": [layout.plate.width, layout.plate.height],
                "rails": layout.rails.iter().map(|r| json!({
                    "id": r.id, "at_mm": [r.x, r.y], "length_mm": r.length,
                    "vertical": r.vertical})).collect::<Vec<_>>(),
                "ducts": layout.ducts.iter().map(|d| json!({
                    "id": d.id, "at_mm": [d.x, d.y], "length_mm": d.length,
                    "width_mm": d.width, "vertical": d.vertical})).collect::<Vec<_>>(),
                "placed": layout.placements.iter().map(|pl| json!({
                    "device": tag(pl.id), "at_mm": [pl.x, pl.y],
                    "size_mm": [pl.size.width, pl.size.height],
                    "rotated": pl.rotated, "rail": pl.rail,
                    "surface": layout.off_plate.get(&pl.id)})).collect::<Vec<_>>(),
                "problems": {
                    "collisions": report.collisions.iter()
                        .map(|&(a, b)| format!("{} / {}", tag(a), tag(b))).collect::<Vec<_>>(),
                    "outside": report.outside.iter().map(|&i| tag(i)).collect::<Vec<_>>(),
                    "too_close": report.too_close.len(),
                    "rail_overflows": report.rail_overflows.len(),
                    "duct_overlaps": report.duct_overlaps.len(),
                    "guessed_size": report.guessed.iter().map(|&i| tag(i)).collect::<Vec<_>>(),
                },
            }));
        }
        json!({
            "workspace": format!("{:?}", self.workspace),
            "project": novo_py::info_json(p, self.backend.file().as_deref()),
            "saved": !self.backend.is_dirty(),
            "page": page.and_then(|id| p.page(id)).map(|pg| json!({
                "id": pg.id.raw(), "name": pg.name, "title": pg.title})),
            "pages": novo_py::pages_json(p),
            "page_symbols": symbols,
            "text_audit": text_audit,
            "selected_ids": selected,
            "devices": novo_py::devices_json(p),
            "wires": novo_py::wires_json(p),
            "nets": novo_py::nets_json(p),
            "cabinets": cabinets,
        })
    }
}

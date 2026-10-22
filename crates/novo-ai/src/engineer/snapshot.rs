//! The project as data for the read tools, cut to the agreed scope.

use crate::consent::DataScope;
use crate::tools::Snapshot;
use novo_core::Project;
use serde_json::{Value, json};

/// The project as data, sized by what the person allowed.
pub fn snapshot_of(p: &Project, selected: &[u64], scope: DataScope) -> Snapshot {
    let mut snap = Snapshot::default();
    if scope == DataScope::None {
        return snap;
    }
    let devices = devices_rich(p);
    let tags: Vec<Value> = devices
        .as_array()
        .map(|a| {
            a.iter()
                .map(|d| json!({"id": d["id"], "tag": d["tag"]}))
                .collect()
        })
        .unwrap_or_default();
    snap.outline = json!({
        "project": {
            "name": p.info().name,
            "pages": p.pages().count(),
            "devices": p.devices().count(),
            "wires": p.wires().count(),
            "terminals": p.terminals().count(),
            "cables": p.cables().count(),
            "parts": p.parts().count(),
        },
        "pages": novo_py::pages_json(p),
        "devices": tags,
        "sections_for_read_project": ["pages", "devices", "symbols", "wires", "nets", "potentials", "terminals", "parts", "problems", "wire_numbers", "standards"],
    });
    if scope >= DataScope::Selection {
        let items: Vec<Value> = p
            .symbols()
            .filter(|s| selected.contains(&s.id.raw()))
            .map(|s| {
                let device = match &s.role {
                    novo_core::SymbolRole::Device { device, .. } => p.device(*device),
                    _ => None,
                };
                json!({
                    "symbol_id": s.id.raw(),
                    "symbol": s.symbol.0,
                    "page_id": s.page.raw(),
                    "at_mm": [s.placement.origin.x, s.placement.origin.y],
                    "device": device.map(|d| json!({
                        "id": d.id.raw(), "tag": d.tag.to_string(),
                        "description": d.description})),
                })
            })
            .collect();
        snap.selection = json!({"selected_symbols": items});
    }
    if scope >= DataScope::Project {
        let mut terminals: Vec<_> = p.terminals().collect();
        terminals.sort_by_key(|t| t.id);
        let terminals: Vec<Value> = terminals
            .into_iter()
            .map(|t| {
                json!({"id": t.id.raw(), "strip": t.strip.raw(), "name": t.name,
                       "designation": p.terminal_designation(t.id)})
            })
            .collect();
        snap.project = json!({
            "pages": novo_py::pages_json(p),
            "devices": devices,
            "symbols": symbols_json(p),
            "standards": json!({
                "selected": novo_checks::standards::selected(p).iter().map(|s| s.short()).collect::<Vec<_>>(),
            }),
            "wires": novo_py::wires_json(p),
            "nets": novo_py::nets_json(p),
            "potentials": novo_py::potentials_json(p),
            "terminals": terminals,
            "parts": p.parts().map(|x| json!({
                "id": x.id.raw(), "manufacturer": x.manufacturer,
                "order_number": x.order_number, "type_number": x.type_number,
                "description": x.description})).collect::<Vec<_>>(),
            // The design checks, read-only: what the Issues list would show.
            "problems": novo_reports::issues::open(p).iter().map(|q| json!({
                "kind": format!("{:?}", q.kind),
                "severity": format!("{:?}", q.severity),
                "message": q.message,
                "page_id": q.page.map(|g| g.raw()),
                "objects": q.at.len(),
                "rule": q.rule})).collect::<Vec<_>>(),
            // Wire-number conflicts against the project's own scheme, read only.
            "wire_numbers": novo_numbering::check(p, &novo_numbering::Scheme::load(p))
                .iter()
                .map(|c| json!({
                    "kind": format!("{:?}", c.kind),
                    "severity": format!("{:?}", c.severity),
                    "number": c.number,
                    "wires": c.wires.iter().map(|w| w.raw()).collect::<Vec<_>>(),
                    "message": c.message}))
                .collect::<Vec<_>>(),
        });
    }
    snap
}

/// Devices with what an engineer reads first: description, properties, parts.
fn devices_rich(p: &Project) -> Value {
    let base = novo_py::devices_json(p);
    let Some(list) = base.as_array() else {
        return base;
    };
    Value::Array(
        list.iter()
            .map(|d| {
                let mut d = d.clone();
                if let Some(dev) = d["id"].as_u64().and_then(|id| p.device(id.into())) {
                    d["properties"] = json!(
                        dev.properties
                            .iter()
                            .map(|(k, v)| (k.clone(), v.to_string()))
                            .collect::<std::collections::BTreeMap<_, _>>()
                    );
                    d["parts"] = json!(
                        dev.parts
                            .iter()
                            .filter_map(|id| p.part(*id))
                            .map(|x| format!("{} {}", x.manufacturer, x.order_number))
                            .collect::<Vec<_>>()
                    );
                    d["symbols"] = json!(p.symbols_of_device(dev.id).len());
                }
                d
            })
            .collect(),
    )
}

/// Where everything is drawn: device, library symbol, page, position in
/// millimetres, pin names and the pins that still have no wire.
fn symbols_json(p: &Project) -> Value {
    let mut out = Vec::new();
    for s in p.symbols() {
        let open: Vec<&str> = s
            .pins
            .iter()
            .enumerate()
            .filter(|(i, pin)| {
                !pin.optional
                    && p.wires_at(novo_core::Endpoint::pin(s.id, *i as u16))
                        .next()
                        .is_none()
            })
            .map(|(_, pin)| pin.name.as_str())
            .collect();
        out.push(json!({
            "symbol_id": s.id.raw(),
            "device": s.device().and_then(|d| p.device(d)).map(|d| d.tag.to_string()),
            "symbol": s.symbol.0,
            "page": p.page(s.page).map(|g| g.name.clone()),
            "at_mm": [s.placement.origin.x.round(), s.placement.origin.y.round()],
            "pins": s.pins.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(),
            "open_pins": open,
        }));
    }
    Value::Array(out)
}

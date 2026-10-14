//! Bill of materials: every part the project uses, with quantities.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use novo_core::{AnyId, DeviceKind, EntityKind, PartId, Project, natural_cmp};

use crate::order::{join_distinct, tag_cmp};
use crate::report::{Dependencies, Report};
use crate::table::{Cell, Column, ReportOutput, Section};

/// The bill of materials.
///
/// Section `parts`: one row per article (manufacturer and order number),
/// counting every use: each entry in a device's part list, each terminal's
/// article ([`novo_terminals::terminal_part`]: its own part or its strip's
/// style) and each cable's part. Cable rows also sum the cable lengths.
/// Rows sort by manufacturer, then order number, in natural order.
///
/// Section `without_parts` (only when non-empty): devices and cables with
/// no part, in tag order. A terminal strip counts as covered when the strip
/// or any of its terminals carries a part.
#[derive(Debug, Clone, Copy, Default)]
pub struct BillOfMaterials;

impl BillOfMaterials {
    pub const ID: &'static str = "bom";
}

/// One use of a part.
struct Use {
    designation: String,
    source: AnyId,
    length_m: Option<f64>,
}

impl Report for BillOfMaterials {
    fn id(&self) -> &str {
        Self::ID
    }

    fn title(&self) -> String {
        "Bill of materials".into()
    }

    fn dependencies(&self) -> Dependencies {
        use EntityKind::*;
        Dependencies::on(&[Device, Terminal, Cable, Part])
    }

    fn generate(&self, project: &Project) -> ReportOutput {
        let mut uses: BTreeMap<PartId, Vec<Use>> = BTreeMap::new();
        let mut devices: Vec<_> = project.devices().collect();
        devices.sort_by(|a, b| tag_cmp(&a.tag, &b.tag).then(a.id.cmp(&b.id)));
        for device in &devices {
            for &part in &device.parts {
                uses.entry(part).or_default().push(Use {
                    designation: device.tag.to_string(),
                    source: device.id.into(),
                    length_m: None,
                });
            }
            for terminal in project.terminals_of_strip(device.id) {
                if let Some(part) = novo_terminals::terminal_part(project, terminal.id) {
                    uses.entry(part).or_default().push(Use {
                        designation: format!("{}:{}", device.tag, terminal.name),
                        source: terminal.id.into(),
                        length_m: None,
                    });
                }
            }
        }
        let mut cables: Vec<_> = project.cables().collect();
        cables.sort_by(|a, b| tag_cmp(&a.tag, &b.tag).then(a.id.cmp(&b.id)));
        for cable in &cables {
            if let Some(part) = cable.part {
                uses.entry(part).or_default().push(Use {
                    designation: cable.tag.to_string(),
                    source: cable.id.into(),
                    length_m: cable.length_m,
                });
            }
        }

        // Group by article, not by record: two part records for one article
        // are one line.
        type Article<'a> = (&'a str, &'a str);
        let mut articles: BTreeMap<Article<'_>, (Vec<PartId>, Vec<Use>)> = BTreeMap::new();
        for (id, list) in uses {
            let Some(part) = project.part(id) else {
                continue;
            };
            let entry = articles
                .entry((part.manufacturer.as_str(), part.order_number.as_str()))
                .or_default();
            entry.0.push(id);
            entry.1.extend(list);
        }
        let mut articles: Vec<_> = articles.into_iter().collect();
        articles.sort_by(|((ma, oa), _), ((mb, ob), _)| article_cmp(ma, oa, mb, ob));

        let mut parts = Section::new(
            "parts",
            "",
            vec![
                Column::new("position", "Pos.", 8.0).right(),
                Column::new("quantity", "Qty", 8.0).right(),
                Column::new("manufacturer", "Manufacturer", 25.0),
                Column::new("order_number", "Order number", 30.0),
                Column::new("type_number", "Type", 25.0),
                Column::new("description", "Description", 45.0),
                Column::new("length_m", "Length (m)", 12.0).right(),
                Column::new("used_by", "Used by", 50.0),
            ],
        );
        for (position, (_, (ids, list))) in articles.into_iter().enumerate() {
            let part = project.part(ids[0]).expect("checked above");
            let lengths: Vec<f64> = list.iter().filter_map(|u| u.length_m).collect();
            let length = (!lengths.is_empty()).then(|| lengths.iter().sum::<f64>());
            let mut sources: Vec<AnyId> = list.iter().map(|u| u.source).collect();
            sources.extend(ids.iter().map(|&id| AnyId::from(id)));
            parts.push(
                vec![
                    Cell::Int(position as i64 + 1),
                    Cell::Int(list.len() as i64),
                    Cell::text(part.manufacturer.clone()),
                    Cell::text(part.order_number.clone()),
                    Cell::text(part.type_number.clone()),
                    Cell::text(part.description.clone()),
                    length.into(),
                    Cell::text(join_distinct(list.iter().map(|u| u.designation.as_str()))),
                ],
                sources,
            );
        }

        let mut missing = Section::new(
            "without_parts",
            "Devices without parts",
            vec![
                Column::new("tag", "Tag", 30.0),
                Column::new("description", "Description", 60.0),
                Column::new("kind", "Kind", 25.0),
            ],
        );
        for device in &devices {
            let covered = !device.parts.is_empty()
                || (device.kind == DeviceKind::TerminalStrip
                    && project
                        .terminals_of_strip(device.id)
                        .iter()
                        .any(|t| novo_terminals::terminal_part(project, t.id).is_some()));
            if !covered {
                missing.push(
                    vec![
                        Cell::text(device.tag.to_string()),
                        Cell::text(device.description.clone()),
                        Cell::text(kind_name(device.kind)),
                    ],
                    vec![device.id.into()],
                );
            }
        }
        for cable in cables.iter().filter(|c| c.part.is_none()) {
            missing.push(
                vec![
                    Cell::text(cable.tag.to_string()),
                    Cell::text(cable.description.clone()),
                    Cell::text("Cable"),
                ],
                vec![cable.id.into()],
            );
        }

        let mut sections = vec![parts];
        if !missing.rows.is_empty() {
            sections.push(missing);
        }
        ReportOutput {
            report: Self::ID.into(),
            title: self.title(),
            revision: project.revision(),
            sections,
        }
    }
}

fn article_cmp(ma: &str, oa: &str, mb: &str, ob: &str) -> Ordering {
    natural_cmp(&ma.to_lowercase(), &mb.to_lowercase())
        .then_with(|| natural_cmp(oa, ob))
        .then_with(|| ma.cmp(mb))
}

pub(crate) fn kind_name(kind: DeviceKind) -> &'static str {
    match kind {
        DeviceKind::General => "Device",
        DeviceKind::TerminalStrip => "Terminal strip",
        DeviceKind::Connector => "Connector",
        DeviceKind::Plc => "PLC",
    }
}

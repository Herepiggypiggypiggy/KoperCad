//! Device list: every device with its structure, part and drawing locations.

use novo_core::{EntityKind, Project};

use crate::bom::kind_name;
use crate::order::{function_of, join_distinct, location_of, location_text, tag_cmp};
use crate::report::{Dependencies, Report};
use crate::table::{Cell, Column, ReportOutput, Section};

/// The device list: one row per device in natural tag order, with its
/// function (`=A1`) and location (`+CAB`) levels, description, kind, parts
/// (order numbers and manufacturers) and every place it is drawn as
/// `page.column` ([`Project::cross_references`], reading order, the main
/// function first when it is drawn).
#[derive(Debug, Clone, Copy, Default)]
pub struct DeviceList;

impl DeviceList {
    pub const ID: &'static str = "devices";
}

impl Report for DeviceList {
    fn id(&self) -> &str {
        Self::ID
    }

    fn title(&self) -> String {
        "Device list".into()
    }

    fn dependencies(&self) -> Dependencies {
        use EntityKind::*;
        Dependencies::on(&[Device, Symbol, Page, Part, Terminal])
    }

    fn generate(&self, project: &Project) -> ReportOutput {
        let mut section = Section::new(
            "devices",
            "",
            vec![
                Column::new("tag", "Tag", 30.0),
                Column::new("function", "Function", 15.0),
                Column::new("location", "Location", 15.0),
                Column::new("description", "Description", 45.0),
                Column::new("kind", "Kind", 18.0),
                Column::new("order_number", "Part", 35.0),
                Column::new("manufacturer", "Manufacturer", 25.0),
                Column::new("drawn_at", "Drawn at", 30.0),
            ],
        );
        let mut devices: Vec<_> = project.devices().collect();
        devices.sort_by(|a, b| tag_cmp(&a.tag, &b.tag).then(a.id.cmp(&b.id)));
        for device in devices {
            let parts: Vec<_> = device
                .parts
                .iter()
                .filter_map(|&p| project.part(p))
                .collect();
            let mut references = project.cross_references(device.id);
            // Main function first, the rest in reading order (stable sort).
            references.sort_by_key(|r| !r.main);
            section.push(
                vec![
                    Cell::text(device.tag.to_string()),
                    Cell::text(function_of(&device.tag)),
                    Cell::text(location_of(&device.tag)),
                    Cell::text(device.description.clone()),
                    Cell::text(kind_name(device.kind)),
                    Cell::text(join_distinct(parts.iter().map(|p| p.order_number.as_str()))),
                    Cell::text(join_distinct(parts.iter().map(|p| p.manufacturer.as_str()))),
                    Cell::text(join_distinct(
                        references.iter().map(|r| location_text(&r.location)),
                    )),
                ],
                vec![device.id.into()],
            );
        }
        ReportOutput {
            report: Self::ID.into(),
            title: self.title(),
            revision: project.revision(),
            sections: vec![section],
        }
    }
}

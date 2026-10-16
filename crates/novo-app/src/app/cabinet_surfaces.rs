//! The surfaces of the shown cabinet for the Cabinet header's Surface picker:
//! the plate, door, walls, roof, floor and any extra plates or profiles that
//! novo-layout::surfaces derives from the enclosure and the `surface.<name>`
//! structure properties, with how many devices name each one and what is
//! wrong with the names. Pure model; the header draws it (PANEL-B3).

use std::collections::BTreeMap;

use novo_core::{CabinetPlacement, Device, DeviceId, Project, StructureId, Value};
use novo_layout::Plate;
use novo_layout::model::enclosure_of;
use novo_layout::surfaces::{MAIN_PLATE, SURFACE_PROPERTY, SURFACE_SIZE_PREFIX, Surfaces};

/// One line of the picker.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SurfaceRow {
    pub name: String,
    /// "Mounting plate", "Door", ...
    pub kind: &'static str,
    /// "600 x 800 mm".
    pub size: String,
    /// Devices placed on it.
    pub devices: usize,
    /// Position and size are planning values, not stored.
    pub assumed: bool,
}

impl SurfaceRow {
    /// The picker line: name, size and count.
    pub fn label(&self) -> String {
        let count = match self.devices {
            0 => "empty".to_owned(),
            1 => "1 device".to_owned(),
            n => format!("{n} devices"),
        };
        format!("{}  {}  {}", self.name, self.size, count)
    }
}

/// The surfaces of one enclosure with their device counts.
#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct SurfaceSummary {
    pub rows: Vec<SurfaceRow>,
    /// Sentences about bad surface properties and placements that name a
    /// surface that does not exist.
    pub problems: Vec<String>,
}

impl SurfaceSummary {
    pub fn row(&self, name: &str) -> Option<&SurfaceRow> {
        self.rows.iter().find(|r| r.name == name)
    }

    /// The sentence under the header when `name` is the picked surface.
    pub fn note(&self, name: &str) -> Option<String> {
        let row = self.row(name)?;
        let basis = if row.assumed {
            " Its size and place are planning values from the cabinet."
        } else {
            ""
        };
        Some(format!(
            "{} {} ({}), {} placed.{basis}",
            row.kind,
            row.name,
            row.size,
            match row.devices {
                0 => "no devices".to_owned(),
                1 => "1 device".to_owned(),
                n => format!("{n} devices"),
            }
        ))
    }
}

/// The surfaces of `structure`; `None` until it has a mounting plate.
pub(super) fn summarize(project: &Project, structure: StructureId) -> Option<SurfaceSummary> {
    let s = project.get::<novo_core::Structure>(structure)?;
    let plate = s.plate?;
    let plate = Plate {
        width: plate.width,
        height: plate.height,
        margin: plate.margin,
    };
    let extra: Vec<(String, String)> = s
        .properties
        .iter()
        .filter_map(|(k, v)| {
            let name = k.strip_prefix(SURFACE_SIZE_PREFIX)?;
            match v {
                Value::Text(t) => Some((name.to_owned(), t.clone())),
                _ => None,
            }
        })
        .collect();
    let (list, mut problems) = match enclosure_of(project, structure) {
        Some(e) => {
            let (list, problems) = Surfaces::from_enclosure(&e, &plate, &extra);
            (list, problems.iter().map(|p| p.message()).collect())
        }
        // No interior yet: the plate is the only surface there is.
        None => (
            Surfaces {
                list: vec![novo_layout::surfaces::Surface {
                    name: MAIN_PLATE.into(),
                    kind: novo_layout::surfaces::SurfaceKind::Plate,
                    origin: [0.0; 3],
                    u: [1.0, 0.0, 0.0],
                    v: [0.0, 1.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    width: plate.width,
                    height: plate.height,
                    margin: plate.margin,
                    assumed: false,
                }],
            },
            Vec::<String>::new(),
        ),
    };

    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for p in project
        .all::<CabinetPlacement>()
        .filter(|p| p.structure == structure)
    {
        let named = match p.properties.get(SURFACE_PROPERTY) {
            Some(Value::Text(t)) => Some(t.as_str()),
            _ => None,
        };
        match list.find(named) {
            Some(found) => *counts.entry(found.name.clone()).or_default() += 1,
            None => {
                let tag = project
                    .get::<Device>(DeviceId::from(p.device.raw()))
                    .and_then(|d| d.tag.device_code().map(str::to_owned))
                    .unwrap_or_else(|| format!("#{}", p.device.raw()));
                problems.push(format!(
                    "{tag} is placed on surface '{}' that this cabinet does not have.",
                    named.unwrap_or_default()
                ));
            }
        }
    }
    let rows = list
        .list
        .iter()
        .map(|s| SurfaceRow {
            name: s.name.clone(),
            kind: s.kind.label(),
            size: format!("{:.0} x {:.0} mm", s.width, s.height),
            devices: counts.get(&s.name).copied().unwrap_or(0),
            assumed: s.assumed,
        })
        .collect();
    Some(SurfaceSummary { rows, problems })
}

impl super::NovoApp {
    /// `cabinet.surface`: opens the header's picker, or says why there is
    /// none when the shown cabinet has only its mounting plate.
    pub(super) fn open_cabinet_surfaces(&mut self) {
        let many = self.backend.document().is_some_and(|doc| {
            let project = doc.project();
            let loc = self.cabinet.location.as_ref();
            project
                .structures()
                .find(|s| s.enclosure && loc == Some(&s.tag))
                .and_then(|s| summarize(project, s.id))
                .is_some_and(|sum| sum.rows.len() > 1)
        });
        if many {
            self.cabinet.surface_open = true;
        } else {
            self.message(
                super::Level::Info,
                "This cabinet has only its mounting plate; add an enclosure in Door and depth to get more surfaces.",
            );
        }
    }
}

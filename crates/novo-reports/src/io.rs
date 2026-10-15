//! I/O list: every PLC channel, spares included (plc.md §6).

use novo_core::{AnyId, EntityKind, Project};

use crate::report::{Dependencies, Report};
use crate::table::{Cell, Column, ReportOutput, Section};

/// The I/O list: one section per PLC station, one row per channel of every
/// card, spare channels included, in address order (inputs, then outputs).
/// The rows come from [`novo_plc::PlcIndex::io_rows`]; channel symbols are
/// recognised by the built-in keys ([`novo_plc::FromKey`]).
///
/// Columns: address in the project's style, channel type (`DI`, `DQ`,
/// `AI`, ...), card, channel number, the channel symbol's pin, signal, the
/// terminals and devices on the channel's net, and where the channel is
/// drawn (`page.column`). A spare channel reads "Spare" and has no pin,
/// net or drawing.
#[derive(Debug, Clone, Copy, Default)]
pub struct IoList;

impl IoList {
    pub const ID: &'static str = "io";
}

impl Report for IoList {
    fn id(&self) -> &str {
        Self::ID
    }

    fn title(&self) -> String {
        "I/O list".into()
    }

    fn dependencies(&self) -> Dependencies {
        // What novo-plc reads, plus the netlist behind "connected to"
        // (junctions, potentials, terminals) and the address style.
        let mut kinds: Vec<EntityKind> = novo_plc::index::READS.to_vec();
        kinds.extend([
            EntityKind::Junction,
            EntityKind::Potential,
            EntityKind::Terminal,
        ]);
        Dependencies::on(&kinds).with_info()
    }

    fn generate(&self, project: &Project) -> ReportOutput {
        let index = novo_plc::PlcIndex::build(project, &novo_plc::FromKey);
        let columns = || {
            vec![
                Column::new("address", "Address", 14.0),
                Column::new("type", "Type", 8.0),
                Column::new("card", "Card", 18.0),
                Column::new("channel", "Ch.", 6.0).right(),
                Column::new("pin", "Pin", 8.0),
                Column::new("signal", "Signal", 30.0),
                Column::new("terminal", "Terminal", 16.0),
                Column::new("connected", "Connected to", 24.0),
                Column::new("drawn", "Drawn at", 10.0),
            ]
        };
        let mut sections: Vec<Section> = Vec::new();
        for row in index.io_rows(project) {
            if sections.last().is_none_or(|s| s.key != row.station) {
                sections.push(Section::new(&row.station, &row.station_title, columns()));
            }
            let section = sections.last_mut().expect("just pushed");
            let names = |list: &[(String, AnyId)]| {
                list.iter()
                    .map(|(name, _)| name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let mut sources: Vec<AnyId> = Vec::new();
            if let Some((_, symbol)) = &row.drawn_at {
                sources.push(*symbol);
            }
            sources.push(row.card.into());
            sources.extend(row.terminals.iter().map(|(_, id)| *id));
            sources.extend(row.connected.iter().map(|(_, id)| *id));
            let signal = if row.spare && row.signal.is_empty() {
                "Spare".to_owned()
            } else {
                row.signal.clone()
            };
            section.push(
                vec![
                    Cell::from(row.address.clone()),
                    Cell::text(row.kind.to_string()),
                    Cell::text(row.card_tag.clone()),
                    Cell::Int(i64::from(row.channel)),
                    Cell::text(row.pin.clone()),
                    Cell::text(signal),
                    Cell::text(names(&row.terminals)),
                    Cell::text(names(&row.connected)),
                    Cell::from(row.drawn_at.as_ref().map(|(at, _)| at.clone())),
                ],
                sources,
            );
        }
        ReportOutput {
            report: Self::ID.into(),
            title: self.title(),
            revision: project.revision(),
            sections,
        }
    }
}

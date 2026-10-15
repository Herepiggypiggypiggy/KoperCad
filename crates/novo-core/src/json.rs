//! JSON export and import: a diffable text form of a whole project.
//!
//! The primary project file is SQLite (WS-04); this format is for git diffs,
//! tests, scripting and exchange. Loading validates every integrity rule.

use serde::{Deserialize, Serialize};

use crate::entity::{
    Cable, Device, Dismissal, Junction, Page, Part, Potential, PotentialSystem, ProjectInfo,
    Structure, SymbolInstance, Terminal, Text, Wire,
};
use crate::error::{ModelError, Result};
use crate::project::Project;
use crate::record::Record;

/// Version of the JSON layout written by [`Project::to_json`].
///
/// New fields and record kinds are added within version 1 with serde
/// defaults, so every newer build reads older files. Older builds do not
/// read newer files that use them (an unknown record kind fails to load),
/// and re-saving a newer file in an older build drops the fields it does
/// not know. Bump this before the first release that must stay readable.
pub const JSON_FORMAT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct ProjectJson {
    format: u32,
    info: ProjectInfo,
    next_id: u64,
    #[serde(default)]
    parts: Vec<Part>,
    #[serde(default)]
    potentials: Vec<Potential>,
    #[serde(default)]
    pages: Vec<Page>,
    #[serde(default)]
    devices: Vec<Device>,
    #[serde(default)]
    terminals: Vec<Terminal>,
    #[serde(default)]
    symbols: Vec<SymbolInstance>,
    #[serde(default)]
    junctions: Vec<Junction>,
    #[serde(default)]
    wires: Vec<Wire>,
    #[serde(default)]
    cables: Vec<Cable>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    texts: Vec<Text>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    potential_systems: Vec<PotentialSystem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    structures: Vec<Structure>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dismissals: Vec<Dismissal>,
}

impl Project {
    /// The whole project as pretty-printed JSON, objects grouped by kind and
    /// sorted by id so the output is stable.
    pub fn to_json(&self) -> String {
        let file = ProjectJson {
            format: JSON_FORMAT_VERSION,
            info: self.info.clone(),
            next_id: self.next_id,
            parts: self.parts.values().cloned().collect(),
            potentials: self.potentials.values().cloned().collect(),
            pages: self.pages.values().cloned().collect(),
            devices: self.devices.values().cloned().collect(),
            terminals: self.terminals.values().cloned().collect(),
            symbols: self.symbols.values().cloned().collect(),
            junctions: self.junctions.values().cloned().collect(),
            wires: self.wires.values().cloned().collect(),
            cables: self.cables.values().cloned().collect(),
            texts: self.texts.values().cloned().collect(),
            potential_systems: self.potential_systems.values().cloned().collect(),
            structures: self.structures.values().cloned().collect(),
            dismissals: self.dismissals.values().cloned().collect(),
        };
        serde_json::to_string_pretty(&file).expect("project serializes to JSON")
    }

    /// Loads a project from [`Project::to_json`] output.
    pub fn from_json(text: &str) -> Result<Project> {
        let file: ProjectJson =
            serde_json::from_str(text).map_err(|e| ModelError::Format(e.to_string()))?;
        if file.format != JSON_FORMAT_VERSION {
            return Err(ModelError::UnsupportedFormat(file.format));
        }
        let records = file
            .parts
            .into_iter()
            .map(Record::Part)
            .chain(file.potentials.into_iter().map(Record::Potential))
            .chain(file.pages.into_iter().map(Record::Page))
            .chain(file.devices.into_iter().map(Record::Device))
            .chain(file.terminals.into_iter().map(Record::Terminal))
            .chain(file.symbols.into_iter().map(Record::Symbol))
            .chain(file.junctions.into_iter().map(Record::Junction))
            .chain(file.wires.into_iter().map(Record::Wire))
            .chain(file.cables.into_iter().map(Record::Cable))
            .chain(file.texts.into_iter().map(Record::Text))
            .chain(
                file.potential_systems
                    .into_iter()
                    .map(Record::PotentialSystem),
            )
            .chain(file.structures.into_iter().map(Record::Structure))
            .chain(file.dismissals.into_iter().map(Record::Dismissal));
        Project::from_records(file.info, file.next_id, records)
    }
}

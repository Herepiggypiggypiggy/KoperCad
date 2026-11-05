use serde::{Deserialize, Serialize};

/// What a triangle is drawn on. Ids are the text form of the model ids, so this
/// crate stays free of `novo-core`. One flag may have several anchors (one
/// triangle each, one count, one waiver).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Anchor {
    Symbol(String),
    Wire(String),
    Pin(String, u32),
    Device(String),
    Terminal(String),
    Cable(String),
    /// A spot on a sheet, in millimetres.
    Page(String, [i64; 2]),
    /// A spot on a mounting plate, in millimetres.
    Plate(String, [i64; 2]),
    Rail(String),
    Duct(String),
    Door(String),
    Opening(String),
    Enclosure(String),
    /// A spot in 3D, in millimetres.
    World(String, [i64; 3]),
    /// A rail item, top bar tile or button.
    Control(String),
    /// An export, import, simulation run or sweep.
    Job(String),
    /// The file itself.
    Project,
}

impl Anchor {
    /// Stable text used in flag ids and accessible names.
    pub fn key(&self) -> String {
        match self {
            Anchor::Symbol(i) => format!("symbol:{i}"),
            Anchor::Wire(i) => format!("wire:{i}"),
            Anchor::Pin(i, n) => format!("pin:{i}:{n}"),
            Anchor::Device(i) => format!("device:{i}"),
            Anchor::Terminal(i) => format!("terminal:{i}"),
            Anchor::Cable(i) => format!("cable:{i}"),
            Anchor::Page(i, p) => format!("page:{i}:{}:{}", p[0], p[1]),
            Anchor::Plate(i, p) => format!("plate:{i}:{}:{}", p[0], p[1]),
            Anchor::Rail(i) => format!("rail:{i}"),
            Anchor::Duct(i) => format!("duct:{i}"),
            Anchor::Door(i) => format!("door:{i}"),
            Anchor::Opening(i) => format!("opening:{i}"),
            Anchor::Enclosure(i) => format!("enclosure:{i}"),
            Anchor::World(i, p) => format!("world:{i}:{}:{}:{}", p[0], p[1], p[2]),
            Anchor::Control(i) => format!("control:{i}"),
            Anchor::Job(i) => format!("job:{i}"),
            Anchor::Project => "project".to_string(),
        }
    }
}

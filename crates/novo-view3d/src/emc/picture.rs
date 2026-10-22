//! What the EMC views draw: one plain-data `Picture` of a cabinet. The rule
//! engine (`novo-emc`, a sibling lane) will fill it from the project; until
//! then the app builds it from the scene with a labelled stub. Nothing here
//! knows the project model or does physics: classes, distances, risks and
//! fixes arrive decided, and the views only show them.
//!
//! Frame: the plate of `cabinet-layout.md`, millimetres, origin top left,
//! `y` down; `z` is the height above the plate surface. This is the frame
//! of `PlateFrame::world`, so 2D and 3D read the same numbers.

/// The cable class of a run, after IEC 61000-5-2 style grouping: what the
/// wire does to its neighbours and what its neighbours do to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Class {
    /// Unfiltered or filtered mains, load current: an aggressor.
    Mains,
    /// Drive output and motor cable: the loudest aggressor.
    Motor,
    /// 24 V DC supply and digital control: moderate in both directions.
    Control,
    /// Analog and sensor signals: a victim.
    Analog,
    /// Fieldbus and Ethernet: a victim that must also not radiate.
    Bus,
    /// Safety circuits: kept clear of every aggressor.
    Safety,
    /// Protective earth and bonding conductors.
    Earth,
}

impl Class {
    pub const ALL: [Class; 7] = [
        Class::Mains,
        Class::Motor,
        Class::Control,
        Class::Analog,
        Class::Bus,
        Class::Safety,
        Class::Earth,
    ];

    pub fn index(self) -> usize {
        match self {
            Class::Mains => 0,
            Class::Motor => 1,
            Class::Control => 2,
            Class::Analog => 3,
            Class::Bus => 4,
            Class::Safety => 5,
            Class::Earth => 6,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Class::Mains => "Mains power",
            Class::Motor => "Motor and drive output",
            Class::Control => "24 V control",
            Class::Analog => "Analog and sensor",
            Class::Bus => "Fieldbus",
            Class::Safety => "Safety",
            Class::Earth => "Earth and bonding",
        }
    }

    /// A word for tooltips and tags.
    pub fn short(self) -> &'static str {
        match self {
            Class::Mains => "mains",
            Class::Motor => "motor",
            Class::Control => "control",
            Class::Analog => "analog",
            Class::Bus => "bus",
            Class::Safety => "safety",
            Class::Earth => "earth",
        }
    }

    /// The runs that make noise for the others.
    pub fn is_aggressor(self) -> bool {
        matches!(self, Class::Mains | Class::Motor)
    }

    /// The runs that pick it up.
    pub fn is_victim(self) -> bool {
        matches!(
            self,
            Class::Analog | Class::Bus | Class::Safety | Class::Control
        )
    }
}

/// How a run is drawn besides its colour, so a class is never colour alone:
/// dash patterns are chosen per class by the views.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    /// The wire or cable this is (the model's raw id), 0 when it has none.
    pub id: u64,
    pub label: String,
    pub class: Class,
    /// A screened cable: drawn with a sheath.
    pub shielded: bool,
    /// Plate millimetres (x, y down) and height above the plate (z).
    pub path: Vec<[f32; 3]>,
    /// Distance the engine wants kept between this run and a victim, mm; the
    /// halo around an aggressor. 0 for a run that has none.
    pub halo_mm: f32,
    /// Why the run has this class, in a few words.
    pub class_reason: String,
    /// The designer set the class by hand; the engine did not derive it.
    pub overridden: bool,
}

/// What a device does to the field around it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// A noisy source: drive, switching supply, contactor coil.
    Source,
    /// A sensitive part: PLC, analog input, encoder interface, bus node.
    Victim,
    Neutral,
}

/// A device with its EMC role.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// The device (the model's raw id).
    pub device: u64,
    pub tag: String,
    pub role: Role,
    /// Footprint on the plate: x, y, width, height in mm.
    pub rect: [f32; 4],
    /// Body height above the plate, mm.
    pub height: f32,
    /// How loud a source is, or how sensitive a victim is, 0 to 1. An index,
    /// not a field strength.
    pub level: f32,
    /// What it is, in a few words: "frequency drive", "analog input".
    pub what: String,
}

impl Node {
    pub fn centre(&self) -> [f32; 2] {
        [
            self.rect[0] + self.rect[2] * 0.5,
            self.rect[1] + self.rect[3] * 0.5,
        ]
    }
}

/// What kind of finding a hot spot is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// An aggressor and a victim run side by side.
    Parallel,
    /// A victim part sits near a source.
    Proximity,
    /// A shield ends badly.
    Shield,
    /// A circuit encloses a large area.
    Loop,
    /// A bonding path is weak or missing.
    Bond,
    /// A drive has no mains filter.
    Filter,
    /// The magnetic field estimate: a device in a strong field, or a wire
    /// picking up a voltage from it.
    Field,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Parallel => "Parallel run",
            Kind::Proximity => "Too close",
            Kind::Shield => "Shield end",
            Kind::Loop => "Loop area",
            Kind::Bond => "Bonding",
            Kind::Filter => "Filter",
            Kind::Field => "Magnetic field",
        }
    }
}

/// How firm the number behind a finding is (docs/EMC.md section 4). The
/// views print it next to the finding, so a result never looks firmer than
/// its method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// A rule on what is drawn: it holds or it does not.
    Rule,
    /// A number from geometry alone, good for ordering.
    Index,
    /// A closed-form estimate with stated assumptions.
    Estimate,
    /// A modelled field (quasi-static, low frequency) with its band and inputs.
    Field,
}

impl Level {
    pub fn name(self) -> &'static str {
        match self {
            Level::Rule => "Rule",
            Level::Index => "Index",
            Level::Estimate => "Estimate",
            Level::Field => "Field model",
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            Level::Rule => "A rule on what is drawn: it holds or it does not.",
            Level::Index => {
                "A number from the geometry alone. Good for ordering, not a measurement."
            }
            Level::Estimate => {
                "A closed-form estimate with the assumptions listed. Not a measurement."
            }
            Level::Field => {
                "A modelled low-frequency field with its band and inputs. Not a measurement, and not valid outside its band."
            }
        }
    }
}

/// A change the engine offers for a finding.
#[derive(Debug, Clone, PartialEq)]
pub struct Fix {
    /// The engine's key for the change, handed back when it is applied.
    pub key: String,
    pub label: String,
    /// What it does, what it costs, in a sentence.
    pub detail: String,
    /// The engine can apply it as commands (undo, redo). False shows it as
    /// advice only.
    pub applicable: bool,
}

/// An input a model assumed, shown on the card; `user_set` tells a figure the
/// designer entered from the default the program chose.
#[derive(Debug, Clone, PartialEq)]
pub struct Assumed {
    pub text: String,
    pub user_set: bool,
}

/// A place where the cabinet is likely to have an EMC problem.
#[derive(Debug, Clone, PartialEq)]
pub struct Hotspot {
    pub id: u32,
    pub kind: Kind,
    pub level: Level,
    /// Where the marker stands: plate x, y and height.
    pub at: [f32; 3],
    /// The stretch the finding covers (a parallel run), plate coordinates;
    /// empty for a point.
    pub span: Vec<[f32; 3]>,
    /// 0 (fine) to 1 (worst), on the same scale as the risk map.
    pub risk: f32,
    pub title: String,
    /// Why it matters, in plain words.
    pub why: String,
    /// The figures behind it: name and value with unit.
    pub numbers: Vec<(String, String)>,
    /// The runs and devices involved (raw model ids).
    pub runs: Vec<u64>,
    pub devices: Vec<u64>,
    /// The rule that raised it (`EMC-02`).
    pub rule: String,
    /// Where the rule comes from, with its verify mark, as docs/EMC.md
    /// section 5 words it. Empty for an interim finding.
    pub source: String,
    /// The frequency band an estimate or field holds in; `None` for a rule or
    /// an index.
    pub band: Option<String>,
    /// The inputs behind an estimate or a field (current, dI/dt ...).
    pub assumptions: Vec<Assumed>,
    pub fixes: Vec<Fix>,
}

/// A shaded loop: the area between the go and the return conductor.
#[derive(Debug, Clone, PartialEq)]
pub struct LoopSheet {
    pub label: String,
    /// The outline on the plate: x, y and height.
    pub outline: Vec<[f32; 3]>,
    pub area_cm2: f32,
    pub risk: f32,
}

/// How good a bonding connection is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Quality {
    Good,
    Weak,
    Bad,
}

/// One link of the earth and bonding graph.
#[derive(Debug, Clone, PartialEq)]
pub struct Bond {
    pub label: String,
    pub path: Vec<[f32; 3]>,
    pub quality: Quality,
}

/// Where a shield ends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShieldEnd {
    /// 360 degrees on a clamp or gland.
    Full,
    /// A wire of this length in mm between shield and ground.
    Pigtail(f32),
    /// Not connected.
    Open,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Shield {
    pub run: u64,
    pub at: [f32; 3],
    pub end: ShieldEnd,
    pub label: String,
}

/// Where a picture comes from, always shown beside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    /// Made up for a demonstration; says nothing about the project.
    Synthetic,
    /// Read from the project with guessed classes: the interim stand-in for
    /// the rule engine.
    Stub,
    /// The rule engine's result.
    Engine,
}

impl Provenance {
    pub fn name(self) -> &'static str {
        match self {
            Provenance::Synthetic => "Demonstration data",
            Provenance::Stub => "Classes guessed from tags and wire colour",
            Provenance::Engine => "Rule engine",
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            Provenance::Synthetic => {
                "Made-up data to show what the view can say. It says nothing about your project."
            }
            Provenance::Stub => {
                "Stand-in until the EMC rule engine is connected: classes are guessed from symbol \
                 types, pin names and wire colours. Distances and lengths are measured from your \
                 routed wires."
            }
            Provenance::Engine => "Classes and findings from the EMC rule engine.",
        }
    }
}

/// A map over the plate, 0 to 1 in each cell, row by row from the top left.
/// The engine may supply one (a quasi-static field slice); otherwise the
/// views make a geometric exposure index (`field::exposure`).
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// Plate millimetres of the first cell's corner.
    pub origin: [f32; 2],
    /// Cell edge, mm.
    pub cell: f32,
    pub nx: usize,
    pub ny: usize,
    pub values: Vec<f32>,
    /// What the map is, as the legend says it.
    pub label: String,
    /// What the two ends of the colour ramp stand for ("1.3 \u{b5}T",
    /// "126 \u{b5}T"), when the values are a physical quantity and not an index.
    pub ends: Option<(String, String)>,
}

impl Field {
    pub fn at(&self, x: f32, y: f32) -> f32 {
        if self.cell <= 0.0 || self.nx == 0 || self.ny == 0 {
            return 0.0;
        }
        let i = ((x - self.origin[0]) / self.cell).floor();
        let j = ((y - self.origin[1]) / self.cell).floor();
        if i < 0.0 || j < 0.0 || i as usize >= self.nx || j as usize >= self.ny {
            return 0.0;
        }
        self.values
            .get(j as usize * self.nx + i as usize)
            .copied()
            .unwrap_or(0.0)
    }
}

/// Everything the views need about one plate.
#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    /// Plate width and height, mm.
    pub plate: [f32; 2],
    pub runs: Vec<Run>,
    pub nodes: Vec<Node>,
    pub hotspots: Vec<Hotspot>,
    pub loops: Vec<LoopSheet>,
    pub bonds: Vec<Bond>,
    pub shields: Vec<Shield>,
    /// The engine's own map, when it has one.
    pub field: Option<Field>,
    pub provenance: Provenance,
    /// Inputs the model assumed, each in a sentence ("12 A motor current").
    pub assumptions: Vec<String>,
    /// What the model cannot judge; always listed.
    pub not_checked: Vec<String>,
}

impl Picture {
    pub fn empty(plate: [f32; 2], provenance: Provenance) -> Picture {
        Picture {
            plate,
            runs: Vec::new(),
            nodes: Vec::new(),
            hotspots: Vec::new(),
            loops: Vec::new(),
            bonds: Vec::new(),
            shields: Vec::new(),
            field: None,
            provenance,
            assumptions: Vec::new(),
            not_checked: DEFAULT_NOT_CHECKED.iter().map(|s| s.to_string()).collect(),
        }
    }

    pub fn run(&self, id: u64) -> Option<&Run> {
        self.runs.iter().find(|r| r.id == id)
    }

    pub fn node(&self, device: u64) -> Option<&Node> {
        self.nodes.iter().find(|n| n.device == device)
    }

    pub fn hotspot(&self, id: u32) -> Option<&Hotspot> {
        self.hotspots.iter().find(|h| h.id == id)
    }

    /// The worst hot spot risk, 0 when there are none.
    pub fn worst(&self) -> f32 {
        self.hotspots.iter().map(|h| h.risk).fold(0.0, f32::max)
    }

    /// Hot spots, worst first (ties by id).
    pub fn ranked(&self) -> Vec<&Hotspot> {
        let mut v: Vec<&Hotspot> = self.hotspots.iter().collect();
        v.sort_by(|a, b| b.risk.total_cmp(&a.risk).then(a.id.cmp(&b.id)));
        v
    }

    /// True when every number the views will draw is finite and in range.
    pub fn is_sane(&self) -> bool {
        let p3 = |p: &[f32; 3]| p.iter().all(|v| v.is_finite());
        self.plate.iter().all(|v| v.is_finite() && *v > 0.0)
            && self
                .runs
                .iter()
                .all(|r| r.path.iter().all(p3) && r.halo_mm.is_finite())
            && self
                .nodes
                .iter()
                .all(|n| n.rect.iter().all(|v| v.is_finite()) && (0.0..=1.0).contains(&n.level))
            && self
                .hotspots
                .iter()
                .all(|h| p3(&h.at) && h.span.iter().all(p3) && (0.0..=1.0).contains(&h.risk))
            && self.loops.iter().all(|l| l.outline.iter().all(p3))
            && self.bonds.iter().all(|b| b.path.iter().all(p3))
    }
}

/// The limits of every model here, listed on every card and report page.
pub const DEFAULT_NOT_CHECKED: [&str; 4] = [
    "Radiated emission of the machine and the margin to any limit",
    "Resonances and leakage of the enclosure (seams, slots, doors)",
    "The real insertion loss of filters and the real impedance of bonds",
    "Compliance: only a measurement on the finished machine shows it",
];

//! What the engine reads: plain data, built from a project by
//! [`crate::project`] or by hand in a test.

use crate::class::{EmcClass, Shield};

/// A point on a mounting plate in mm, x right, y down.
pub type P = (f64, f64);

/// Where a run's class came from, shown with the reason on every card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Set by the user (`emc.class` on the wire or its cable).
    Declared,
    /// Read from the drawing (drive output, safety device, potential).
    Derived,
    /// No evidence either way: the voltage or symbols did not say.
    Guessed,
}

/// One wire run: a wire (or cable core) from one pin to another.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    /// The wire's raw id.
    pub id: u64,
    /// What to call it: wire number, else the end pins.
    pub name: String,
    pub class: EmcClass,
    pub origin: Origin,
    /// Why this class, in a few words.
    pub class_reason: String,
    pub cable: Option<u64>,
    /// The cable's shield, when it has one.
    pub shield: Option<Shield>,
    pub twisted: bool,
    pub net: Option<u32>,
    /// Devices at the two ends (terminal strips included).
    pub from: Option<u64>,
    pub to: Option<u64>,
    /// The enclosure plate the route lies on.
    pub plate: Option<u64>,
    /// The route on the plate, centre line from end to end.
    pub path: Vec<P>,
    /// The ducts the route lies in.
    pub ducts: Vec<u64>,
    pub diameter_mm: f64,
    /// The drive whose output this run is, for a motor run.
    pub drive: Option<u64>,
    /// Leaves the cabinet (the other end is outside it).
    pub leaves_cabinet: bool,
}

impl Run {
    /// The class was set by the user, not read from the drawing.
    pub fn overridden(&self) -> bool {
        self.origin == Origin::Declared
    }
}

/// A duct on a plate.
#[derive(Debug, Clone, PartialEq)]
pub struct Duct {
    pub id: u64,
    pub a: P,
    pub b: P,
    pub width_mm: f64,
    /// A metal divider keeps two classes apart inside one duct.
    pub divider: bool,
}

/// What a device does for EMC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Drive,
    /// Mains filter or EMC filter.
    Filter,
    /// A PLC, analog module or encoder interface.
    Sensitive,
    /// A switched-mode supply or other noisy device.
    Noisy,
    Other,
}

/// How a drive's mains filter is provided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterState {
    /// The part says it has one built in (or is category C1/C2).
    Built,
    /// The project says one is fitted outside (a device tag, may be empty).
    Declared,
    /// Nothing says.
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub id: u64,
    pub tag: String,
    pub role: Role,
    pub plate: Option<u64>,
    /// x, y, width, height on the plate, mm.
    pub rect: Option<(f64, f64, f64, f64)>,
    pub filter: FilterState,
    /// For a filter: the tag of the drive it serves, when stated.
    pub filter_for: Option<String>,
    /// Mounted on bare metal that bonds to the plate (`emc.bonded`), when stated.
    pub bonded: Option<bool>,
}

/// How a plate's surface was left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finish {
    /// Galvanised, bare or otherwise conductive.
    Conductive,
    /// Painted or anodised where parts sit.
    Painted,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plate {
    pub id: u64,
    pub tag: String,
    pub finish: Option<Finish>,
}

/// Thresholds and assumptions. Every field is shown next to the results it
/// steers; the project can override each (`emc.<name>`).
#[derive(Debug, Clone, PartialEq)]
pub struct Params {
    /// Shorter parallel stretches than this are not reported (mm).
    pub min_parallel_mm: f64,
    /// Longest loop between go and return before it is flagged (cm2).
    pub max_loop_cm2: f64,
    /// Longest distance from a filter to its drive (mm).
    pub filter_max_mm: f64,
    /// Closest a sensitive device may sit to a drive (mm).
    pub sensitive_min_mm: f64,
    /// Longest shield pigtail (mm).
    pub pigtail_max_mm: f64,
    /// Frequency for the pigtail impedance (Hz).
    pub frequency_hz: f64,
    /// Current step assumed in an aggressor, A per microsecond, for the
    /// induced voltage estimate.
    pub didt_a_per_us: f64,
    /// Height of the wires above the plate for the estimate (mm).
    pub wire_height_mm: f64,
    /// Planning distance overrides, mm, by (aggressor, victim).
    pub distance_override: Vec<(EmcClass, EmcClass, f64)>,
    /// Names of the fields the project set (`min_parallel_mm`, ...).
    pub user_set: Vec<&'static str>,
}

impl Default for Params {
    fn default() -> Params {
        Params {
            min_parallel_mm: 300.0,
            max_loop_cm2: 100.0,
            filter_max_mm: 300.0,
            sensitive_min_mm: 100.0,
            pigtail_max_mm: 50.0,
            frequency_hz: 10.0e6,
            didt_a_per_us: 10.0,
            wire_height_mm: 15.0,
            distance_override: Vec::new(),
            user_set: Vec::new(),
        }
    }
}

impl Params {
    /// The planning distance for a pair, mm, with the project's overrides.
    pub fn distance_mm(&self, a: EmcClass, b: EmcClass) -> f64 {
        self.distance_override
            .iter()
            .find(|(x, y, _)| (*x == a && *y == b) || (*x == b && *y == a))
            .map(|(_, _, d)| *d)
            .unwrap_or_else(|| crate::class::default_distance_mm(a, b))
    }
}

/// Everything the rules read.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Input {
    pub runs: Vec<Run>,
    pub ducts: Vec<Duct>,
    pub devices: Vec<Device>,
    pub plates: Vec<Plate>,
    pub params: Params,
}

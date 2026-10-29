//! The standard circuit library: generic circuits written for KoperCAD as
//! plain data (one TOML file each, under `crates/novo-parts/circuits/`),
//! organised in chapters, built into ordinary macro files when the program
//! starts and kept in the macro library next to the user's own.
//!
//! A circuit file lists devices, library symbols with their places, and
//! wires; see [`CircuitSpec`]. Building one draws it in a scratch project
//! through the same commands a user's drawing takes, so a circuit placed
//! from the library is exactly as editable as one the user drew. Every
//! circuit is checked while it is built: a pin that is neither wired nor
//! listed as an open point, or a mounting view that does not fit its plate,
//! is an error in the library's own tests, not a surprise for the user.
//!
//! Each device can also carry a mounting view ([`MountingView`]): where it sits
//! on a reference plate or the door, a typical size, or that it sits on the
//! machine. The view is a guide for sizing the cabinet; the real size comes
//! from the part the user assigns.
//!
//! Everything here is written from general electrical knowledge. Nothing is
//! copied from a manufacturer's macro, a CAD vendor's library or a manual.

use thiserror::Error;

mod build;
mod mounting;
mod spec;

pub use mounting::{MountItem, MountKind, MountPlate, MountRail, MountingView};
pub use spec::CircuitSpec;

use super::{Imported, MacroError, MacroFile, MacroLibrary};

/// The top folder of the circuit library in a macro library.
pub const CIRCUIT_ROOT: &str = "Circuits";

/// One chapter of the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chapter {
    pub number: u8,
    /// The `chapter` value in a circuit file.
    pub slug: &'static str,
    pub title: &'static str,
    /// One line on what the chapter holds.
    pub blurb: &'static str,
}

impl Chapter {
    /// The macro library folder of the chapter, e.g. `Circuits/3 Hoists`.
    pub fn folder(&self) -> String {
        format!("{CIRCUIT_ROOT}/{} {}", self.number, self.title)
    }
}

/// The chapters, in order. A circuit names one by its slug.
pub const CHAPTERS: &[Chapter] = &[
    Chapter {
        number: 1,
        slug: "supply",
        title: "Supply and distribution",
        blurb: "Main switch, control transformer, DC power supply",
    },
    Chapter {
        number: 2,
        slug: "protection",
        title: "Protection and starters",
        blurb: "Motor breakers, contactors, direct-on-line and reversing starters",
    },
    Chapter {
        number: 3,
        slug: "hoists",
        title: "Hoists",
        blurb: "Hoist motors with brake, speed steps, end limits",
    },
    Chapter {
        number: 4,
        slug: "travel",
        title: "Travel drives",
        blurb: "Bridge, trolley and slew drives: contactor and frequency controlled",
    },
    Chapter {
        number: 5,
        slug: "safety",
        title: "Safety",
        blurb: "Emergency stop chains, main contactor, safety relays",
    },
    Chapter {
        number: 6,
        slug: "control",
        title: "Operator stations",
        blurb: "Radio receiver and pendant interfaces to the motion contactors",
    },
    Chapter {
        number: 7,
        slug: "peripherals",
        title: "Sensors and signalling",
        blurb: "Limit switches, motor temperature, warning lamp and horn",
    },
];

/// Why a circuit file could not become a macro.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CircuitError {
    #[error("{origin}: {reason}")]
    Parse { origin: String, reason: String },
    #[error("circuit '{circuit}': {reason}")]
    Invalid { circuit: String, reason: String },
}

impl CircuitError {
    pub(crate) fn at(spec: &CircuitSpec, reason: String) -> Self {
        CircuitError::Invalid {
            circuit: spec.name.clone(),
            reason,
        }
    }
}

/// A circuit file shipped with the program: its path under `circuits/` and
/// its text.
pub struct CircuitSource {
    pub path: &'static str,
    pub text: &'static str,
}

macro_rules! sources {
    ($($path:literal),* $(,)?) => {
        &[$(CircuitSource {
            path: $path,
            text: include_str!(concat!("../../../circuits/", $path)),
        }),*]
    };
}

/// Every shipped circuit file.
const SOURCES: &[CircuitSource] = sources![
    "1-supply/control-supply-24v.toml",
    "1-supply/control-transformer.toml",
    "2-protection/feeder-dol.toml",
    "2-protection/feeder-reversing.toml",
    "3-hoists/hoist-load-limit-relay.toml",
    "3-hoists/hoist-single-speed-brake.toml",
    "3-hoists/hoist-two-speed-brake.toml",
    "3-hoists/brake-rectifier-fast-release.toml",
    "3-hoists/hoist-vfd-brake.toml",
    "4-travel/bridge-travel-two-motors-one-drive.toml",
    "4-travel/travel-two-speed-brake.toml",
    "4-travel/travel-vfd-brake.toml",
    "5-safety/estop-main-contactor.toml",
    "5-safety/estop-safety-relay.toml",
    "6-control/pendant-two-step-station.toml",
    "6-control/radio-receiver-relays.toml",
    "7-peripherals/anti-condensation-heater.toml",
    "7-peripherals/cabinet-fan-thermostat.toml",
    "7-peripherals/horn-and-beacon.toml",
    "7-peripherals/limit-switch-permits.toml",
    "7-peripherals/motor-thermistor-relay.toml",
];

/// The circuit files that ship with the program.
pub fn circuit_sources() -> &'static [CircuitSource] {
    SOURCES
}

/// Builds one circuit file into a macro file.
pub fn build_circuit(text: &str, origin: &str) -> Result<MacroFile, CircuitError> {
    build::build(&CircuitSpec::parse(text, origin)?)
}

/// Every shipped circuit as a macro file with the library folder of its
/// chapter, in chapter order.
pub fn circuit_library() -> Result<Vec<(String, MacroFile)>, CircuitError> {
    let mut out = Vec::new();
    for source in SOURCES {
        let spec = CircuitSpec::parse(source.text, source.path)?;
        let chapter = CHAPTERS
            .iter()
            .find(|c| c.slug == spec.chapter)
            .ok_or_else(|| {
                CircuitError::at(&spec, format!("unknown chapter '{}'", spec.chapter))
            })?;
        out.push((chapter.number, chapter.folder(), build::build(&spec)?));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.2.name.cmp(&b.2.name)));
    Ok(out
        .into_iter()
        .map(|(_, folder, file)| (folder, file))
        .collect())
}

impl MacroLibrary {
    /// Adds the shipped circuits that are not in the library yet, each in
    /// the folder of its chapter. A circuit the user renamed, moved or
    /// deleted is not put back over their work: one that already exists is
    /// left alone.
    pub fn install_circuits(&self) -> Result<Vec<Imported>, MacroError> {
        let circuits = circuit_library().map_err(|e| MacroError::Format {
            path: self.root().to_path_buf(),
            reason: e.to_string(),
        })?;
        let mut done = Vec::new();
        for (folder, file) in circuits {
            match self.save(&folder, &file) {
                Ok(path) => done.push(Imported::Added(path)),
                Err(MacroError::Exists(name)) => done.push(Imported::Skipped(name)),
                Err(e) => return Err(e),
            }
        }
        Ok(done)
    }

    /// The starters and the circuit library, put in the library the first
    /// time.
    pub fn install_standard(&self) -> Result<Vec<Imported>, MacroError> {
        let mut done = self.install_starters()?;
        done.extend(self.install_circuits()?);
        Ok(done)
    }
}

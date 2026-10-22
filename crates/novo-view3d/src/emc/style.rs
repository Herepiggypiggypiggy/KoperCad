//! What the EMC view shows and in which colours. The colours come from the
//! theme (`novo-theme::emc`), so this crate carries only the shape of them.

use super::picture::Class;

/// Which layers of the EMC view are on. The first four are the default look;
/// the others answer a question each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EmcLayers {
    /// Runs coloured by cable class (and devices by role).
    pub classes: bool,
    /// The keep-clear zones around noisy runs.
    pub halos: bool,
    /// The glow around noisy parts.
    pub sources: bool,
    /// The numbered markers where something is likely to go wrong.
    pub spots: bool,
    /// The exposure map over the plate.
    pub risk: bool,
    /// Loop areas as shaded sheets.
    pub loops: bool,
    /// The earth and bonding graph.
    pub bonding: bool,
    /// Where each shield ends.
    pub shields: bool,
}

impl Default for EmcLayers {
    fn default() -> Self {
        EmcLayers {
            classes: true,
            halos: true,
            sources: true,
            spots: true,
            risk: false,
            loops: false,
            bonding: false,
            shields: false,
        }
    }
}

impl EmcLayers {
    pub fn count(&self) -> usize {
        [
            self.classes,
            self.halos,
            self.sources,
            self.spots,
            self.risk,
            self.loops,
            self.bonding,
            self.shields,
        ]
        .iter()
        .filter(|b| **b)
        .count()
    }
}

/// Colours, as sRGB bytes.
#[derive(Debug, Clone, Copy)]
pub struct Style {
    /// Per class, indexed by `Class::index`.
    pub class: [[u8; 3]; 7],
    /// The risk ramp, 0 (calm) to 1 (worst); the heat view's ember ramp.
    pub ramp: fn(f32) -> [u8; 3],
    /// The two colours of a protective conductor.
    pub earth: ([u8; 3], [u8; 3]),
    pub good: [u8; 3],
}

impl Style {
    pub fn colour(&self, class: Class) -> [u8; 3] {
        self.class[class.index()]
    }

    pub fn risk(&self, risk: f32) -> [u8; 3] {
        (self.ramp)(risk.clamp(0.0, 1.0))
    }
}

/// Alpha of the keep-clear halos, strongest nearest the run.
pub const HALO_ALPHA: [u8; 3] = [18, 30, 46];

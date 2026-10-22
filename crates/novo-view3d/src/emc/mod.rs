//! EMC views: the picture the rule engine hands over (`picture`), the exposure
//! index map (`field`) and the overlays and tinted scene that draw it. See
//! `docs/EMC.md` and `docs/design/emc-view.md`.

pub mod field;
pub mod geom;
pub mod overlays;
pub mod picture;
pub mod style;

pub use overlays::{label, overlays, tint_scene};
pub use picture::{
    Assumed, Bond, Class, Field, Fix, Hotspot, Kind, Level, LoopSheet, Node, Picture, Provenance,
    Quality, Role, Run, Shield, ShieldEnd,
};
pub use style::{EmcLayers, Style};

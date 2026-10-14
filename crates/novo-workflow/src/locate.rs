//! Where a device is drawn, for jumps (G, Shift+G) and the palette's
//! location rows.
//!
//! Thin layer over novo-core's [`Project::cross_references`]: the main
//! function (the coil) first, then the rest in reading order, each written
//! `/3.4` (page 3, column 4) like a cross reference on the drawing.

use novo_core::{CrossReference, DeviceId, Project};

/// Every place a device is drawn: main function first, then in reading
/// order (page, then column).
pub fn locations(project: &Project, device: DeviceId) -> Vec<CrossReference> {
    let mut refs = project.cross_references(device);
    // Stable: reading order is kept within each group.
    refs.sort_by_key(|r| !r.main);
    refs
}

/// `/3.4` for page 3, column 4.
pub fn reference(r: &CrossReference) -> String {
    format!("/{}", r.location)
}

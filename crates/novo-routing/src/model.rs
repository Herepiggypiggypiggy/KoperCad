//! The bridge between the project model and the routing engine: read the
//! ducts on an enclosure's mounting plate. Engine duct ids are the raw
//! model ids of the rails.

use novo_core::{Project, Rail, RailKind, StructureId};

use crate::{Duct, Network, Point};

/// The ducts on the plate of an enclosure, in model id order. DIN rails are
/// not ducts and are left out. A duct without a cross-section keeps
/// width and height 0, which the engine reports as unsized.
pub fn ducts(project: &Project, structure: StructureId) -> Vec<Duct> {
    project
        .all::<Rail>()
        .filter(|r| r.structure == structure && r.kind == RailKind::Duct)
        .map(|r| Duct {
            id: r.id.raw(),
            a: Point::new(r.x, r.y),
            b: if r.vertical {
                Point::new(r.x, r.y + r.length)
            } else {
                Point::new(r.x + r.length, r.y)
            },
            width: r.width,
            height: r.height,
        })
        .collect()
}

/// The routing network of an enclosure's ducts.
pub fn network(project: &Project, structure: StructureId) -> Network {
    Network::new(&ducts(project, structure))
}

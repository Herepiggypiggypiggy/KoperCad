//! Door and depth checks of an enclosure, as problems (novo-layout's
//! `CabinetLayout::door_report`). Quiet for a plate whose enclosure has no
//! interior set, so projects without an enclosure see nothing.

use novo_core::{AnyId, DeviceId};
use novo_layout::model::load;

use crate::{Context, Problem, ProblemKind};

pub(crate) fn door(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    let tag = |id: u64| {
        project
            .device(DeviceId::from(id))
            .map_or_else(|| "A device".to_owned(), |d| d.tag.to_string())
    };
    let device = |id: u64| vec![AnyId::Device(DeviceId::from(id))];
    for structure in project.structures().filter(|s| s.plate.is_some()) {
        let Some(layout) = load(project, structure.id) else {
            continue;
        };
        let Some(report) = layout.door_report() else {
            continue;
        };
        let name = &structure.tag;
        for &(id, excess) in &report.too_deep {
            out.push(Problem::new(
                ProblemKind::DeviceTooDeep,
                device(id),
                format!(
                    "{} is {} mm too deep for the door of {name} to close",
                    tag(id),
                    excess.round()
                ),
            ));
        }
        for &(id, gap) in &report.tight {
            out.push(Problem::new(
                ProblemKind::DoorGapTight,
                device(id),
                format!(
                    "{} leaves {} mm to the door of {name}, less than the {} mm wanted",
                    tag(id),
                    gap.round(),
                    novo_layout::enclosure::MIN_DOOR_GAP_MM
                ),
            ));
        }
        for &id in &report.unknown_depth {
            out.push(Problem::new(
                ProblemKind::DepthUnknown,
                device(id),
                format!(
                    "{} has no depth, so the door of {name} was not checked for it",
                    tag(id)
                ),
            ));
        }
        let at = vec![AnyId::Structure(structure.id)];
        if report.plate_too_big {
            out.push(Problem::new(
                ProblemKind::PlateTooBig,
                at.clone(),
                format!("The plate of {name} is larger than the inside of its enclosure"),
            ));
        }
        if let Some(short) = report.front_short {
            out.push(Problem::new(
                ProblemKind::DoorSwing,
                at.clone(),
                format!(
                    "The door of {name} needs {} mm more room in front when open",
                    short.round()
                ),
            ));
        }
        if let Some(short) = report.hinge_side_short {
            out.push(Problem::new(
                ProblemKind::DoorSwing,
                at,
                format!(
                    "The door of {name} needs {} mm more room beside the hinge when open",
                    short.round()
                ),
            ));
        }
    }
}

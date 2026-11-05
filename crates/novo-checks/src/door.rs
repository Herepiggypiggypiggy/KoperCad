//! Door and depth checks of an enclosure, as problems (novo-layout's
//! `CabinetLayout::door_report`). Quiet for a plate whose enclosure has no
//! interior set, so projects without an enclosure see nothing.

use novo_core::{AnyId, DeviceId};
use novo_layout::model::load;
use novo_layout::spec::{EnclosureSpec, Source};
use novo_layout::spec_fit;
use novo_layout::swing::Obstacle;

use crate::{Context, Problem, ProblemKind, Severity};

/// Ends a door or depth message whose weakest number is a program default.
const DEFAULTS_NOTICE: &str = "Estimate from program defaults, not a measurement of your cabinet.";

pub(crate) fn door(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    let tag = |id: u64| {
        project
            .device(DeviceId::from(id))
            .map_or_else(|| "A device".to_owned(), |d| d.tag.to_string())
    };
    let device = |id: u64| vec![AnyId::Device(DeviceId::from(id))];
    for structure in project.structures().filter(|s| s.plate.is_some()) {
        let Some(mut layout) = load(project, structure.id) else {
            continue;
        };
        let name = &structure.tag;
        // A chosen enclosure is the room the checks judge against; a number
        // it lacks is reported, never replaced by a default.
        let spec = EnclosureSpec::read(&structure.properties);
        let mut origin = String::new();
        // A depth taken from a program default is a guess, not a maker's number.
        let mut guessed = false;
        // Shown while any number is a catalogue or default value.
        let notice = spec.notice().map_or_else(String::new, |n| format!(" {n}"));
        if !spec.is_empty() {
            let at = vec![AnyId::Structure(structure.id)];
            match spec_fit::fit(&spec, layout.enclosure.as_ref()) {
                Ok(fit) => {
                    guessed = fit.weakest == Source::Default;
                    // The shared notice names the catalogue; a guessed number is a program default.
                    let notice = match (guessed, notice.is_empty()) {
                        (true, false) => format!(" {DEFAULTS_NOTICE}"),
                        _ => notice,
                    };
                    origin = format!(" (enclosure data: {}){notice}", fit.weakest.words());
                    layout.enclosure = Some(fit.enclosure);
                }
                Err(missing) => out.push(Problem::new(
                    ProblemKind::EnclosureDataMissing,
                    at,
                    format!(
                        "{missing} The door and depth checks of {name} use the typed values, if any"
                    ),
                )),
            }
        }
        let Some(report) = layout.door_report() else {
            continue;
        };
        for &(id, excess) in &report.too_deep {
            let mut problem = Problem::new(
                ProblemKind::DeviceTooDeep,
                device(id),
                format!(
                    "{} is {} mm too deep for the door of {name} to close{origin}",
                    tag(id),
                    excess.round()
                ),
            );
            if guessed {
                problem.severity = Severity::Warning;
            }
            out.push(problem);
        }
        for &(id, gap) in &report.tight {
            out.push(Problem::new(
                ProblemKind::DoorGapTight,
                device(id),
                format!(
                    "{} leaves {} mm to the door of {name}, less than the {} mm wanted{origin}",
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
        // Devices on the door against the walls and the plate devices as the
        // door opens (novo-layout's door swing sweep).
        if let Some(swing) = layout.swing_report() {
            for hit in &swing.hits {
                let mut at = device(hit.door_device);
                if let Obstacle::Device(id) = hit.obstacle {
                    at.push(AnyId::Device(DeviceId::from(id)));
                }
                out.push(Problem::new(ProblemKind::DoorSwing, at, hit.message(&tag)));
            }
        }
        let at = vec![AnyId::Structure(structure.id)];
        if report.plate_too_big {
            out.push(Problem::new(
                ProblemKind::PlateTooBig,
                at.clone(),
                format!("The plate of {name} is larger than the inside of its enclosure{origin}"),
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

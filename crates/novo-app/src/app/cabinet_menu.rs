//! The right-click menu of the Cabinet plate: what is under the pointer
//! decides the rows. A device offers rotate and take off the plate, a rail
//! or duct offers delete, and empty plate offers the plate-wide actions
//! (Place all, Rails and ducts, Route all, Size enclosure, Heat, Table, 3D).
//! Every row runs a registered command, so the menu is a second way to the
//! same actions as the toolbar and the command field.
//!
//! Where the user finds it: right-click on the plate in the Cabinet.

use eframe::egui;

/// What the click landed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    Device(u64),
    Rail(u64),
    Plate,
}

/// Which canvas the menu belongs to; the rows differ ("Show in 3D" is for the
/// plate, "Show in Cabinet" for the 3D view).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Face {
    Plate,
    View3d,
}

/// The open menu.
#[derive(Debug, Clone, Copy)]
pub(super) struct PlateMenu {
    pub face: Face,
    pub target: Target,
    pub anchor: egui::Pos2,
    /// Opened this frame, so the click that opened it does not close it.
    opened: bool,
}

impl PlateMenu {
    pub(super) fn new(face: Face, target: Target, anchor: egui::Pos2) -> Self {
        Self {
            face,
            target,
            anchor,
            opened: true,
        }
    }
}

/// `(label with key, command id)` rows for a target, `None` for a separator.
pub(super) fn rows(face: Face, target: Target) -> Vec<Option<(&'static str, &'static str)>> {
    if face == Face::View3d {
        return match target {
            Target::Device(_) => vec![
                Some(("Rotate   R", "edit.rotate")),
                Some(("Duplicate   Ctrl+D", "view.3d_duplicate")),
                Some(("Take off the plate   Del", "edit.delete")),
                None,
                Some(("Show in Cabinet", "view.cabinet")),
            ],
            Target::Rail(_) => vec![
                Some(("Delete   Del", "edit.delete")),
                None,
                Some(("Rails and ducts…   Ctrl+Shift+R", "cabinet.rails")),
            ],
            Target::Plate => vec![
                Some(("Zoom to fit   F", "view.fit")),
                Some(("Isometric", "view.iso")),
                Some(("Front", "view.front")),
                Some(("Top", "view.top")),
                None,
                Some(("Door open or closed", "view.3d_door")),
                Some(("Enclosure on or off", "view.3d_shell")),
                Some(("Wires on or off", "view.3d_wires")),
                Some(("Shadows on or off", "view.3d_shadows")),
                None,
                Some(("Simulate heat", "heat.simulate")),
                Some(("Show in Cabinet", "view.cabinet")),
            ],
        };
    }
    match target {
        Target::Device(_) => vec![
            Some(("Rotate   R", "edit.rotate")),
            Some(("Take off the plate   Del", "edit.delete")),
            None,
            Some(("Show in 3D", "view.3d")),
        ],
        Target::Rail(_) => vec![
            Some(("Delete   Del", "edit.delete")),
            None,
            Some(("Rails and ducts…   Ctrl+Shift+R", "cabinet.rails")),
        ],
        Target::Plate => vec![
            Some(("Place all", "cabinet.place-all")),
            Some(("Rails and ducts…   Ctrl+Shift+R", "cabinet.rails")),
            Some(("Route all wires", "cabinet.routes")),
            Some(("Size the enclosure…", "cabinet.size-enclosure")),
            None,
            Some(("Simulate heat", "heat.simulate")),
            Some(("Table   Ctrl+T", "view.cabinet_table")),
            Some(("Show in 3D", "view.3d")),
            Some(("Zoom to fit", "view.fit")),
        ],
    }
}

/// Draws the menu; returns the command picked, if any. Closes `menu` on a
/// pick, a click elsewhere or Escape.
pub(super) fn show(ctx: &egui::Context, menu: &mut Option<PlateMenu>) -> Option<&'static str> {
    let open = (*menu)?;
    let mut picked = None;
    let area = egui::Area::new(egui::Id::new("cabinet_plate_menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(open.anchor)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(190.0);
                for row in rows(open.face, open.target) {
                    match row {
                        Some((label, command)) => {
                            if ui.button(label).clicked() {
                                picked = Some(command);
                            }
                        }
                        None => {
                            ui.separator();
                        }
                    }
                }
            });
        });
    let outside =
        !open.opened && ctx.input(|i| i.pointer.any_pressed()) && !area.response.contains_pointer();
    let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    if picked.is_some() || outside || escape {
        *menu = None;
    } else if let Some(m) = menu {
        m.opened = false;
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_runs_a_registered_command() {
        for (face, target) in [Face::Plate, Face::View3d]
            .into_iter()
            .flat_map(|f| [Target::Device(1), Target::Rail(1), Target::Plate].map(|t| (f, t)))
        {
            for (_, command) in rows(face, target).into_iter().flatten() {
                assert!(
                    novo_workflow::commands::is_known(command),
                    "{command} is not a command"
                );
                assert!(
                    super::super::workbench::app_runs(command),
                    "{command} does not run yet"
                );
            }
        }
    }
}

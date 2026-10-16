//! Saved camera views in the 3D toolbar: a menu of the person's own views,
//! kept in the settings (per user, not in the project).

use eframe::egui;
use glam::Vec3;
use novo_view3d::{Camera, Projection};

use crate::settings::CameraBookmark;

/// How many views the menu keeps.
pub(super) const MAX_VIEWS: usize = 12;

/// What the Views menu asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ViewsAction {
    Go(usize),
    Save,
    Delete(usize),
}

pub(super) fn bookmark(name: String, c: &Camera) -> CameraBookmark {
    CameraBookmark {
        name,
        target: c.target.to_array(),
        yaw: c.yaw,
        pitch: c.pitch,
        distance: c.distance,
        fov_y: c.fov_y,
        orthographic: c.projection == Projection::Orthographic,
    }
}

/// The camera a bookmark names. Values from a hand-edited settings file are
/// held to what the camera can do.
pub(super) fn camera_of(b: &CameraBookmark) -> Camera {
    let finite = |v: f32, d: f32| if v.is_finite() { v } else { d };
    let d = Camera::default();
    Camera {
        target: Vec3::from_array(b.target.map(|v| finite(v, 0.0))),
        yaw: finite(b.yaw, d.yaw),
        pitch: finite(b.pitch, d.pitch).clamp(-1.55, 1.55),
        distance: finite(b.distance, d.distance).clamp(10.0, 100_000.0),
        fov_y: finite(b.fov_y, d.fov_y).clamp(0.1, 2.0),
        projection: if b.orthographic {
            Projection::Orthographic
        } else {
            Projection::Perspective
        },
    }
}

/// The first "View n" not in use.
pub(super) fn next_name(views: &[CameraBookmark]) -> String {
    (1..)
        .map(|n| format!("View {n}"))
        .find(|name| !views.iter().any(|v| &v.name == name))
        .expect("a free number exists")
}

/// Saves the camera as a new view; false when the menu is full.
pub(super) fn save(views: &mut Vec<CameraBookmark>, camera: &Camera) -> bool {
    if views.len() >= MAX_VIEWS {
        return false;
    }
    let name = next_name(views);
    views.push(bookmark(name, camera));
    true
}

/// The Views menu in the toolbar.
pub(super) fn menu(ui: &mut egui::Ui, views: &[CameraBookmark]) -> Option<ViewsAction> {
    let mut action = None;
    ui.menu_button("Views", |ui| {
        if ui
            .add_enabled(views.len() < MAX_VIEWS, egui::Button::new("Save this view"))
            .on_hover_text("Remember where the camera is now (kept for you, not in the project)")
            .clicked()
        {
            action = Some(ViewsAction::Save);
            ui.close();
        }
        if views.is_empty() {
            ui.weak("No saved views yet");
        } else {
            ui.separator();
        }
        for (i, v) in views.iter().enumerate() {
            ui.horizontal(|ui| {
                if ui.button(&v.name).clicked() {
                    action = Some(ViewsAction::Go(i));
                    ui.close();
                }
                if ui
                    .small_button("✕")
                    .on_hover_text("Forget this view")
                    .clicked()
                {
                    action = Some(ViewsAction::Delete(i));
                    ui.close();
                }
            });
        }
    });
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam() -> Camera {
        Camera {
            target: Vec3::new(300.0, 400.0, 20.0),
            yaw: 0.7,
            pitch: 0.3,
            distance: 2500.0,
            fov_y: 0.6,
            projection: Projection::Orthographic,
        }
    }

    #[test]
    fn a_saved_view_gives_the_same_camera_back() {
        let c = cam();
        assert_eq!(camera_of(&bookmark("A".into(), &c)), c);
    }

    #[test]
    fn names_take_the_first_free_number_and_the_menu_has_a_limit() {
        let mut views = Vec::new();
        assert!(save(&mut views, &cam()));
        assert!(save(&mut views, &cam()));
        views.remove(0);
        assert_eq!(next_name(&views), "View 1");
        while save(&mut views, &cam()) {}
        assert_eq!(views.len(), MAX_VIEWS);
    }

    #[test]
    fn a_hand_edited_view_cannot_break_the_camera() {
        let mut b = bookmark("X".into(), &cam());
        b.distance = f32::NAN;
        b.pitch = 9.0;
        b.fov_y = 0.0;
        let c = camera_of(&b);
        assert!(c.distance.is_finite() && c.pitch <= 1.55 && c.fov_y >= 0.1);
    }
}

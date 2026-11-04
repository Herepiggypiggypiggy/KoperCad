//! Problem triangles in the 3D view (lane T3 of docs/design/problem-triangles.md).
//!
//! This is the shared 3D flag painter: lanes T2, T4 and T6 hand it flags
//! anchored on the cabinet and it draws them. A flag is drawn as the same
//! triangle as everywhere else, at constant screen size (14 pt), at the
//! projected spot of its anchor. A triangle whose spot lies behind a part is
//! faded, a triangle on a selected part is drawn last (on top) and never
//! faded, triangles that land within 14 px of each other become one triangle
//! with a count. Every triangle can be reached with Tab and opened with
//! Enter. Its accessible name reads "Error: Door cannot close".
//!
//! Anchors drawn here: Device, Duct, Rail, Door, Enclosure (scene items with
//! the model's raw id), Plate (a point on the mounting plate), World (a point
//! in 3D) and Opening or Wire (the spot the caller passes in `Layer::extra`:
//! the 3D wire runs are routes, not scene items, so a schematic wire id is
//! never matched to one by number).
//!
//! Refusals of 3D edits (a duct over a duct, no room for a copy) are
//! [`Transients`]: yellow triangles on the selection that go on the next
//! input. They are not in the flag store, so they never count.

use eframe::egui;
use glam::Vec3;
use novo_flags::{Anchor, Detail, Flag, FlagDraft, FlagId, Life, Severity, Source};
use novo_view3d::Scene;
use novo_view3d::scene::{ItemId, ItemKind};
use novo_view3d::widget::View3d;

/// Side of a triangle, pt: the same at every window size.
pub(super) const SIZE: f32 = 14.0;
/// Side of the area that takes a click, pt.
pub(super) const HIT: f32 = 24.0;
/// Triangles closer than this, px, become one with a count.
pub(super) const CLUSTER_PX: f32 = 14.0;
/// Opacity of a triangle behind a part.
pub(super) const HIDDEN_ALPHA: f32 = 0.35;
/// A refusal stays at most this long, seconds, when no input comes.
const TRANSIENT_SECS: f64 = 8.0;

/// What the painter needs to put a world point on the screen.
pub(super) struct Layer<'a> {
    pub view: &'a View3d,
    pub rect: egui::Rect,
    pub scene: &'a Scene,
    /// Spots the scene has no item for (the openings of the Heat panel).
    pub extra: &'a [(Anchor, Vec3)],
}

/// A triangle the pointer or the keyboard asked to open.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Hit {
    pub flag: FlagId,
    /// Where it is on the screen; the popover opens beside it.
    pub at: egui::Pos2,
    /// A refusal: it has a tooltip, no popover.
    pub transient: bool,
}

/// The scene item an anchor stands for, `None` for spots and non-scene things.
pub(super) fn item_of(anchor: &Anchor) -> Option<ItemId> {
    let (kind, raw) = match anchor {
        Anchor::Device(r) => (ItemKind::Device, r),
        Anchor::Duct(r) => (ItemKind::Duct, r),
        Anchor::Rail(r) => (ItemKind::Rail, r),
        Anchor::Door(r) => (ItemKind::Door, r),
        Anchor::Enclosure(r) => (ItemKind::Enclosure, r),
        _ => return None,
    };
    Some(ItemId::new(kind, raw.parse().ok()?))
}

/// Where an anchor is in the world: the middle of the front face of its item
/// (the face away from the plate, as the heat labels do), the given plate or
/// world point, or the spot the caller passed in `extra`.
pub(super) fn world_of(scene: &Scene, extra: &[(Anchor, Vec3)], anchor: &Anchor) -> Option<Vec3> {
    if let Some(id) = item_of(anchor) {
        let item = scene.items.iter().find(|i| i.id == id)?;
        return Some(
            scene
                .world_matrix(item)
                .transform_point3(Vec3::new(0.0, 0.0, item.size.z)),
        );
    }
    match anchor {
        Anchor::Plate(_, [x, y]) => Some(scene.frame.world(*x as f64, *y as f64, 0.0)),
        Anchor::World(_, [x, y, z]) => Some(Vec3::new(*x as f32, *y as f32, *z as f32)),
        other => extra.iter().find(|(a, _)| a == other).map(|(_, p)| *p),
    }
}

/// One spot on the screen and the flags that landed there.
#[derive(Debug, Clone, PartialEq)]
struct Spot {
    at: egui::Pos2,
    /// Indices into the flag list, worst first.
    flags: Vec<usize>,
    selected: bool,
    hidden: bool,
}

/// Groups the points that lie within [`CLUSTER_PX`] of an earlier group's
/// first point. Order is kept: the first point of a group names its place.
fn cluster(points: &[(egui::Pos2, usize)]) -> Vec<(egui::Pos2, Vec<usize>)> {
    let mut groups: Vec<(egui::Pos2, Vec<usize>)> = Vec::new();
    for (at, i) in points {
        match groups
            .iter_mut()
            .find(|(g, _)| g.distance(*at) <= CLUSTER_PX)
        {
            Some((_, list)) => {
                if !list.contains(i) {
                    list.push(*i);
                }
            }
            None => groups.push((*at, vec![*i])),
        }
    }
    groups
}

fn spots(layer: &Layer, flags: &[&Flag]) -> Vec<Spot> {
    let selection = &layer.view.state.selection;
    // (position, flag index, item the point belongs to, hidden)
    let mut points: Vec<(egui::Pos2, usize)> = Vec::new();
    let mut facts: Vec<(egui::Pos2, bool, bool)> = Vec::new();
    for (i, flag) in flags.iter().enumerate() {
        for anchor in &flag.anchors {
            let Some(world) = world_of(layer.scene, layer.extra, anchor) else {
                continue;
            };
            let Some(at) = layer.view.project(layer.rect, layer.scene, world) else {
                continue;
            };
            if !layer.rect.contains(at) {
                continue;
            }
            let own = item_of(anchor);
            let selected = own.is_some_and(|o| selection.contains(&o));
            let hidden = !selected
                && layer
                    .view
                    .hidden_behind(layer.rect, layer.scene, world, own);
            points.push((at, i));
            facts.push((at, selected, hidden));
        }
    }
    let mut out: Vec<Spot> = cluster(&points)
        .into_iter()
        .map(|(at, mut list)| {
            list.sort_by_key(|i| flags[*i].severity.rank());
            let near = |(p, _, _): &&(egui::Pos2, bool, bool)| p.distance(at) <= CLUSTER_PX;
            let selected = facts.iter().filter(near).any(|f| f.1);
            // One visible flag in the group is enough to keep it unfaded.
            let hidden = !selected && facts.iter().filter(near).all(|f| f.2);
            Spot {
                at,
                flags: list,
                selected,
                hidden,
            }
        })
        .collect();
    // Selected last: drawn on top.
    out.sort_by_key(|s| s.selected);
    out
}

/// Draws the triangles of `flags` over the 3D view and returns the one that
/// was clicked, or opened with Enter.
pub(super) fn paint(ui: &mut egui::Ui, layer: &Layer, flags: &[&Flag]) -> Option<Hit> {
    if flags.is_empty() {
        return None;
    }
    let pal = crate::theme::ui_palette();
    let mut hit = None;
    for (n, spot) in spots(layer, flags).iter().enumerate() {
        let worst = flags[spot.flags[0]];
        let rect = egui::Rect::from_center_size(spot.at, egui::Vec2::splat(HIT));
        let id = egui::Id::new(("view3d.flag", worst.id.0, n));
        let response = ui.interact(rect, id, egui::Sense::click());
        let name = if spot.flags.len() > 1 {
            format!(
                "{} (and {} more)",
                worst.accessible_name(),
                spot.flags.len() - 1
            )
        } else {
            worst.accessible_name()
        };
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name.clone())
        });
        let mut painter = ui.painter().with_clip_rect(layer.rect);
        if spot.hidden && !response.hovered() && !response.has_focus() {
            painter.set_opacity(HIDDEN_ALPHA);
        }
        glyph(&painter, spot.at, worst, pal);
        if spot.flags.len() > 1 {
            badge(&painter, spot.at, spot.flags.len(), pal);
        }
        if response.has_focus() {
            painter.circle_stroke(
                spot.at,
                HIT * 0.5,
                egui::Stroke::new(2.0, ui.visuals().selection.stroke.color),
            );
        }
        let response = response.on_hover_text(name);
        if response.clicked() {
            hit = Some(Hit {
                flag: worst.id,
                at: spot.at,
                transient: matches!(worst.life, Life::Run(ref r) if r == TRANSIENT_RUN),
            });
        }
    }
    hit
}

/// One triangle of a flag, centred on `at`: red solid, yellow hollow, dashed
/// when the flag could not be judged.
pub(super) fn glyph(
    painter: &egui::Painter,
    at: egui::Pos2,
    flag: &Flag,
    pal: &novo_theme::Palette,
) {
    novo_theme::widgets::flag_triangle(
        painter,
        at,
        SIZE,
        flag.severity == Severity::Red,
        flag.unchecked,
        pal,
    );
}

/// The count of a group, small, at the upper right of its triangle.
fn badge(painter: &egui::Painter, at: egui::Pos2, n: usize, pal: &novo_theme::Palette) {
    let c = at + egui::vec2(SIZE * 0.55, -SIZE * 0.55);
    painter.circle_filled(c, 5.5, pal.text_primary);
    painter.text(
        c,
        egui::Align2::CENTER_CENTER,
        n.min(9).to_string(),
        egui::FontId::proportional(9.0),
        pal.sunken,
    );
}

/// The `Life::Run` key that marks a refusal (so the painter knows it has no
/// popover).
const TRANSIENT_RUN: &str = "view3d.refusal";

/// Refusals of 3D edits: a yellow triangle on the selection, gone on the
/// next press or key (or after a few seconds).
#[derive(Debug, Default)]
pub(super) struct Transients {
    items: Vec<Transient>,
    /// Flags that last as long as something is being typed: set every frame
    /// by the caller, drawn once, then dropped.
    typing: Vec<Flag>,
    /// The typing flags of the frame being drawn.
    drawn_typing: Vec<Flag>,
}

#[derive(Debug)]
struct Transient {
    flag: Flag,
    /// Set by the first frame that drew it; input before that frame is the
    /// input that caused the refusal.
    born: Option<f64>,
}

impl Transients {
    /// A refused edit: `title` is the noun phrase (at most 28 characters),
    /// `lines` the numbers and what to do, `anchors` the parts concerned.
    pub fn refuse(&mut self, title: &str, lines: &[&str], anchors: Vec<Anchor>) {
        let flag = Flag::from_draft(FlagDraft {
            severity: Severity::Yellow,
            unchecked: false,
            source: Source::Layout,
            rule: format!("view3d.refusal.{title}"),
            anchors,
            title: title.to_owned(),
            detail: Detail::new(lines.iter().map(|l| (*l).to_owned())),
            actions: Vec::new(),
            waiver_key: None,
            life: Life::Run(TRANSIENT_RUN.to_owned()),
        });
        self.items.retain(|t| t.flag.id != flag.id);
        self.items.push(Transient { flag, born: None });
    }

    /// A problem with a value being typed (a rail pushed into something): it
    /// is drawn on the next frame and gone unless set again, so it lasts as
    /// long as the typed value does and the typing itself does not end it.
    pub fn typing(&mut self, title: &str, lines: &[String], anchors: Vec<Anchor>) {
        let flag = Flag::from_draft(FlagDraft {
            severity: Severity::Yellow,
            unchecked: false,
            source: Source::Layout,
            rule: format!("view3d.typing.{title}"),
            anchors,
            title: title.to_owned(),
            detail: Detail::new(lines.iter().cloned()),
            actions: Vec::new(),
            waiver_key: None,
            life: Life::Run(TRANSIENT_RUN.to_owned()),
        });
        self.typing.push(flag);
    }

    /// The refusals to draw this frame. Drops those that a later press, key
    /// or the timeout has ended.
    pub fn live(&mut self, ctx: &egui::Context) -> Vec<&Flag> {
        let (now, input) = ctx.input(|i| {
            (
                i.time,
                i.events.iter().any(|e| {
                    matches!(
                        e,
                        egui::Event::Key { pressed: true, .. }
                            | egui::Event::PointerButton { pressed: true, .. }
                    )
                }),
            )
        });
        self.items.retain_mut(|t| match t.born {
            None => {
                t.born = Some(now);
                true
            }
            Some(born) => !input && now - born < TRANSIENT_SECS,
        });
        if !self.items.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
        // Drawn this once; the caller sets them again while they hold.
        let typing = std::mem::take(&mut self.typing);
        self.drawn_typing = typing;
        self.items
            .iter()
            .map(|t| &t.flag)
            .chain(self.drawn_typing.iter())
            .collect()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.typing.is_empty()
    }
}

impl super::NovoApp {
    /// Locate for a flag that is not an Issues check: select the parts it is
    /// anchored on in the 3D view, so its triangle is drawn on top.
    pub(super) fn locate_flag_in_3d(&mut self, flag: &Flag) {
        let items: Vec<ItemId> = flag.anchors.iter().filter_map(item_of).collect();
        if !items.is_empty() {
            self.view3d.view.state.selection = items.into_iter().collect();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_view3d::{Item, PlateFrame, Pose, Shape};

    fn scene() -> Scene {
        let mut s = Scene::new(PlateFrame {
            width: 600.0,
            height: 400.0,
        });
        let mut dev = Item::new(
            ItemId::new(ItemKind::Device, 7),
            Shape::Box,
            Pose::IDENTITY,
            Vec3::new(40.0, 60.0, 30.0),
        );
        dev.tint = None;
        s.items.push(dev);
        s
    }

    #[test]
    fn scene_anchors_map_to_items_and_spots_do_not() {
        assert_eq!(
            item_of(&Anchor::Device("7".into())),
            Some(ItemId::new(ItemKind::Device, 7))
        );
        assert_eq!(
            item_of(&Anchor::Duct("3".into())),
            Some(ItemId::new(ItemKind::Duct, 3))
        );
        assert_eq!(item_of(&Anchor::Device("x".into())), None);
        assert_eq!(item_of(&Anchor::Project), None);
        assert_eq!(
            item_of(&Anchor::Wire("7".into())),
            None,
            "3D wire runs are routes, not items with the schematic wire id"
        );
        assert_eq!(item_of(&Anchor::Plate("1".into(), [0, 0])), None);
    }

    #[test]
    fn a_device_flag_sits_on_its_front_face() {
        let s = scene();
        let p = world_of(&s, &[], &Anchor::Device("7".into())).unwrap();
        // Front face: the local height, 30 mm above the plate.
        assert!((p.z - 30.0).abs() < 1e-4, "{p:?}");
        assert!(world_of(&s, &[], &Anchor::Device("8".into())).is_none());
    }

    #[test]
    fn plate_world_and_extra_anchors_resolve() {
        let s = scene();
        let plate = world_of(&s, &[], &Anchor::Plate("1".into(), [100, 50])).unwrap();
        assert_eq!(plate, s.frame.world(100.0, 50.0, 0.0));
        let world = world_of(&s, &[], &Anchor::World("1".into(), [1, 2, 3])).unwrap();
        assert_eq!(world, Vec3::new(1.0, 2.0, 3.0));
        let opening = Anchor::Opening("0".into());
        let extra = [(opening.clone(), Vec3::new(5.0, 6.0, 7.0))];
        assert_eq!(
            world_of(&s, &extra, &opening),
            Some(Vec3::new(5.0, 6.0, 7.0))
        );
        assert!(world_of(&s, &[], &opening).is_none());
    }

    #[test]
    fn near_points_become_one_group_with_all_flags() {
        let pts = [
            (egui::pos2(100.0, 100.0), 0),
            (egui::pos2(108.0, 104.0), 1),
            (egui::pos2(300.0, 100.0), 2),
            (egui::pos2(100.0, 100.0), 0),
        ];
        let g = cluster(&pts);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].1, vec![0, 1], "no flag twice in a group");
        assert_eq!(g[1].1, vec![2]);
        // Just outside the distance stays apart.
        let far = [
            (egui::pos2(0.0, 0.0), 0),
            (egui::pos2(CLUSTER_PX + 0.5, 0.0), 1),
        ];
        assert_eq!(cluster(&far).len(), 2);
    }

    fn ctx_with(events: Vec<egui::Event>, time: f64) -> egui::Context {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(
            egui::RawInput {
                time: Some(time),
                events,
                ..Default::default()
            },
            |_| {},
        );
        out.textures_delta.clear();
        ctx
    }

    fn press() -> egui::Event {
        egui::Event::PointerButton {
            pos: egui::Pos2::ZERO,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }
    }

    #[test]
    fn a_refusal_is_yellow_stays_until_the_next_input_and_never_counts() {
        let mut t = Transients::default();
        t.refuse(
            "No room for a copy",
            &["Free some plate space first"],
            vec![Anchor::Device("7".into())],
        );
        // The frame that caused it, even with a press in it, keeps it.
        assert_eq!(t.live(&ctx_with(vec![press()], 1.0)).len(), 1);
        let f = t.live(&ctx_with(vec![], 2.0));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, Severity::Yellow);
        assert!(!f[0].unchecked);
        assert_eq!(f[0].accessible_name(), "Warning: No room for a copy");
        // The next press ends it.
        assert!(t.live(&ctx_with(vec![press()], 3.0)).is_empty());
        assert!(t.is_empty());
    }

    #[test]
    fn a_typed_value_problem_lasts_only_while_it_is_set_and_typing_does_not_end_it() {
        let mut t = Transients::default();
        let key = egui::Event::Key {
            key: egui::Key::Num5,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        };
        t.typing(
            "Rail hits a device",
            &["-K1 is in the way".to_owned()],
            vec![Anchor::Rail("2".into())],
        );
        assert_eq!(t.live(&ctx_with(vec![key.clone()], 1.0)).len(), 1);
        t.typing(
            "Rail hits a device",
            &["-K1 is in the way".to_owned()],
            vec![Anchor::Rail("2".into())],
        );
        assert_eq!(t.live(&ctx_with(vec![key], 1.1)).len(), 1);
        // Not set again: gone.
        assert!(t.live(&ctx_with(vec![], 1.2)).is_empty());
    }

    #[test]
    fn a_refusal_ends_by_itself_and_the_same_refusal_is_not_listed_twice() {
        let mut t = Transients::default();
        for _ in 0..2 {
            t.refuse("Duct over a duct", &[], vec![Anchor::Duct("3".into())]);
        }
        assert_eq!(t.live(&ctx_with(vec![], 1.0)).len(), 1);
        assert!(
            t.live(&ctx_with(vec![], 1.0 + TRANSIENT_SECS + 0.1))
                .is_empty()
        );
    }
}

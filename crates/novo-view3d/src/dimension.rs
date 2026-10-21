//! Dimensions drawn over the 3D image: the enclosure's overall size and, for
//! one selected part, its size and the clear distance to what is next to it
//! or to the plate edge, in millimetres. GPU free: this only decides which
//! measuring lines exist; the widget projects and paints them.
//!
//! The distances are what the workshop needs to drill by hand: from the
//! part's edge to the plate edge or to its neighbour, along the plate.

use crate::math::Aabb;
use crate::scene::{ItemId, ItemKind, Scene};
use glam::Vec3;
use std::collections::BTreeSet;

/// Gaps smaller than this are not drawn (touching parts): millimetres.
const MIN_GAP_MM: f32 = 0.05;
/// How far the overall dimension lines stand off the enclosure: millimetres.
const STAND_OFF_MM: f32 = 25.0;
/// How far a part's size lines stand off the part: millimetres.
const PART_OFF_MM: f32 = 6.0;

/// What a line measures, for styling and for the tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Width, height or depth of the enclosure.
    Overall,
    /// Width, height or depth of the selected part.
    Size,
    /// Clear distance from the selected part to a neighbour.
    Gap,
    /// Clear distance from the selected part to the plate edge.
    Edge,
}

/// One measuring line from `a` to `b` in world millimetres.
#[derive(Debug, Clone, PartialEq)]
pub struct Dimension {
    pub a: Vec3,
    pub b: Vec3,
    pub role: Role,
    /// The measured length, mm.
    pub mm: f32,
    /// The text drawn on the line, for example `W 54 mm`.
    pub text: String,
    /// For a gap between two devices: the clear distance the two parts ask
    /// for on the facing sides, mm.
    pub required: Option<f32>,
}

impl Dimension {
    /// `Some(false)` when a gap is smaller than the parts ask for.
    pub fn clearance_ok(&self) -> Option<bool> {
        self.required.map(|r| self.mm + CLEARANCE_EPS_MM >= r)
    }
}

/// Rounding slack when comparing a measured gap with the required one.
const CLEARANCE_EPS_MM: f32 = 0.05;

/// Millimetres as written on a line: one decimal, none when it is whole.
pub fn format_mm(mm: f32) -> String {
    let tenths = (mm * 10.0).round() as i64;
    if tenths % 10 == 0 {
        format!("{} mm", tenths / 10)
    } else {
        format!("{:.1} mm", tenths as f32 / 10.0)
    }
}

fn line(a: Vec3, b: Vec3, role: Role, prefix: &str) -> Dimension {
    let mm = (b - a).length();
    let text = if prefix.is_empty() {
        format_mm(mm)
    } else {
        format!("{prefix} {}", format_mm(mm))
    };
    Dimension {
        a,
        b,
        role,
        mm,
        text,
        required: None,
    }
}

/// The dimensions to show for `scene` with `selection`. The overall size of
/// the enclosure (its shell, else the plate) is always there; one selected
/// part adds its size and its distances. Several selected parts add nothing:
/// the picture would be crowded and a distance needs one part.
pub fn dimensions(scene: &Scene, selection: &BTreeSet<ItemId>) -> Vec<Dimension> {
    let mut out = Vec::new();
    let plate = bounds_of(scene, ItemKind::Plate);
    let shell = bounds_of(scene, ItemKind::Enclosure);
    let overall = shell.or(plate);
    if let Some(b) = overall.filter(|b| !b.is_empty()) {
        overall_lines(&b, shell.is_some(), &mut out);
    }
    let mut chosen = selection.iter().filter_map(|id| scene.get(*id));
    if let (Some(item), None) = (chosen.next(), chosen.next())
        && !matches!(
            item.id.kind,
            ItemKind::Plate | ItemKind::Enclosure | ItemKind::Door | ItemKind::Wire
        )
    {
        let b = scene.world_bounds(item);
        part_lines(scene, item.id, &b, plate.as_ref(), &mut out);
    }
    out
}

/// The free space a selected device asks for, as the four corners of a
/// rectangle on the surface it stands on: its footprint grown by the need on
/// each side. Devices that ask for nothing get none.
pub fn clearance_zones(scene: &Scene, selection: &BTreeSet<ItemId>) -> Vec<[Vec3; 4]> {
    selection
        .iter()
        .filter_map(|id| scene.get(*id))
        .filter(|i| i.id.kind == ItemKind::Device)
        .filter_map(|i| {
            let need = i.needs.filter(|n| n.iter().any(|v| *v > 0.0))?;
            let b = scene.world_bounds(i);
            // Left, right, above (higher y), below, as in `Item::needs`.
            let (x0, x1) = (b.min.x - need[0], b.max.x + need[1]);
            let (y0, y1) = (b.min.y - need[3], b.max.y + need[2]);
            let z = b.min.z;
            Some([
                Vec3::new(x0, y0, z),
                Vec3::new(x1, y0, z),
                Vec3::new(x1, y1, z),
                Vec3::new(x0, y1, z),
            ])
        })
        .collect()
}

fn bounds_of(scene: &Scene, kind: ItemKind) -> Option<Aabb> {
    let mut b = Aabb::EMPTY;
    for i in scene.items.iter().filter(|i| i.id.kind == kind) {
        b = b.union(&scene.world_bounds(i));
    }
    (!b.is_empty()).then_some(b)
}

/// Width below the enclosure, height at its left, depth along its bottom
/// left edge, each standing off so the line does not cover the box. Without
/// a shell the box is only the plate, whose third side is its thickness, not
/// a depth of the enclosure: then there is no depth line.
fn overall_lines(b: &Aabb, has_shell: bool, out: &mut Vec<Dimension>) {
    let z = b.max.z;
    out.push(line(
        Vec3::new(b.min.x, b.min.y - STAND_OFF_MM, z),
        Vec3::new(b.max.x, b.min.y - STAND_OFF_MM, z),
        Role::Overall,
        "W",
    ));
    out.push(line(
        Vec3::new(b.min.x - STAND_OFF_MM, b.min.y, z),
        Vec3::new(b.min.x - STAND_OFF_MM, b.max.y, z),
        Role::Overall,
        "H",
    ));
    if has_shell && b.max.z - b.min.z > 0.5 {
        out.push(line(
            Vec3::new(b.min.x - STAND_OFF_MM, b.min.y - STAND_OFF_MM, b.min.z),
            Vec3::new(b.min.x - STAND_OFF_MM, b.min.y - STAND_OFF_MM, b.max.z),
            Role::Overall,
            "D",
        ));
    }
}

fn part_lines(scene: &Scene, id: ItemId, b: &Aabb, plate: Option<&Aabb>, out: &mut Vec<Dimension>) {
    let z = b.max.z;
    let (cx, cy) = ((b.min.x + b.max.x) * 0.5, (b.min.y + b.max.y) * 0.5);
    // Size: width under the part, height on its left, depth on its lower right edge.
    out.push(line(
        Vec3::new(b.min.x, b.min.y - PART_OFF_MM, z),
        Vec3::new(b.max.x, b.min.y - PART_OFF_MM, z),
        Role::Size,
        "W",
    ));
    out.push(line(
        Vec3::new(b.min.x - PART_OFF_MM, b.min.y, z),
        Vec3::new(b.min.x - PART_OFF_MM, b.max.y, z),
        Role::Size,
        "H",
    ));
    if b.max.z - b.min.z > 0.5 {
        out.push(line(
            Vec3::new(b.max.x + PART_OFF_MM, b.min.y, b.min.z),
            Vec3::new(b.max.x + PART_OFF_MM, b.min.y, b.max.z),
            Role::Size,
            "D",
        ));
    }

    // Neighbours: what stands wholly beside the part and overlaps it across.
    let others: Vec<(Aabb, Option<[f32; 4]>)> = scene
        .items
        .iter()
        .filter(|i| i.id != id)
        .filter(|i| {
            matches!(
                i.id.kind,
                ItemKind::Device | ItemKind::Duct | ItemKind::Rail | ItemKind::Other
            )
        })
        .map(|i| (scene.world_bounds(i), i.needs))
        .collect();
    let own = scene.get(id).and_then(|i| i.needs);
    let overlap_y = |o: &Aabb| o.min.y < b.max.y && o.max.y > b.min.y;
    let overlap_x = |o: &Aabb| o.min.x < b.max.x && o.max.x > b.min.x;

    // (nearest neighbour edge, or the plate edge) on each side.
    // Each side gives (edge position, is a neighbour, the neighbour's needs).
    type Near = (f32, bool, Option<[f32; 4]>);
    let left: Option<Near> = others
        .iter()
        .filter(|(o, _)| overlap_y(o) && o.max.x <= b.min.x)
        .map(|(o, n)| (o.max.x, true, *n))
        .max_by(|a, c| a.0.total_cmp(&c.0))
        .or_else(|| plate.map(|p| (p.min.x, false, None)));
    let right: Option<Near> = others
        .iter()
        .filter(|(o, _)| overlap_y(o) && o.min.x >= b.max.x)
        .map(|(o, n)| (o.min.x, true, *n))
        .min_by(|a, c| a.0.total_cmp(&c.0))
        .or_else(|| plate.map(|p| (p.max.x, false, None)));
    let below: Option<Near> = others
        .iter()
        .filter(|(o, _)| overlap_x(o) && o.max.y <= b.min.y)
        .map(|(o, n)| (o.max.y, true, *n))
        .max_by(|a, c| a.0.total_cmp(&c.0))
        .or_else(|| plate.map(|p| (p.min.y, false, None)));
    let above: Option<Near> = others
        .iter()
        .filter(|(o, _)| overlap_x(o) && o.min.y >= b.max.y)
        .map(|(o, n)| (o.min.y, true, *n))
        .min_by(|a, c| a.0.total_cmp(&c.0))
        .or_else(|| plate.map(|p| (p.max.y, false, None)));

    let role = |neighbour: bool| if neighbour { Role::Gap } else { Role::Edge };
    // `side`/`facing` index `Item::needs` (left, right, above, below): the
    // part's own need on that side and the neighbour's on the side facing it.
    let mut gap = |from: Vec3, to: Vec3, near: Near, side: usize, facing: usize| {
        if (to - from).length() >= MIN_GAP_MM {
            let mut d = line(from, to, role(near.1), "");
            if let (Some(own), Some(other)) = (own, near.2) {
                let need = own[side].max(other[facing]);
                d.required = Some(need);
                d.text = format!("{} (min {})", d.text, format_mm(need));
            }
            out.push(d);
        }
    };
    if let Some(n) = left.filter(|n| n.0 <= b.min.x) {
        gap(Vec3::new(n.0, cy, z), Vec3::new(b.min.x, cy, z), n, 0, 1);
    }
    if let Some(n) = right.filter(|n| n.0 >= b.max.x) {
        gap(Vec3::new(b.max.x, cy, z), Vec3::new(n.0, cy, z), n, 1, 0);
    }
    if let Some(n) = below.filter(|n| n.0 <= b.min.y) {
        gap(Vec3::new(cx, n.0, z), Vec3::new(cx, b.min.y, z), n, 3, 2);
    }
    if let Some(n) = above.filter(|n| n.0 >= b.max.y) {
        gap(Vec3::new(cx, b.max.y, z), Vec3::new(cx, n.0, z), n, 2, 3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Item, PlateFrame, Pose, Shape};

    fn scene() -> Scene {
        let mut s = Scene::new(PlateFrame {
            width: 400.0,
            height: 300.0,
        });
        // Plate 400 x 300 at x 0..400, y 0..300, 3 mm thick below the surface.
        s.push(Item::new(
            ItemId::new(ItemKind::Plate, 1),
            Shape::Box,
            Pose::at(Vec3::new(200.0, 150.0, -3.0)),
            Vec3::new(400.0, 300.0, 3.0),
        ));
        // Three 50 mm wide devices on one row, 100 mm tall, 60 mm deep.
        for (n, x) in [(10u64, 100.0f32), (11, 200.0), (12, 330.0)] {
            s.push(Item::new(
                ItemId::new(ItemKind::Device, n),
                Shape::Box,
                Pose::at(Vec3::new(x, 150.0, 0.0)),
                Vec3::new(50.0, 100.0, 60.0),
            ));
        }
        s
    }

    fn pick(s: &Scene, n: u64) -> BTreeSet<ItemId> {
        let _ = s;
        BTreeSet::from([ItemId::new(ItemKind::Device, n)])
    }

    fn find<'a>(d: &'a [Dimension], role: Role, text: &str) -> Option<&'a Dimension> {
        d.iter().find(|d| d.role == role && d.text == text)
    }

    #[test]
    fn millimetres_are_written_with_one_decimal_only_when_needed() {
        assert_eq!(format_mm(54.0), "54 mm");
        assert_eq!(format_mm(54.04), "54 mm");
        assert_eq!(format_mm(54.5), "54.5 mm");
        assert_eq!(format_mm(0.26), "0.3 mm");
        assert_eq!(format_mm(1234.96), "1235 mm");
    }

    #[test]
    fn the_overall_size_of_the_plate_is_always_there_without_a_depth() {
        let s = scene();
        let d = dimensions(&s, &BTreeSet::new());
        assert_eq!(d.len(), 2, "{d:?}");
        assert!(find(&d, Role::Overall, "W 400 mm").is_some(), "{d:?}");
        assert!(find(&d, Role::Overall, "H 300 mm").is_some(), "{d:?}");
        // The plate's third side is its thickness (3 mm), not a depth.
        assert!(d.iter().all(|d| !d.text.starts_with("D ")), "{d:?}");
    }

    #[test]
    fn the_shell_gives_the_overall_size_when_there_is_one() {
        let mut s = scene();
        s.push(Item::new(
            ItemId::new(ItemKind::Enclosure, 2),
            Shape::Box,
            Pose::at(Vec3::new(200.0, 150.0, -200.0)),
            Vec3::new(420.0, 320.0, 200.0),
        ));
        let d = dimensions(&s, &BTreeSet::new());
        assert!(find(&d, Role::Overall, "W 420 mm").is_some(), "{d:?}");
        assert!(find(&d, Role::Overall, "D 200 mm").is_some(), "{d:?}");
    }

    #[test]
    fn one_part_shows_its_size_and_its_distances_to_neighbours_and_the_plate() {
        let s = scene();
        // The middle device spans x 175..225, y 100..200, z 0..60.
        let d = dimensions(&s, &pick(&s, 11));
        assert!(find(&d, Role::Size, "W 50 mm").is_some(), "{d:?}");
        assert!(find(&d, Role::Size, "H 100 mm").is_some(), "{d:?}");
        assert!(find(&d, Role::Size, "D 60 mm").is_some(), "{d:?}");
        // Left neighbour ends at 125: 50 mm; right starts at 305: 80 mm.
        let left = find(&d, Role::Gap, "50 mm").expect("gap to the left neighbour");
        assert_eq!((left.a.x, left.b.x), (125.0, 175.0));
        let right = find(&d, Role::Gap, "80 mm").expect("gap to the right neighbour");
        assert_eq!((right.a.x, right.b.x), (225.0, 305.0));
        // Nothing above or below: the plate edge, 100 mm each way.
        let edges: Vec<_> = d.iter().filter(|d| d.role == Role::Edge).collect();
        assert_eq!(edges.len(), 2, "{d:?}");
        assert!(edges.iter().all(|e| e.text == "100 mm"), "{edges:?}");
    }

    #[test]
    fn a_part_next_to_the_plate_edge_measures_to_the_edge() {
        let s = scene();
        // The first device spans x 75..125: 75 mm from the plate's left edge.
        let d = dimensions(&s, &pick(&s, 10));
        let edge = find(&d, Role::Edge, "75 mm").expect("distance to the left edge");
        assert_eq!((edge.a.x, edge.b.x), (0.0, 75.0));
        // And its right neighbour is the middle device: 175 - 125 = 50 mm.
        assert!(find(&d, Role::Gap, "50 mm").is_some(), "{d:?}");
    }

    #[test]
    fn touching_parts_have_no_gap_line_and_overlap_is_not_a_gap() {
        let mut s = scene();
        s.push(Item::new(
            ItemId::new(ItemKind::Device, 13),
            Shape::Box,
            Pose::at(Vec3::new(250.0, 150.0, 0.0)),
            Vec3::new(50.0, 100.0, 60.0),
        ));
        // Device 13 spans 225..275 and touches the middle device at 225.
        let d = dimensions(&s, &pick(&s, 11));
        assert!(
            d.iter()
                .all(|d| d.role != Role::Gap || d.mm > 1.0 || d.a.x < 225.0),
            "{d:?}"
        );
        let right: Vec<_> = d
            .iter()
            .filter(|d| d.role == Role::Gap && d.a.x >= 225.0)
            .collect();
        assert!(right.is_empty(), "no line for touching parts: {right:?}");
    }

    #[test]
    fn several_selected_parts_or_a_plate_add_no_part_lines() {
        let s = scene();
        let two = BTreeSet::from([
            ItemId::new(ItemKind::Device, 10),
            ItemId::new(ItemKind::Device, 11),
        ]);
        assert_eq!(dimensions(&s, &two).len(), 2, "overall lines only");
        let plate = BTreeSet::from([ItemId::new(ItemKind::Plate, 1)]);
        assert_eq!(dimensions(&s, &plate).len(), 2);
        let missing = BTreeSet::from([ItemId::new(ItemKind::Device, 99)]);
        assert_eq!(dimensions(&s, &missing).len(), 2);
    }

    #[test]
    fn an_empty_scene_has_no_dimensions() {
        let s = Scene::new(PlateFrame {
            width: 10.0,
            height: 10.0,
        });
        assert!(dimensions(&s, &BTreeSet::new()).is_empty());
    }

    #[test]
    fn a_device_that_asks_for_space_gets_a_zone_grown_by_its_needs() {
        let mut s = scene();
        for it in &mut s.items {
            if it.id == ItemId::new(ItemKind::Device, 11) {
                it.needs = Some([10.0, 20.0, 30.0, 40.0]);
            }
        }
        let b = s.world_bounds(s.get(ItemId::new(ItemKind::Device, 11)).unwrap());
        let z = clearance_zones(&s, &pick(&s, 11));
        assert_eq!(z.len(), 1);
        assert_eq!(z[0][0], Vec3::new(b.min.x - 10.0, b.min.y - 40.0, b.min.z));
        assert_eq!(z[0][2], Vec3::new(b.max.x + 20.0, b.max.y + 30.0, b.min.z));
        // A part with no stated need, or one that is not selected, has none.
        assert!(clearance_zones(&s, &pick(&s, 12)).is_empty());
        assert!(clearance_zones(&s, &BTreeSet::new()).is_empty());
    }

    #[test]
    fn a_turned_device_asks_for_its_space_on_the_sides_of_the_plate() {
        let mut s = scene();
        for it in &mut s.items {
            if it.id == ItemId::new(ItemKind::Device, 11) {
                // R: a quarter turn clockwise seen from the door. The part is
                // 50 x 100 upright, so 100 wide and 50 tall on the plate.
                it.pose.rotation = glam::Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2);
                // Plate sides, as the build fills them in.
                it.needs = Some([10.0, 20.0, 30.0, 40.0]);
            }
        }
        let b = s.world_bounds(s.get(ItemId::new(ItemKind::Device, 11)).unwrap());
        assert!(
            (b.max.x - b.min.x - 100.0).abs() < 0.01 && (b.max.y - b.min.y - 50.0).abs() < 0.01
        );
        let z = clearance_zones(&s, &pick(&s, 11));
        assert_eq!(z.len(), 1);
        // Left 10 and right 20 grow the wide side, above 30 and below 40 the tall one:
        // the needs are not turned a second time.
        assert!((z[0][0].x - (b.min.x - 10.0)).abs() < 0.01);
        assert!((z[0][0].y - (b.min.y - 40.0)).abs() < 0.01);
        assert!((z[0][2].x - (b.max.x + 20.0)).abs() < 0.01);
        assert!((z[0][2].y - (b.max.y + 30.0)).abs() < 0.01);
    }

    #[test]
    fn gaps_between_devices_say_whether_they_keep_the_asked_clearance() {
        let mut s = scene();
        for it in &mut s.items {
            if it.id.kind == ItemKind::Device {
                // 60 mm asked on the left and right of every device.
                it.needs = Some([60.0, 60.0, 0.0, 0.0]);
            }
        }
        let d = dimensions(&s, &pick(&s, 11));
        // Left neighbour 50 mm away: too close. Right neighbour 80 mm: fine.
        let left = find(&d, Role::Gap, "50 mm (min 60 mm)").expect("left gap");
        assert_eq!(left.clearance_ok(), Some(false));
        let right = find(&d, Role::Gap, "80 mm (min 60 mm)").expect("right gap");
        assert_eq!(right.clearance_ok(), Some(true));
        // Edges and gaps to parts that state no need carry no verdict.
        assert!(
            d.iter()
                .filter(|m| m.role != Role::Gap)
                .all(|m| m.clearance_ok().is_none())
        );
    }
}

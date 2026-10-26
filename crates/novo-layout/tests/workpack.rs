//! The workshop pack on a hand-built cabinet: every step gets a card, the
//! hole positions agree with the drill table, wires are chunked and every
//! wire is on exactly one wire card.

use novo_layout::assembly::Phase;
use novo_layout::drill::{Cutout, CutoutShape, DeviceFeatures, Hole, Surface, drill_table};
use novo_layout::state::CabinetLayout;
use novo_layout::workpack::{
    CardKind, OpeningJob, OpeningWall, WIRES_PER_CARD, WireCut, WireGroup, add_opening_cards,
    build, from_edges,
};
use novo_layout::{Duct, Margins, Placement, Plate, Rail, Size};

fn place(id: u64, x: f64, y: f64, rotated: bool, rail: Option<u64>) -> Placement {
    Placement {
        id,
        x,
        y,
        rotated,
        size: Size {
            width: 45.0,
            height: 90.0,
            guessed: false,
        },
        clearance: None,
        margins: Margins::default(),
        rail,
    }
}

fn layout() -> CabinetLayout {
    let mut l = CabinetLayout::new(Plate {
        width: 800.0,
        height: 600.0,
        margin: 10.0,
    });
    l.rails = vec![
        Rail {
            id: 2,
            x: 50.0,
            y: 300.0,
            length: 400.0,
            vertical: false,
        },
        Rail {
            id: 1,
            x: 50.0,
            y: 100.0,
            length: 400.0,
            vertical: false,
        },
    ];
    l.ducts = vec![Duct {
        id: 3,
        x: 50.0,
        y: 250.0,
        length: 700.0,
        width: 50.0,
        vertical: false,
    }];
    l.placements = vec![
        place(12, 80.0, 400.0, false, None),
        place(14, 300.0, 400.0, true, None),
        place(11, 200.0, 100.0, false, Some(1)),
        place(10, 100.0, 100.0, false, Some(1)),
    ];
    l
}

fn features() -> Vec<DeviceFeatures> {
    let holes = vec![
        Hole {
            dx: 5.0,
            dy: 5.0,
            dia: 4.5,
        },
        Hole {
            dx: 40.0,
            dy: 85.0,
            dia: 4.5,
        },
    ];
    let f = |id: u64, label: &str, cutout: Option<Cutout>| DeviceFeatures {
        id,
        label: label.into(),
        surface: Surface::Plate,
        holes: holes.clone(),
        cutout,
        problems: Vec::new(),
    };
    vec![
        f(12, "-Q12", None),
        f(
            14,
            "-Q14",
            Some(Cutout {
                shape: CutoutShape::Rect { w: 20.0, h: 10.0 },
                centre: Some((10.0, 20.0)),
            }),
        ),
    ]
}

fn wire(n: &str, from: &str, to: &str, cut: f64) -> WireCut {
    WireCut {
        number: n.into(),
        from: from.into(),
        to: to.into(),
        kind: "1.5 mm2 black".into(),
        cut_mm: cut,
        estimated: false,
        end_mm: Some(10.0),
        fitting: "ferrule",
        part: None,
        strip_from_mm: Some(10.0),
        strip_to_mm: Some(10.0),
        strip_basis: "typical",
    }
}

fn groups(count: usize) -> Vec<WireGroup> {
    vec![
        WireGroup {
            title: "Power".into(),
            wires: (0..count)
                .map(|i| wire(&format!("W{i}"), "-Q10:1", "-Q11:2", 300.0 - i as f64))
                .collect(),
        },
        WireGroup {
            title: "Control".into(),
            wires: vec![wire("C1", "-Q12:A1", "-Q10:3", 150.0)],
        },
    ]
}

fn pack(count: usize) -> novo_layout::workpack::Pack {
    build(
        "+CAB1",
        &layout(),
        &features(),
        &|id| format!("-Q{id}"),
        &groups(count),
    )
}

#[test]
fn every_rail_duct_and_device_step_has_its_own_card() {
    let p = pack(3);
    let kinds: Vec<(CardKind, String)> = p.cards.iter().map(|c| (c.kind, c.mark.clone())).collect();
    assert_eq!(
        kinds[..9],
        [
            (CardKind::Rail, "R1".into()),
            (CardKind::Rail, "R2".into()),
            (CardKind::Duct, "D1".into()),
            (CardKind::Clip, "-Q10".into()),
            (CardKind::Clip, "-Q11".into()),
            (CardKind::Screw, "-Q12".into()),
            (CardKind::Screw, "-Q14".into()),
            (CardKind::Wires, "Power".into()),
            (CardKind::Wires, "Control".into()),
        ]
    );
    assert_eq!(p.cards.last().unwrap().kind, CardKind::Labels);
    // The overview is the build order, step for step.
    assert_eq!(p.overview.len(), 3 + 4 + 4 + 4);
    assert!(p.overview.iter().enumerate().all(|(i, r)| r.step == i + 1));
}

#[test]
fn card_hole_numbers_and_positions_agree_with_the_drill_table() {
    let l = layout();
    let table = drill_table(&l, &features());
    let p = pack(1);
    for tag in ["-Q12", "-Q14"] {
        let card = p.cards.iter().find(|c| c.mark == tag).unwrap();
        let t = card.template.as_ref().unwrap();
        let rows: Vec<_> = table.holes.iter().filter(|h| h.source == tag).collect();
        assert_eq!(t.holes.len(), rows.len(), "{tag}");
        let placement = l
            .placements
            .iter()
            .find(|q| format!("-Q{}", q.id) == tag)
            .unwrap();
        let fp = placement.footprint();
        for h in &t.holes {
            let row = rows
                .iter()
                .find(|r| r.no == h.no)
                .unwrap_or_else(|| panic!("{tag}: no. {} is not in the table", h.no));
            assert!((fp.x + h.x - row.x).abs() < 0.011, "{tag} x");
            assert!((fp.y + h.y - row.y).abs() < 0.011, "{tag} y");
        }
    }
    // Rail holes carry the table's numbers too.
    let rail = p.cards.iter().find(|c| c.mark == "R1").unwrap();
    let nos: Vec<usize> = rail
        .template
        .as_ref()
        .unwrap()
        .holes
        .iter()
        .map(|h| h.no)
        .collect();
    assert!(nos.iter().all(|n| *n > 0), "{nos:?}");
}

#[test]
fn a_turned_device_turns_its_template_with_it() {
    let p = pack(1);
    let t = p
        .cards
        .iter()
        .find(|c| c.mark == "-Q14")
        .unwrap()
        .template
        .as_ref()
        .unwrap();
    assert_eq!((t.width, t.height), (90.0, 45.0), "footprint is swapped");
    // Hole (5,5) of the 45 x 90 part: turned 90 degrees clockwise it lands
    // at (90 - 5, 5) in the 90 x 45 footprint.
    assert!(
        t.holes.iter().any(|h| h.x == 85.0 && h.y == 5.0),
        "{:?}",
        t.holes
    );
    let cut = t.cutout.as_ref().unwrap();
    assert_eq!(cut.shape, CutoutShape::Rect { w: 10.0, h: 20.0 });
    assert_eq!((cut.x, cut.y), (70.0, 10.0));
}

#[test]
fn the_rail_strip_has_the_rail_holes_from_its_start() {
    let p = pack(1);
    let c = p.cards.iter().find(|c| c.mark == "R1").unwrap();
    let t = c.template.as_ref().unwrap();
    assert!(t.rail);
    assert_eq!((t.width, t.height), (400.0, 35.0));
    let xs: Vec<f64> = t.holes.iter().map(|h| h.x).collect();
    assert_eq!(xs.first(), Some(&10.0));
    assert_eq!(xs.last(), Some(&390.0));
    assert!(c.tools[0].contains("400"), "{:?}", c.tools);
    // One rule walked along the rail: positions, folded pitch, last hole to end.
    assert_eq!(
        c.dims,
        [
            "Holes, mm from the rail start: 10, 200, 390 (all on the centre line)",
            "Pitch between holes, mm: 190 x 2",
            "Last hole to the rail end: 10 mm",
        ]
    );
}

#[test]
fn a_clipped_device_without_holes_says_nothing_is_drilled() {
    let p = pack(1);
    let c = p.cards.iter().find(|c| c.mark == "-Q10").unwrap();
    assert!(c.template.is_none());
    assert!(c.notes.iter().any(|n| n.contains("Nothing to drill")));
    assert_eq!(c.ticks, ["Fitted", "Checked"]);
    // Its wires are listed from the cut list, two ends counted.
    assert!(c.wires.iter().any(|w| w.number == "C1"));
}

#[test]
fn wires_are_chunked_and_every_wire_is_on_exactly_one_wire_card() {
    let count = WIRES_PER_CARD * 2 + 3;
    let p = pack(count);
    let wire_cards: Vec<_> = p
        .cards
        .iter()
        .filter(|c| c.kind == CardKind::Wires)
        .collect();
    assert_eq!(wire_cards.len(), 3 + 1);
    let listed: usize = wire_cards.iter().map(|c| c.wires.len()).sum();
    assert_eq!(listed, count + 1);
    assert!(wire_cards.iter().all(|c| c.wires.len() <= WIRES_PER_CARD));
    // Step ranges follow the build order and cover every wire step once.
    let mut covered = Vec::new();
    for c in &wire_cards {
        covered.extend(c.steps.0..=c.steps.1);
    }
    let want: Vec<usize> = p
        .overview
        .iter()
        .filter(|r| r.phase == Phase::Wires)
        .map(|r| r.step)
        .collect();
    assert_eq!(covered, want);
}

#[test]
fn distances_are_given_from_the_nearest_edge() {
    assert_eq!(
        from_edges(100.0, 50.0, (800.0, 600.0)),
        "100 mm from the left edge, 50 mm from the top edge"
    );
    assert_eq!(
        from_edges(790.0, 590.5, (800.0, 600.0)),
        "10 mm from the right edge, 9.5 mm from the bottom edge"
    );
}

#[test]
fn the_tool_list_counts_drills_and_cuts() {
    let p = pack(2);
    let drill: Vec<_> = p
        .tools
        .iter()
        .filter(|t| t.tool.starts_with("Drill"))
        .collect();
    assert!(
        drill
            .iter()
            .any(|t| t.tool == "Drill dia 4.5 mm" && t.for_what.starts_with("4 holes")),
        "{:?}",
        p.tools
    );
    assert!(p.tools.iter().any(|t| t.tool == "Drill dia 5.5 mm"));
    assert!(
        p.tools
            .iter()
            .any(|t| t.tool.starts_with("Punch, nibbler or jigsaw"))
    );
    assert!(
        p.tools
            .iter()
            .any(|t| t.for_what.contains("DIN rail 400 mm: R1, R2"))
    );
    assert!(
        p.tools
            .iter()
            .any(|t| t.for_what.contains("wiring duct 700 mm: D1"))
    );
    assert!(
        p.tools
            .iter()
            .any(|t| t.for_what.starts_with("3 wires in 2 groups"))
    );
}

#[test]
fn the_pack_is_deterministic() {
    assert_eq!(pack(5), pack(5));
}

#[test]
fn an_empty_cabinet_gives_an_empty_pack() {
    let l = CabinetLayout::new(Plate {
        width: 400.0,
        height: 300.0,
        margin: 10.0,
    });
    let p = build("+CAB2", &l, &[], &|id| format!("-Q{id}"), &[]);
    assert!(p.cards.is_empty());
    assert!(p.overview.is_empty());
}

fn grille(wall: OpeningWall, along: f64, down: Option<f64>) -> OpeningJob {
    OpeningJob {
        name: "Grille".into(),
        what: "grille".into(),
        wall,
        along,
        down,
        width: 200.0,
        height: 100.0,
        size: (600.0, 800.0),
        depth: 250.0,
    }
}

#[test]
fn a_door_grille_is_a_card_outside_the_build_order_with_edge_distances() {
    let mut p = pack(3);
    let before = (p.cards.len(), p.tools.len());
    add_opening_cards(&mut p, &[grille(OpeningWall::Door, 300.0, Some(150.0))]);
    assert_eq!(p.cards.len(), before.0 + 1);
    let c = p.cards.first().unwrap();
    assert_eq!(c.kind, CardKind::Opening);
    assert_eq!(c.steps, (0, 0));
    assert!(c.mark.contains("door"), "{}", c.mark);
    // Centre at x 300 of 600, y 150 of 800: the nearest edges are named.
    assert!(
        c.dims[0].contains(&from_edges(300.0, 150.0, (600.0, 800.0))),
        "{:?}",
        c.dims
    );
    // Edges: 300 +- 100 from the left, 150 +- 50 from the top.
    assert!(c.dims[1].contains("200") && c.dims[1].contains("400"));
    assert!(c.dims[1].contains("100") && c.dims[1].contains("200"));
    assert!(c.notes.iter().any(|n| n.contains("flange")));
    // The template is the cut-out plus a frame, centre cross hair.
    let t = c.template.as_ref().unwrap();
    assert_eq!((t.width, t.height), (220.0, 120.0));
    let cut = t.cutout.as_ref().unwrap();
    assert_eq!((cut.x, cut.y), (110.0, 60.0));
    assert_eq!(p.tools.len(), before.1 + 1);
    let punch = p
        .tools
        .iter()
        .position(|t| t.for_what.contains("grille cut-out"));
    let every = p.tools.iter().position(|t| t.for_what == "every step");
    assert!(punch.is_some() && punch < every, "{:?}", p.tools);
}

#[test]
fn a_side_wall_opening_is_drawn_with_its_width_along_the_wall() {
    let mut p = pack(0);
    add_opening_cards(&mut p, &[grille(OpeningWall::Left, 400.0, None)]);
    let c = p.cards.first().unwrap();
    let t = c.template.as_ref().unwrap();
    // 200 mm along y (drawn vertical), 100 mm deep (drawn horizontal).
    assert_eq!((t.width, t.height), (120.0, 220.0));
    assert!(c.dims[0].contains("400"));
    assert!(c.notes.iter().any(|n| n.contains("depth")));
    let top = grille(OpeningWall::Top, 100.0, None);
    add_opening_cards(&mut p, &[top]);
    assert!(p.cards[1].dims[0].contains("100"));
}

#[test]
fn no_openings_change_nothing_and_the_pack_stays_deterministic() {
    let mut a = pack(5);
    let b = pack(5);
    add_opening_cards(&mut a, &[]);
    assert_eq!(a, b);
    let jobs = [grille(OpeningWall::Door, 300.0, Some(150.0))];
    let (mut x, mut y) = (pack(5), pack(5));
    add_opening_cards(&mut x, &jobs);
    add_opening_cards(&mut y, &jobs);
    assert_eq!(x, y);
}

#[test]
fn identical_openings_share_one_counted_tool_line_and_fans_point_to_the_datasheet() {
    let mut p = pack(2);
    let before = p.tools.len();
    let mut fan = grille(OpeningWall::Door, 100.0, Some(100.0));
    fan.what = "fan".into();
    add_opening_cards(
        &mut p,
        &[
            grille(OpeningWall::Right, 100.0, None),
            grille(OpeningWall::Right, 300.0, None),
            grille(OpeningWall::Right, 500.0, None),
            fan,
        ],
    );
    // Three equal grilles are one line, the fan another.
    assert_eq!(p.tools.len(), before + 2);
    assert!(
        p.tools.iter().any(|t| t.for_what.starts_with("3 x grille")),
        "{:?}",
        p.tools
    );
    let cards: Vec<_> = p
        .cards
        .iter()
        .filter(|c| c.kind == CardKind::Opening)
        .collect();
    assert_eq!(cards.len(), 4);
    assert!(cards[3].notes.iter().any(|n| n.contains("datasheet")));
    assert!(!cards[0].notes.iter().any(|n| n.contains("datasheet")));
}

#[test]
fn the_end_text_names_the_allowance_with_unit_and_the_fitting() {
    let mut w = wire("W1", "-Q1:1", "-Q2:2", 200.0);
    assert_eq!(w.end_text(), "10 mm ferrule");
    w.end_mm = Some(20.0);
    w.fitting = "lug";
    assert_eq!(w.end_text(), "20 mm lug");
    // Both fittings take the same allowance: the word would be a guess.
    w.fitting = "";
    assert_eq!(w.end_text(), "20 mm");
    // No allowance in the cut list, or none at all: nothing, never "0 ferrule".
    w.end_mm = None;
    assert_eq!(w.end_text(), "");
    w.end_mm = Some(0.0);
    assert_eq!(w.end_text(), "");
}

fn with_openings(list: Vec<novo_layout::openings::Opening>) -> CabinetLayout {
    let mut l = layout();
    l.openings = list;
    l.surface_sizes.insert("door".into(), (600.0, 800.0));
    l.surface_sizes.insert("left".into(), (250.0, 800.0));
    l
}

fn opening(
    name: &str,
    surface: &str,
    shape: novo_layout::openings::OpeningShape,
    x: f64,
    y: f64,
) -> novo_layout::openings::Opening {
    novo_layout::openings::Opening {
        name: name.into(),
        surface: surface.into(),
        shape,
        x,
        y,
        keep: 5.0,
        exit: None,
    }
}

#[test]
fn wiring_openings_get_a_card_each_with_edge_distances_outside_the_build_order() {
    use novo_layout::openings::{Exit, OpeningShape};
    use novo_layout::workpack::add_wiring_opening_cards;
    let mut pass = opening(
        "entry",
        "door",
        OpeningShape::Rect { w: 80.0, h: 40.0 },
        300.0,
        100.0,
    );
    pass.exit = Some(Exit {
        surface: "left".into(),
        x: 125.0,
        y: 100.0,
    });
    let l = with_openings(vec![
        opening(
            "grommet",
            "plate",
            OpeningShape::Round { dia: 30.0 },
            400.0,
            300.0,
        ),
        pass,
        opening(
            "lost",
            "roof",
            OpeningShape::Round { dia: 20.0 },
            10.0,
            10.0,
        ),
    ]);
    let mut p = pack(2);
    let before = p.cards.len();
    let overview = p.overview.len();
    add_wiring_opening_cards(&mut p, &l);
    // The roof is not an enclosure surface here: named, not carded.
    assert_eq!(p.cards.len(), before + 2);
    assert_eq!(p.overview.len(), overview);
    assert!(
        p.problems
            .iter()
            .any(|m| m.contains("'lost'") && m.contains("roof"))
    );
    let cards: Vec<_> = p
        .cards
        .iter()
        .filter(|c| c.kind == CardKind::Opening)
        .collect();
    assert_eq!(cards[0].steps, (0, 0));
    assert!(cards[0].mark.contains("grommet") && cards[0].mark.contains("mounting plate"));
    assert!(cards[0].dims[0].contains(&from_edges(400.0, 300.0, (800.0, 600.0))));
    assert!(cards[0].dims[1].contains("385") && cards[0].dims[1].contains("415"));
    assert!(cards[0].tools[0].contains("dia 30"));
    assert!(cards[1].tools[0].contains("80 x 40"));
    assert!(
        cards[1]
            .notes
            .iter()
            .any(|n| n.contains("left") && n.contains("cut both ends"))
    );
    let t = cards[0].template.as_ref().unwrap();
    assert_eq!((t.width, t.height), (50.0, 50.0));
}

#[test]
fn no_wiring_openings_change_nothing() {
    use novo_layout::workpack::add_wiring_opening_cards;
    let mut a = pack(3);
    add_wiring_opening_cards(&mut a, &with_openings(Vec::new()));
    assert_eq!(a, pack(3));
}

#[test]
fn a_wall_opening_is_measured_from_the_back_wall_and_says_which_side_is_seen() {
    use novo_layout::openings::{Exit, OpeningShape};
    use novo_layout::workpack::add_wiring_opening_cards;
    let rect = OpeningShape::Rect { w: 40.0, h: 20.0 };
    let mut l = with_openings(vec![
        opening("left hole", "left", rect, 100.0, 300.0),
        opening("right hole", "right", rect, 100.0, 300.0),
        opening("roof hole", "roof", rect, 400.0, 120.0),
        opening("door hole", "door", rect, 100.0, 300.0),
    ]);
    l.surface_sizes.insert("right".into(), (250.0, 800.0));
    l.surface_sizes.insert("roof".into(), (600.0, 250.0));
    l.openings[0].exit = Some(Exit {
        surface: "right".into(),
        x: 60.0,
        y: 300.0,
    });
    let mut p = pack(0);
    add_wiring_opening_cards(&mut p, &l);
    let card = |name: &str| {
        p.cards
            .iter()
            .find(|c| c.kind == CardKind::Opening && c.mark.contains(name))
            .unwrap()
    };
    let left = card("left hole");
    assert_eq!(
        left.dims[0],
        "Centre: 100 mm from the back wall, 300 mm from the top edge"
    );
    assert!(
        left.dims[1].contains("80 to 120 mm from the back wall"),
        "{:?}",
        left.dims
    );
    assert!(left.notes.iter().any(|n| n.contains("Seen from outside")
        && n.contains("back wall is on the left (from inside it is on the right)")));
    // The pass-through end is on the right wall, measured from its back wall too.
    assert!(
        left.notes
            .iter()
            .any(|n| n.contains("60 mm from the back wall") && n.contains("seen from inside")),
        "{:?}",
        left.notes
    );
    assert!(
        card("right hole").notes[0]
            .contains("Seen from inside the cabinet: the back wall is on the left (from outside it is on the right)")
    );
    // The roof runs front to back in y.
    assert!(card("roof hole").dims[0].contains("120 mm from the back wall"));
    assert!(card("roof hole").notes[0].contains("from above"));
    // The door keeps nearest edges and says it is seen from outside.
    assert!(card("door hole").dims[0].contains("from the left edge"));
    assert!(card("door hole").notes[0].contains("from outside"));
}

#[test]
fn a_rail_card_draws_the_clipped_devices_along_the_strip() {
    let p = pack(0);
    let rail = |mark_has: &str| {
        p.cards
            .iter()
            .find(|c| c.kind == CardKind::Rail && c.mark.contains(mark_has))
            .unwrap()
    };
    // R1 starts at x 50 and holds -Q10 (x 100) and -Q11 (x 200), 45 wide.
    let t = rail("R1").template.as_ref().unwrap();
    let clips: Vec<(&str, f64, f64)> = t
        .clips
        .iter()
        .map(|c| (c.tag.as_str(), c.at, c.len))
        .collect();
    assert_eq!(clips, vec![("-Q10", 50.0, 45.0), ("-Q11", 150.0, 45.0)]);
    assert_eq!(t.clips[0].short(), "Q10");
    // Written once for the person without the drawing, nearest first.
    assert!(
        rail("R1")
            .notes
            .iter()
            .any(|n| n.contains("-Q10 at 50, -Q11 at 150"))
    );
    // R2 holds no device: nothing drawn, no note.
    assert!(rail("R2").template.as_ref().unwrap().clips.is_empty());
    assert!(!rail("R2").notes.iter().any(|n| n.contains("Clipped")));
    // Screwed devices are never on a strip.
    assert!(!t.clips.iter().any(|c| c.tag == "-Q12"));
}

#[test]
fn many_clipped_devices_are_all_drawn_and_a_few_named() {
    let mut l = layout();
    l.rails = vec![Rail {
        id: 1,
        x: 0.0,
        y: 100.0,
        length: 700.0,
        vertical: false,
    }];
    l.placements = (0..9)
        .map(|i| place(i + 1, 10.0 + 70.0 * i as f64, 100.0, false, Some(1)))
        .collect();
    let p = build("+CAB1", &l, &[], &|id| format!("-Q{id}"), &[]);
    let c = &p.cards[0];
    assert_eq!(c.template.as_ref().unwrap().clips.len(), 9);
    assert!(
        c.notes.iter().any(|n| n.contains("and 3 more")),
        "{:?}",
        c.notes
    );
}

#[test]
fn a_vertical_rail_measures_its_devices_down_the_rail() {
    let mut l = layout();
    l.rails = vec![Rail {
        id: 1,
        x: 60.0,
        y: 100.0,
        length: 400.0,
        vertical: true,
    }];
    l.placements = vec![place(10, 40.0, 150.0, false, Some(1))];
    let p = build("+CAB1", &l, &[], &|id| format!("-Q{id}"), &[]);
    let t = p.cards[0].template.as_ref().unwrap();
    // Footprint 45 x 90: 90 along the rail, 50 from its start (y 150 - 100).
    assert_eq!((t.clips[0].at, t.clips[0].len), (50.0, 90.0));
}

#[test]
fn a_clip_card_says_which_end_of_its_rail_is_the_start() {
    let mut l = layout();
    l.rails = vec![
        Rail {
            id: 1,
            x: 50.0,
            y: 100.0,
            length: 400.0,
            vertical: false,
        },
        Rail {
            id: 2,
            x: 600.0,
            y: 100.0,
            length: 300.0,
            vertical: true,
        },
    ];
    l.placements = vec![
        place(10, 80.0, 100.0, false, Some(1)),
        place(11, 600.0, 150.0, false, Some(2)),
    ];
    let p = build("+CAB1", &l, &[], &|id| format!("-Q{id}"), &[]);
    let note = |tag: &str| {
        p.cards
            .iter()
            .find(|c| c.kind == CardKind::Clip && c.mark.contains(tag))
            .unwrap()
            .notes
            .join(" | ")
    };
    assert!(
        note("Q10").contains("R1 starts at its left end, seen from the front"),
        "{}",
        note("Q10")
    );
    assert!(
        note("Q11").contains("R2 starts at its top end"),
        "{}",
        note("Q11")
    );
}

#[test]
fn a_ducts_step_text_and_card_give_the_same_corner() {
    let p = pack(0);
    let card = p.cards.iter().find(|c| c.kind == CardKind::Duct).unwrap();
    // The duct's centre line is at y 250, 50 wide: its outline starts at y 225.
    assert!(
        card.text.contains("top left corner at x 50, y 225"),
        "{}",
        card.text
    );
    assert!(
        card.dims[0].contains("225 mm from the top edge"),
        "{:?}",
        card.dims
    );
    let row = p.overview.iter().find(|r| r.mark == card.mark).unwrap();
    assert_eq!(row.text.split(" on the").next(), Some(card.text.as_str()));
}

#[test]
fn the_part_text_tells_wires_sharing_a_number_apart() {
    let mut w = wire("4", "-F2:2", "-X1:1", 796.0);
    assert_eq!(w.part_text(), "");
    w.part = Some((2, 3));
    assert_eq!(w.part_text(), "2 of 3");
}

#[test]
fn the_strip_text_names_the_length_its_source_and_both_ends_when_they_differ() {
    let mut w = wire("W1", "-Q1:1", "-Q2:2", 200.0);
    assert_eq!(w.strip_text(), "strip 10 mm typ.");
    w.strip_basis = "part";
    assert_eq!(w.strip_text(), "strip 10 mm part");
    w.strip_basis = "stated";
    w.strip_from_mm = Some(8.0);
    w.strip_to_mm = Some(12.5);
    assert_eq!(w.strip_text(), "strip 8/12.5 mm stated");
    w.strip_basis = "typical";
    assert_eq!(w.strip_text(), "strip 8/12.5 mm typ.");
    // One end unknown reads as unknown, not as the same at both.
    w.strip_to_mm = None;
    assert_eq!(w.strip_text(), "strip 8/? mm typ.");
    w.strip_to_mm = Some(12.5);
    w.strip_basis = "stated";
    // A lug counts by its barrel, no strip text beside it.
    w.fitting = "lug";
    assert_eq!(w.strip_text(), "");
    w.fitting = "ferrule";
    w.strip_from_mm = None;
    w.strip_to_mm = None;
    w.strip_basis = "";
    assert_eq!(w.strip_text(), "");
}

#[test]
fn duct_fixings_follow_the_planning_rule() {
    use novo_layout::workpack::{DUCT_FIXING_END_MM, DUCT_FIXING_PITCH_MM, duct_fixings};
    for len in [30.0, 100.0, 250.0, 400.0, 1000.0, 1999.0] {
        let at = duct_fixings(len);
        assert!(!at.is_empty());
        if len > 2.0 * DUCT_FIXING_END_MM {
            assert_eq!(at[0], DUCT_FIXING_END_MM);
            assert!((at[at.len() - 1] - (len - DUCT_FIXING_END_MM)).abs() < 1e-9);
        }
        assert!(
            at.windows(2)
                .all(|w| w[1] - w[0] <= DUCT_FIXING_PITCH_MM + 1e-9),
            "{len}: {at:?}"
        );
    }
    assert_eq!(duct_fixings(400.0), [25.0, 200.0, 375.0]);
}

#[test]
fn the_duct_note_says_it_is_a_planning_rule_and_where_the_fixing_goes() {
    use novo_layout::workpack::duct_fixing_note;
    for len in [100.0, 530.0, 2000.0] {
        let n = duct_fixing_note(len);
        for want in [
            "planning rule, not a datasheet value",
            "centre line of the duct base",
            "not in the drilling list",
        ] {
            assert!(n.contains(want), "{len}: {n}");
        }
    }
    // A 2 m duct: 11 fixings, each named.
    assert!(duct_fixing_note(2000.0).contains("11 rivets"));
}

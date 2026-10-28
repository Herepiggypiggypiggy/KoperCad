//! What the heat and sizing lessons say about their sample projects, checked against what the
//! engines give: the quoted losses and totals, which device is the hottest place in the preview,
//! the sizing numbers, and the cooling lessons' "one device runs hotter than it allows". The
//! lesson text is the claim; these tests are the reviewer who reads it against the code. A lesson
//! that quotes a figure from an engine adds its claim here (and `qa/scenarios/530-learn-heat-claims.qa`
//! walks the same lesson in the window at both sizes).

use novo_core::{Device, DeviceId, Project, Structure, StructureId};
use novo_layout::sizing::{EnclosureCatalog, EnclosureSize, Installation, Material, SizingRequest};
use novo_thermal::Status;
use novo_thermal::bridge::{BridgeOptions, scenario_from_project};

use super::heat::{Input, drawn_peak_tags, drawn_temps, items_of};

const WIRE_ROUTING: &str = include_str!("../../../../assets/lessons/wire-routing/project.json");
const PLC_FINISHED: &str = include_str!("../../../../assets/lessons/plc-panel/finished.json");
const COOLING: &str = include_str!("../../../../assets/lessons/cooling-what-if/project.json");
const COOLER_LAYOUT: &str = include_str!("../../../../assets/lessons/cooler-layout/project.json");

fn open(json: &str) -> (Project, StructureId) {
    let project = Project::from_json(json).unwrap();
    let cab = project
        .structures()
        .find(|s: &&Structure| s.enclosure)
        .expect("the sample project has a cabinet")
        .id;
    (project, cab)
}

/// The preview input of a sample project, the way the Cabinet workspace builds it.
fn preview(project: &Project, cab: StructureId) -> Input {
    let layout = novo_layout::model::load(project, cab).unwrap();
    let tag = |id: u64| {
        project
            .get::<Device>(DeviceId::from(id))
            .map(|d| d.tag.to_string())
            .unwrap_or_default()
    };
    Input {
        plate: (layout.plate.width, layout.plate.height),
        ambient: 35.0,
        items: items_of(project, &layout, &tag),
        solution: None,
        kept: None,
    }
}

fn loss_of(input: &Input, tag_end: &str) -> f64 {
    input
        .items
        .iter()
        .find(|i| i.tag.ends_with(tag_end))
        .unwrap_or_else(|| panic!("no device {tag_end}"))
        .loss_w
}

/// The tag of the device with the highest preview reading.
fn hottest_in_preview(input: &Input) -> String {
    let temps = drawn_temps(input);
    let (i, _) = temps
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap();
    input.items[i].tag.clone()
}

#[test]
fn heat_and_sizing_quotes_the_losses_and_the_hottest_place_the_preview_shows() {
    let (project, cab) = open(WIRE_ROUTING);
    let input = preview(&project, cab);
    // "-T1 20 W, -F1 6 W, -F2 4 W, -K1 3 W, -F3 1.5 W. Together that is 34.5 W."
    for (tag, w) in [
        ("-T1", 20.0),
        ("-F1", 6.0),
        ("-F2", 4.0),
        ("-K1", 3.0),
        ("-F3", 1.5),
    ] {
        assert_eq!(loss_of(&input, tag), w, "{tag}");
    }
    let total: f64 = input.items.iter().map(|i| i.loss_w).sum();
    assert_eq!(total, 34.5);
    // The legend: "Total loss 34 W".
    assert_eq!(format!("{total:.0}"), "34");
    // "The hottest place sits on the transformer -T1, the biggest source": the badge and the
    // first hot-spot card.
    assert!(hottest_in_preview(&input).ends_with("-T1"));
    assert!(drawn_peak_tags(&input)[0].ends_with("-T1"));
    // The badge example "-T1  78 °C": the preview's reading, within the rounding of its fit.
    let temps = drawn_temps(&input);
    let t1 = input
        .items
        .iter()
        .position(|i| i.tag.ends_with("-T1"))
        .unwrap();
    assert!(
        (temps[t1] - 78.0).abs() < 2.0,
        "T1 reads {:.1} in the preview",
        temps[t1]
    );
}

#[test]
fn heat_and_sizing_quotes_the_sizing_window() {
    let (project, cab) = open(WIRE_ROUTING);
    let devices = novo_layout::sizing_model::sizing_devices(&project, cab);
    // What "Add" in the enclosure list makes.
    let mut catalog = EnclosureCatalog::new();
    catalog.items.push(EnclosureSize {
        name: "Enclosure 1".into(),
        width: 600.0,
        height: 800.0,
        depth: 300.0,
        plate_width: 550.0,
        plate_height: 750.0,
        margin: 10.0,
        material: Material::Steel,
    });
    let at = |ambient_max| SizingRequest {
        free_space: 0.2,
        duct_allowance: 0.2,
        installation: Installation::FreeStanding,
        ambient_max,
        inside_max: 40.0,
        ..SizingRequest::default()
    };
    let rec = novo_layout::sizing::recommend(&devices, &catalog, &at(35.0));
    let t = rec.thermal.expect("a recommendation");
    assert_eq!(format!("{:.0}", rec.loss), "34");
    // "plate fill 10 %, loss 34 W, inside at the highest ambient 39 C, Natural cooling is enough".
    assert_eq!(format!("{:.0}", rec.fill.unwrap() * 100.0), "10");
    assert_eq!(format!("{:.0}", t.inside_temperature), "39");
    assert!(t.natural_ok);
    // "Ambient max 45 C: 49 C, above the 40 C limit, Needs about 34 W of cooling."
    let hot = novo_layout::sizing::recommend(&devices, &catalog, &at(45.0));
    let t = hot.thermal.expect("a recommendation");
    assert_eq!(format!("{:.0}", t.inside_temperature), "49");
    assert!(!t.natural_ok);
    assert_eq!(format!("{:.0}", t.cooling_needed), "34");
}

#[test]
fn the_plc_panel_lesson_names_the_drive_as_the_hot_spot() {
    let (project, cab) = open(PLC_FINISHED);
    let input = preview(&project, cab);
    // "about 118 W in all, the drive 60 W, the supply 18 W, the PLC 14 W".
    let total: f64 = input.items.iter().map(|i| i.loss_w).sum();
    assert!((total - 118.0).abs() < 1.5, "{total}");
    assert_eq!(input.items.len(), 15);
    let drive = input
        .items
        .iter()
        .max_by(|a, b| a.loss_w.total_cmp(&b.loss_w))
        .unwrap();
    assert_eq!(drive.loss_w, 60.0);
    // "The hot spot sits on the drive."
    assert_eq!(hottest_in_preview(&input), drive.tag);
    assert_eq!(drawn_peak_tags(&input)[0], drive.tag);
}

#[test]
fn the_cooling_lessons_start_from_a_cabinet_with_a_device_over_its_limit() {
    for (lesson, json) in [("cooling", COOLING), ("cooler-layout", COOLER_LAYOUT)] {
        let (project, cab) = open(json);
        let bridge = scenario_from_project(&project, cab, &BridgeOptions::default()).unwrap();
        let result = novo_thermal::simulate_steady(&bridge.scenario).unwrap();
        assert!(
            result.devices.iter().any(|d| d.status == Status::Over),
            "{lesson}: no device is over its limit in the 2D model"
        );
    }
}

const CABINET_LAYOUT: &str = include_str!("../../../../assets/lessons/cabinet-layout/project.json");
const CABINET_LAYOUT_FINISHED: &str =
    include_str!("../../../../assets/lessons/cabinet-layout/finished.json");

fn cells(row: &novo_reports::Row) -> Vec<String> {
    row.cells.iter().map(ToString::to_string).collect()
}

/// The text of one cell of a report section, by column key.
fn cell(section: &novo_reports::Section, row: &novo_reports::Row, key: &str) -> String {
    row.cells[section
        .column(key)
        .unwrap_or_else(|| panic!("no column {key}"))]
    .to_string()
}

#[test]
fn the_wire_routing_lesson_quotes_the_routes_panel() {
    let (mut project, cab) = open(WIRE_ROUTING);
    let o = super::cabinet_routes::overlay(&project, cab);
    // "11 wires routed, fullest duct 10%".
    assert_eq!(o.summary(), "11 wires routed, fullest duct 10%");
    // "The longest are the three from -F2 to -X1 at 966 to 1014 mm", longest first.
    let lengths: Vec<f64> = o.rows.iter().map(|r| r.length_mm).collect();
    assert_eq!(lengths[..3], [1014.0, 990.0, 966.0]);
    assert!(lengths[3] < 966.0);
    assert!(
        o.rows[..3]
            .iter()
            .all(|r| r.ends.contains("-F2") && r.ends.contains("-X1"))
    );
    // "Set Service loop per end from 50 mm to 100 mm: every length grows by 100 mm (966 becomes 1066)."
    project
        .transact("rule", |tx| {
            novo_routing::project_rules::set_rule(
                tx,
                novo_routing::project_rules::SLACK_MM,
                Some(100.0),
            )
        })
        .unwrap();
    let o = super::cabinet_routes::overlay(&project, cab);
    assert_eq!(
        o.rows[..3].iter().map(|r| r.length_mm).collect::<Vec<_>>(),
        [1114.0, 1090.0, 1066.0]
    );
}

#[test]
fn the_cut_lengths_lesson_quotes_the_wire_list_and_the_cut_list() {
    use novo_reports::{CutList, Report, WireList};
    let (project, _) = open(WIRE_ROUTING);
    let list = WireList.generate(&project);
    let wires = &list.sections[0];
    // "Of the 19 rows here, 15 say routed and 4 say cable length."
    assert_eq!(wires.rows.len(), 19);
    let basis = |b: &str| {
        wires
            .rows
            .iter()
            .filter(|r| cell(wires, r, "length_basis") == b)
            .count()
    };
    assert_eq!((basis("routed"), basis("cable length")), (15, 4));
    // "Wire 4, from -F2 to -X1, is 966 mm; the wire between the push buttons -S0 and -S1 only 335."
    let row = |no: &str| {
        wires
            .rows
            .iter()
            .find(|r| cell(wires, r, "number") == no)
            .unwrap()
    };
    assert_eq!(cell(wires, row("4"), "length_mm"), "966");
    assert!(
        cell(wires, row("4"), "from").ends_with("-F2:2")
            && cell(wires, row("4"), "to").ends_with("-X1:1")
    );
    let buttons = wires
        .rows
        .iter()
        .find(|r| {
            cell(wires, r, "from").ends_with("-S0:12") && cell(wires, r, "to").ends_with("-S1:13")
        })
        .unwrap();
    assert_eq!(cell(wires, buttons, "length_mm"), "335");
    // The four motor wires are the cores of cable -W1, 12 m long.
    for r in wires
        .rows
        .iter()
        .filter(|r| cell(wires, r, "length_basis") == "cable length")
    {
        assert_eq!(cell(wires, r, "length_mm"), "12000");
        assert!(cell(wires, r, "to").contains("-M1") || cell(wires, r, "from").contains("-X1"));
    }
    // The cut list: "4 wires of 1.5 mm² (no metres: cores) and 21 wires whose size is not set (10.85 m)".
    let cut = CutList::of(&project).generate(&project);
    let summary = &cut.sections[0];
    let rows: Vec<Vec<String>> = summary.rows.iter().map(cells).collect();
    assert_eq!(rows[0], ["1.5 mm²", "4", "0"]);
    assert_eq!(rows[1], ["Size unknown", "21", "10.85"]);
    // "Wire 4 shows Wire mm 966, Ends mm 20 and Cut mm 986."
    let group = cut.sections.iter().find(|s| s.key == "group-2").unwrap();
    let w4 = group
        .rows
        .iter()
        .find(|r| cell(group, r, "number") == "4")
        .unwrap();
    assert_eq!(
        [
            cell(group, w4, "length_mm"),
            cell(group, w4, "allowance_mm"),
            cell(group, w4, "cut_mm")
        ],
        ["966", "20", "986"]
    );
}

#[test]
fn the_panel_labels_lesson_quotes_the_label_counts() {
    use novo_reports::{Labels, Report};
    let (project, _) = open(WIRE_ROUTING);
    let out = Labels.generate(&project);
    let count = |key: &str| {
        out.sections
            .iter()
            .find(|s| s.key == key)
            .map_or(0, |s| s.rows.len())
    };
    // "wires (50 rows, one per wire end), terminals (4), devices (9) and cables (1)": 25 wires,
    // the 19 of the wire list and the six supply feeds L1, L2, L3, PE, L and N.
    assert_eq!(
        (
            count("wires"),
            count("terminals"),
            count("devices"),
            count("cables")
        ),
        (50, 4, 9, 1)
    );
    let wires = &out.sections[0];
    let feeds = wires
        .rows
        .iter()
        .filter(|r| {
            cell(wires, r, "line2").ends_with("(feed)")
                || cell(wires, r, "line3").ends_with("(feed)")
        })
        .count();
    assert_eq!(feeds, 12);
    // "A row reads for example 2 | =A1+CAB-K1:1 | =A1+CAB-F1:2".
    assert!(
        wires
            .rows
            .iter()
            .any(|r| cells(r) == ["2", "=A1+CAB-K1:1", "=A1+CAB-F1:2"])
    );
    // "-F1 Main circuit fuses", "-W1 Motor cable 4G1.5, 4 cores".
    let devices = out.sections.iter().find(|s| s.key == "devices").unwrap();
    assert!(
        devices
            .rows
            .iter()
            .any(|r| cells(r) == ["=A1+CAB-F1", "Main circuit fuses", ""])
    );
    let cables = out.sections.iter().find(|s| s.key == "cables").unwrap();
    assert_eq!(
        cells(&cables.rows[0]),
        ["=A1-W1", "Motor cable 4G1.5", "4 cores"]
    );
}

#[test]
fn the_plate_lessons_quote_the_drill_table_and_the_rails() {
    let (project, cab) = open(WIRE_ROUTING);
    let layout = novo_layout::model::load(&project, cab).unwrap();
    let features: Vec<_> = layout
        .placements
        .iter()
        .filter_map(|p| project.get::<Device>(DeviceId::from(p.id)))
        .map(|d| novo_layout::model::features(&project, d))
        .collect();
    // "20 holes of 5.5 mm and 4 of 6 mm"; "four holes on each 535 mm rail here, 20 in all".
    let summary = novo_layout::drill::drill_table(&layout, &features).summary();
    assert_eq!(
        summary.iter().map(|(_, d, n)| (*d, *n)).collect::<Vec<_>>(),
        [(5.5, 20), (6.0, 4)]
    );
    assert_eq!(layout.rails.len(), 5);
    assert!(layout.rails.iter().all(|r| r.length == 535.0));
    // "The five rails of 535 mm and the six ducts (five of 557.5 mm and the trunk of 780 mm)".
    let mut ducts: Vec<f64> = layout.ducts.iter().map(|d| d.length).collect();
    ducts.sort_by(f64::total_cmp);
    assert_eq!(ducts, [557.5, 557.5, 557.5, 557.5, 557.5, 780.0]);
}

#[test]
fn the_door_and_depth_lessons_quote_the_checks() {
    use novo_layout::enclosure::{Door, Enclosure};
    let (mut project, cab) = open(WIRE_ROUTING);
    let set = |project: &mut Project, e: Enclosure| {
        project
            .transact("inside size", |tx| {
                novo_layout::model::set_enclosure(tx, cab, &e)
            })
            .unwrap();
        novo_layout::model::load(project, cab)
            .unwrap()
            .door_report()
            .unwrap()
    };
    let tag = |project: &Project, id: u64| {
        project
            .get::<Device>(DeviceId::from(id))
            .unwrap()
            .tag
            .to_string()
    };
    let base = Enclosure {
        width: 600.0,
        height: 800.0,
        depth: 250.0,
        plate_setback: 20.0,
        door: Some(Door::default()),
        front_clearance: None,
        hinge_side_clearance: None,
    };
    // "230 mm usable depth", "The open door needs 600 mm in front".
    let r = set(&mut project, base);
    assert!(r.is_clean(), "{r:?}");
    assert_eq!(base.usable_depth(), 230.0);
    assert_eq!(r.swing.unwrap().front, 600.0);
    // Depth 100: "usable depth 100 - 20 = 80 mm; -K1 and -F2 10 mm too deep, -T1 5 mm; -F1 and -F3 still fit".
    let r = set(
        &mut project,
        Enclosure {
            depth: 100.0,
            ..base
        },
    );
    let mut too_deep: Vec<(String, f64)> = r
        .too_deep
        .iter()
        .map(|(id, mm)| (tag(&project, *id), *mm))
        .collect();
    too_deep.sort_by(|a, b| a.0.cmp(&b.0));
    let names: Vec<_> = too_deep
        .iter()
        .map(|(t, mm)| (t.rsplit('-').next().unwrap().to_owned(), *mm))
        .collect();
    assert_eq!(
        names,
        [
            ("F2".to_owned(), 10.0),
            ("K1".to_owned(), 10.0),
            ("T1".to_owned(), 5.0)
        ]
    );
    assert!(r.tight.is_empty(), "{r:?}");
    // 500 mm free in front: "600 mm needed, 500 free (100 mm short)"; opening to 60 degrees: about 520.
    let r = set(
        &mut project,
        Enclosure {
            front_clearance: Some(500.0),
            ..base
        },
    );
    assert_eq!(r.front_short, Some(100.0));
    let sixty = Enclosure {
        door: Some(Door {
            max_angle: 60.0,
            ..Door::default()
        }),
        ..base
    };
    assert_eq!(format!("{:.0}", sixty.swing().unwrap().front), "520");
}

#[test]
fn the_cabinet_layout_lessons_quote_the_unplaced_list_and_the_rails() {
    let (project, cab) = open(CABINET_LAYOUT);
    let layout = novo_layout::model::load(&project, cab).unwrap();
    // "0 placed, 8 unplaced"; -F1 is a 54 x 90 mm front.
    assert!(layout.placements.is_empty());
    assert_eq!(novo_layout::model::candidates(&project, cab).len(), 8);
    let (project, cab) = open(CABINET_LAYOUT_FINISHED);
    let layout = novo_layout::model::load(&project, cab).unwrap();
    assert_eq!(layout.placements.len(), 8);
    assert!(layout.placements.iter().any(|p| {
        let f = p.footprint();
        (f.w, f.h) == (54.0, 90.0)
    }));
}

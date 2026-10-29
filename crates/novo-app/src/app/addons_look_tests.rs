//! Add-on icons and commands in the look tables (lane V0 meets lane C0b).

use std::collections::BTreeMap;

use novo_addons::icons::{IconDef, Shape};
use novo_addons::{ExtCommand, Extensions, Home};
use novo_theme::drawn::{self, Icon, Prim};
use novo_theme::presentation::{self, ADDON_ICON, Glyph};

use super::addons_look::{TABLES, icon_spec, sync};

fn command(name: &str, icon: &str, label: &str) -> ExtCommand {
    ExtCommand {
        id: format!("addon.looktest.{name}"),
        addon_id: "looktest".into(),
        addon_name: "Look test".into(),
        label: label.into(),
        description: String::new(),
        icon: icon.into(),
        keywords: vec![],
        key: None,
        key_conflict: false,
        home: Home::PaletteOnly,
        opens: None,
    }
}

fn pendant() -> IconDef {
    IconDef {
        name: "looktest.pendant".into(),
        tooltip: "Pendant".into(),
        shape: vec![
            Shape::Rect {
                x: 6.0,
                y: 3.0,
                w: 12.0,
                h: 18.0,
                r: 2.0,
            },
            Shape::Arc {
                cx: 12.0,
                cy: 12.0,
                r: 5.0,
                from: 0.0,
                to: 90.0,
            },
            Shape::Circle {
                cx: 12.0,
                cy: 18.0,
                r: 1.0,
                filled: true,
            },
        ],
    }
}

fn extensions() -> Extensions {
    Extensions {
        commands: vec![
            command("own", "looktest.pendant", "Pendant panel"),
            command("builtin", "contactor", "Contactor test"),
            command("unknown", "no-such-icon", "Unknown icon"),
            command("none", "", "No icon"),
        ],
        panels: vec![],
        icons: BTreeMap::from([("looktest.pendant".to_owned(), pendant())]),
    }
}

#[test]
fn running_add_ons_join_the_look_tables_and_disabled_ones_leave() {
    let _tables = TABLES.lock().unwrap_or_else(|e| e.into_inner());
    let done = sync(&extensions());
    assert_eq!(done.icons, 1);
    assert_eq!(done.commands, 4);
    assert!(done.refused.is_empty() && done.refused_commands.is_empty());

    let own = Icon::by_name("looktest.pendant").expect("the add-on's icon is registered");
    assert_eq!(own.title, "Pendant");
    assert_eq!(presentation::glyph("addon.looktest.own"), Glyph::Drawn(own));
    assert_eq!(
        presentation::glyph("addon.looktest.builtin"),
        Glyph::Drawn(&drawn::CONTACTOR)
    );
    assert_eq!(presentation::glyph("addon.looktest.unknown"), ADDON_ICON);
    assert_eq!(presentation::glyph("addon.looktest.none"), ADDON_ICON);
    assert_eq!(
        presentation::short_label("addon.looktest.own"),
        Some("Pendant panel")
    );

    // The add-on set changed (disabled or removed): nothing stays behind.
    let gone = sync(&Extensions::default());
    assert_eq!(gone, Default::default());
    assert!(Icon::by_name("looktest.pendant").is_none());
    assert_eq!(presentation::of("addon.looktest.own"), None);
    // The built-in set is untouched.
    assert_eq!(drawn::ALL.len(), 40);
    assert_eq!(presentation::short_label("file.save"), Some("Save"));
}

#[test]
fn a_command_label_the_table_refuses_is_reported_and_the_rest_still_work() {
    let _tables = TABLES.lock().unwrap_or_else(|e| e.into_inner());
    let mut ext = extensions();
    ext.commands.push(command(
        "long",
        "",
        &"x".repeat(presentation::MAX_SHORT + 1),
    ));
    let done = sync(&ext);
    assert_eq!(
        done.refused_commands,
        vec!["addon.looktest.long".to_owned()]
    );
    assert_eq!(done.commands, 4);
    sync(&Extensions::default());
}

#[test]
fn an_add_on_arc_runs_counter_clockwise_on_screen() {
    // 0 to 90 degrees around (12, 12): from the right (x 17) to the top (y 7),
    // so the middle of the arc is up and to the right.
    let spec = icon_spec(&pendant());
    let _tables = TABLES.lock().unwrap_or_else(|e| e.into_inner());
    let icon = drawn::register(spec).unwrap();
    let arc = icon.shapes[1].prims(2.0);
    let Prim::Stroke { points, .. } = &arc[0] else {
        panic!("a stroke")
    };
    let first = points.first().unwrap();
    let last = points.last().unwrap();
    assert!(
        (first[0] - 17.0).abs() < 1e-3 && (first[1] - 12.0).abs() < 1e-3,
        "{first:?}"
    );
    assert!(
        (last[0] - 12.0).abs() < 1e-3 && (last[1] - 7.0).abs() < 1e-3,
        "{last:?}"
    );
    let mid = points[points.len() / 2];
    assert!(mid[0] > 12.0 && mid[1] < 12.0, "{mid:?}");
    drawn::clear();
}

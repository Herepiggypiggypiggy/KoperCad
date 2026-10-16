//! The names `{ui:id}` shows in lesson text for every anchor, so a step
//! reads the same whether or not the control has been drawn yet. Add a line
//! here with each new entry of `ANCHORS` (a test checks both lists agree).

/// `(anchor id, the name on screen)`. The name is what the control says (a
/// button's text) or what the person calls it (a panel, a badge).
pub(crate) const ANCHOR_LABELS: &[(&str, &str)] = &[
    ("start.learn", "Learn"),
    ("menu.project", "KoperCAD menu"),
    ("palette.button", "Search or run"),
    ("palette.field", "Command field"),
    ("dock.select", "Select"),
    ("dock.place", "Place"),
    ("dock.connect", "Connect"),
    ("dock.corners", "Corners"),
    ("dock.lines", "Lines"),
    ("dock.hint", "Hint line"),
    ("dock.proposals", "Proposals"),
    ("issue.pill", "Issues"),
    ("drawer.terminals", "Terminals drawer"),
    ("drawer.terminals.tab", "Terminals tab"),
    ("drawer.cables", "Cables drawer"),
    ("drawer.cables.tab", "Cables tab"),
    ("drawer.cables.accept", "Accept proposed cables"),
    ("drawer.devices", "Devices drawer"),
    ("drawer.devices.tab", "Devices tab"),
    ("live.card", "Live potentials"),
    ("live.pole", "Pole badge"),
    ("live.coil", "Coil badge"),
    ("live.output", "Output badge"),
    ("cabinet.place_all", "Place all"),
    ("cabinet.accept", "Accept"),
    ("cabinet.rules", "Rules"),
    ("cabinet.plate", "Plate setup"),
    ("cabinet.routes", "Route all"),
    ("cabinet.cutlist", "Cut list"),
    ("cabinet.table", "Table"),
    ("cabinet.heat", "Heat"),
    ("heat.chips", "Heat chips"),
    ("heat.legend", "Heat legend"),
    ("heat.simulate", "Simulate button"),
    ("cabinet.canvas", "Cabinet canvas"),
    ("workspace.view3d", "3D canvas"),
    ("switcher.schematic", "Schematic chip"),
    ("switcher.cabinet", "Cabinet chip"),
    ("switcher.view3d", "3D chip"),
    ("switcher.reports", "Reports chip"),
    ("switcher.commissioning", "Commissioning chip"),
    ("outline.pages", "Pages list"),
    ("outline.toggle", "Pages button"),
    ("commissioning.who", "By field"),
    ("commissioning.punch", "Punch tab"),
    ("commissioning.report", "Report PDF button"),
    ("commissioning.next_open", "Next open button"),
    ("commissioning.pass", "Pass button"),
    ("commissioning.fail", "Fail button"),
    ("commissioning.resolve", "Resolve button"),
    ("cabinet.unplaced", "Unplaced list"),
    ("cabinet.problems", "Problems"),
    ("cabinet.inspector", "Cabinet inspector"),
    ("cabinet.door", "Door and depth"),
    ("cabinet.door.apply", "Apply"),
    ("cabinet.routes_panel", "Routes panel"),
    ("cabinet.wire_rules", "Wire rules"),
    ("cabinet.sizing", "Size enclosure"),
    ("cabinet.rails", "Rails"),
    ("cabinet.plate_drawing", "Plate drawing"),
    ("cabinet.more", "More"),
    ("issues.list", "Issues list"),
    ("macro.place", "Place macro"),
    ("macros.window", "Macros"),
];

/// The name of an anchor, if the table has it.
pub(crate) fn anchor_label(id: &str) -> Option<&'static str> {
    ANCHOR_LABELS
        .iter()
        .find(|(anchor, _)| *anchor == id)
        .map(|(_, label)| *label)
}

/// A menu entry as an anchor: `menu.File` for the menu in the bar and
/// `cmd.export.sheets` for the command row inside it, so a lesson writes
/// `{ui:cmd.export.sheets}` and the card shows the row's own label.
#[derive(Debug, Clone)]
pub(crate) struct MenuAnchor {
    pub id: String,
    pub label: String,
    /// The anchor to point at while this one is not on screen (the menu a
    /// command row lives in is closed).
    pub parent: Option<String>,
}

/// Every menu and every command row of the bar's menus.
pub(crate) fn menu_anchors() -> &'static [MenuAnchor] {
    static ALL: std::sync::OnceLock<Vec<MenuAnchor>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| {
        use novo_workflow::menus::{self, MENUS, MenuItem};
        let mut all = Vec::new();
        for menu in MENUS {
            let items = menus::layout(menu);
            if items.is_empty() {
                continue;
            }
            let parent = format!("menu.{menu}");
            all.push(MenuAnchor {
                id: parent.clone(),
                label: menu.to_owned(),
                parent: None,
            });
            let row = |info: &novo_workflow::commands::CommandInfo| MenuAnchor {
                id: format!("cmd.{}", info.id),
                label: info.label.to_owned(),
                parent: Some(parent.clone()),
            };
            for item in &items {
                match item {
                    MenuItem::Command(info) => all.push(row(info)),
                    MenuItem::Submenu { items, .. } => all.extend(items.iter().map(|i| row(i))),
                    MenuItem::Separator => {}
                }
            }
        }
        all
    })
}

/// The name `{ui:id}` shows: the table's, or a menu entry's own label.
pub(crate) fn any_anchor_label(id: &str) -> Option<String> {
    anchor_label(id).map(str::to_owned).or_else(|| {
        menu_anchors()
            .iter()
            .find(|m| m.id == id)
            .map(|m| m.label.clone())
    })
}

/// Where to point when `id` is not on screen this frame: the menu a
/// command row is in, or the More button a folded cabinet bar button hides
/// behind.
pub(crate) fn anchor_fallback(id: &str) -> Option<String> {
    if let Some(m) = menu_anchors().iter().find(|m| m.id == id) {
        return m.parent.clone();
    }
    match id {
        "cabinet.rules"
        | "cabinet.plate"
        | "cabinet.sizing"
        | "cabinet.cutlist"
        | "cabinet.heat"
        | "cabinet.rails"
        | "cabinet.plate_drawing" => Some("cabinet.more".to_owned()),
        // The list and the field are not there until their button is pressed.
        "outline.pages" => Some("outline.toggle".to_owned()),
        "palette.field" => Some("palette.button".to_owned()),
        "heat.chips" | "heat.legend" | "heat.simulate" => Some("cabinet.heat".to_owned()),
        _ => None,
    }
}

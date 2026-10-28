//! What running add-ons add to the program's own tables: commands, panels, icons. The program
//! builds one [`Extensions`] from the scanned [`AddonSet`] whenever an add-on is installed,
//! turned on or off, or removed, and its menu, rail, command search and panel host read it
//! next to the static tables (`docs/design/visual-redesign-addons.md`).
//!
//! A disabled, failed or removed add-on contributes nothing, so it leaves nothing in the menus,
//! on the rail or in Ctrl K.

use std::collections::BTreeMap;

use crate::icons::IconDef;
use crate::manifest::{CommandDecl, HomeKind, PanelDecl, Slot};
use crate::store::AddonSet;

/// Where an add-on command is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Home {
    /// At the foot of this menu area, under the label "Add-ons".
    Menu(String),
    /// A row in the add-on's flyout on these workspace rails.
    Rail(Vec<String>),
    /// Only in Ctrl K.
    PaletteOnly,
}

/// One command from an add-on, as a runtime descriptor (not a row of the static command table).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtCommand {
    /// `addon.<add-on id>.<name>`.
    pub id: String,
    pub addon_id: String,
    pub addon_name: String,
    /// At most 28 characters.
    pub label: String,
    /// The long sentence for the tooltip and the Ctrl K description.
    pub description: String,
    /// Icon name to resolve: the add-on's own (`<id>.<name>`), a built-in name, else the glyph.
    pub icon: String,
    pub keywords: Vec<String>,
    /// The wanted key. `key_conflict` says the program already uses it (the program wins).
    pub key: Option<String>,
    pub key_conflict: bool,
    pub home: Home,
    /// The full id of the panel this command opens (`addon.<id>.<panel>`), if any. Without one
    /// the add-on's engine handles the command.
    pub opens: Option<String>,
}

/// One panel an add-on declares, drawn by the host with `panel.rs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtPanel {
    /// `addon.<add-on id>.<panel>`: the `panel::id` of the host panel.
    pub id: String,
    pub addon_id: String,
    pub addon_name: String,
    pub title: String,
    pub slot: Slot,
    /// The layout file inside the add-on.
    pub file: String,
}

/// The merged contributions of all running add-ons.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Extensions {
    pub commands: Vec<ExtCommand>,
    pub panels: Vec<ExtPanel>,
    /// Own icons by full name.
    pub icons: BTreeMap<String, IconDef>,
}

impl Extensions {
    /// Builds the tables from the running add-ons. `key_in_use` says whether the program
    /// already binds a key (so the add-on's wish loses).
    pub fn build(set: &AddonSet, key_in_use: &dyn Fn(&str) -> bool) -> Extensions {
        Extensions::build_excluding(set, key_in_use, &|_| false)
    }

    /// As [`Extensions::build`], leaving out the add-ons `stopped` names: one whose engine
    /// stopped this session contributes nothing until it is turned off and on again.
    pub fn build_excluding(
        set: &AddonSet,
        key_in_use: &dyn Fn(&str) -> bool,
        stopped: &dyn Fn(&str) -> bool,
    ) -> Extensions {
        let mut out = Extensions::default();
        let mut keys_taken: Vec<String> = Vec::new();
        for entry in set.enabled().filter(|e| !stopped(&e.id)) {
            let Some(manifest) = &entry.manifest else {
                continue;
            };
            let addon = &manifest.addon;
            for icon in &entry.icons {
                out.icons.insert(icon.name.clone(), icon.clone());
            }
            for p in &manifest.panels {
                out.panels.push(panel(&addon.id, &addon.name, p));
            }
            for c in &manifest.commands {
                let mut ext = command(manifest.command_id(&c.id), &addon.id, &addon.name, c);
                if let Some(key) = &c.key {
                    let norm = key.to_ascii_lowercase();
                    ext.key_conflict = key_in_use(key) || keys_taken.contains(&norm);
                    if !ext.key_conflict {
                        keys_taken.push(norm);
                    }
                }
                out.commands.push(ext);
            }
        }
        out
    }

    pub fn command(&self, id: &str) -> Option<&ExtCommand> {
        self.commands.iter().find(|c| c.id == id)
    }

    pub fn panel(&self, id: &str) -> Option<&ExtPanel> {
        self.panels.iter().find(|p| p.id == id)
    }

    /// The commands for the foot of one menu area, grouped by the order add-ons were found.
    pub fn menu_rows(&self, area: &str) -> Vec<&ExtCommand> {
        self.commands
            .iter()
            .filter(|c| matches!(&c.home, Home::Menu(m) if m == area))
            .collect()
    }

    /// The rail flyout rows of one workspace: `(add-on id, add-on name, commands)`.
    pub fn rail_rows(&self, workspace: &str) -> Vec<(String, String, Vec<&ExtCommand>)> {
        let mut groups: Vec<(String, String, Vec<&ExtCommand>)> = Vec::new();
        for c in &self.commands {
            if matches!(&c.home, Home::Rail(ws) if ws.iter().any(|w| w == workspace)) {
                match groups.iter_mut().find(|g| g.0 == c.addon_id) {
                    Some(g) => g.2.push(c),
                    None => groups.push((c.addon_id.clone(), c.addon_name.clone(), vec![c])),
                }
            }
        }
        groups
    }

    /// Panels shown in one slot.
    pub fn panels_in(&self, slot: Slot) -> impl Iterator<Item = &ExtPanel> {
        self.panels.iter().filter(move |p| p.slot == slot)
    }
}

fn panel(addon_id: &str, addon_name: &str, p: &PanelDecl) -> ExtPanel {
    ExtPanel {
        id: format!("addon.{addon_id}.{}", p.id),
        addon_id: addon_id.to_owned(),
        addon_name: addon_name.to_owned(),
        title: p.title.clone(),
        slot: p.slot,
        file: p.file.clone(),
    }
}

fn command(id: String, addon_id: &str, addon_name: &str, c: &CommandDecl) -> ExtCommand {
    ExtCommand {
        id,
        addon_id: addon_id.to_owned(),
        addon_name: addon_name.to_owned(),
        label: c.label.clone(),
        description: c.description.clone(),
        icon: c.icon.clone(),
        keywords: c.keywords.clone(),
        key: c.key.clone(),
        key_conflict: false,
        home: match c.home {
            HomeKind::Menu => Home::Menu(c.menu.clone().unwrap_or_default()),
            HomeKind::Rail => Home::Rail(c.workspaces.clone()),
            HomeKind::PaletteOnly => Home::PaletteOnly,
        },
        opens: c.opens.as_ref().map(|p| format!("addon.{addon_id}.{p}")),
    }
}

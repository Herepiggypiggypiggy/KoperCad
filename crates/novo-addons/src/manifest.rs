//! `manifest.toml`: what an add-on is, what it needs and what it adds. Everything the base
//! program shows or does for an add-on comes from this file, so every rule is checked here, at
//! install and again at each start. Where add-on contributions appear is settled in
//! `docs/design/visual-redesign-addons.md`.
//!
//! ```toml
//! [addon]
//! id = "controllers"
//! name = "Controllers"
//! version = "0.1.0"
//! publisher = "KoperVonk"
//! description = "Virtual operator controllers for the live view."
//! requires_app = ">=0.1.0"
//! api_version = 1
//! permissions = ["menu", "live-view-card"]
//!
//! [engine]                 # optional: a sandboxed WebAssembly module (C0b-2)
//! file = "engine.wasm"
//! host_api = 1
//!
//! [[command]]
//! id = "panel"
//! label = "Controller panel"
//! description = "Open the virtual controller panel."
//! icon = "controllers.radio"   # own icon (data/icons.toml), a built-in name, else the add-on glyph
//! home = "menu"
//! menu = "Simulate"
//! opens = "radio"
//!
//! [[panel]]
//! id = "radio"
//! title = "Radio"
//! slot = "live-view-card"
//! file = "data/panels/radio.panel.toml"
//!
//! [[contribute]]
//! kind = "controller-profile"
//! file = "data/generic-radio.controller.toml"
//!
//! [files]                  # SHA-256 of every file except manifest.toml and signature
//! "data/generic-radio.controller.toml" = "<64 hex digits>"
//! ```

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::error::{AddonError, Result};
use crate::version::{Requirement, Version};

/// The add-on API generation this program speaks. A package for another one is refused with a
/// message that says so.
pub const API_VERSION: u32 = 1;
/// The host API generation of the sandboxed engine (C0b-2).
pub const HOST_API_VERSION: u32 = 1;
/// A menu row, rail row or palette result is at most this long (visual redesign, 5.2).
pub const MAX_LABEL: usize = 28;
/// Most rows one add-on may put in one menu area.
pub const MAX_MENU_ROWS_PER_AREA: usize = 4;
/// Most rows one add-on may put in its flyout of one workspace rail.
pub const MAX_RAIL_ROWS_PER_WORKSPACE: usize = 6;
/// Largest `manifest.toml` read.
pub const MAX_MANIFEST_BYTES: usize = 256 * 1024;
/// The optional file with the add-on's own icons.
pub const ICONS_FILE: &str = "data/icons.toml";

/// The menu areas an add-on command may sit in (the app menu's areas, visual redesign 5.1).
pub const MENU_AREAS: [&str; 8] = [
    "File", "Edit", "Draw", "View", "Check", "Reports", "Simulate", "Help",
];
/// The workspaces a rail row may be added to.
pub const WORKSPACES: [&str; 5] = ["schematic", "cabinet", "3d", "reports", "commissioning"];

/// What an add-on may do. Listed in the install prompt and in Details; each one the add-on
/// uses must be declared, and one it does not declare is refused at load.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Permission {
    /// Menu entries.
    Menu,
    /// Rows in its flyout on a workspace rail.
    RailItem,
    /// A card in the live view side panel.
    LiveViewCard,
    /// A floating panel the person can move and resize.
    FloatingPanel,
    /// Read the open project (devices, pins, wires).
    ReadProject,
    /// Change bindings the add-on keeps in the project.
    WriteBindings,
    /// Add symbols to the symbol library.
    AddSymbols,
    /// Add design checks.
    AddChecks,
    /// Read input devices such as a gamepad.
    InputDevices,
}

impl Permission {
    pub const ALL: [Permission; 9] = [
        Permission::Menu,
        Permission::RailItem,
        Permission::LiveViewCard,
        Permission::FloatingPanel,
        Permission::ReadProject,
        Permission::WriteBindings,
        Permission::AddSymbols,
        Permission::AddChecks,
        Permission::InputDevices,
    ];

    /// The name used in `manifest.toml`.
    pub fn key(self) -> &'static str {
        match self {
            Permission::Menu => "menu",
            Permission::RailItem => "rail-item",
            Permission::LiveViewCard => "live-view-card",
            Permission::FloatingPanel => "floating-panel",
            Permission::ReadProject => "read-project",
            Permission::WriteBindings => "write-bindings",
            Permission::AddSymbols => "add-symbols",
            Permission::AddChecks => "add-checks",
            Permission::InputDevices => "input-devices",
        }
    }

    /// One plain sentence for the install prompt.
    pub fn describe(self) -> &'static str {
        match self {
            Permission::Menu => "Add entries to the menus and the command search",
            Permission::RailItem => "Add rows to its flyout on a workspace's tool rail",
            Permission::LiveViewCard => "Show a card in the live view side panel",
            Permission::FloatingPanel => "Open its own movable panel",
            Permission::ReadProject => "Read the open project (devices, pins and wires)",
            Permission::WriteBindings => "Save its own bindings in the project",
            Permission::AddSymbols => "Add symbols to the symbol library",
            Permission::AddChecks => "Add design checks",
            Permission::InputDevices => "Read input devices such as a gamepad",
        }
    }
}

/// `[addon]`: identity and requirements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddonInfo {
    /// Lower-case letters, digits and hyphens, 2 to 48 characters, starting with a letter.
    pub id: String,
    pub name: String,
    pub version: String,
    pub publisher: String,
    #[serde(default)]
    pub description: String,
    /// Program versions it works with, for example `>=0.1.0, <0.3`.
    #[serde(default = "any_version")]
    pub requires_app: String,
    /// Add-on API generation.
    pub api_version: u32,
    #[serde(default)]
    pub permissions: Vec<Permission>,
    /// SPDX expression for the details page; the file `LICENSE` may also be in the package.
    #[serde(default)]
    pub license: String,
}

fn any_version() -> String {
    "*".to_owned()
}

/// `[engine]`: the sandboxed WebAssembly module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineDecl {
    pub file: String,
    pub host_api: u32,
}

/// Where a command lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HomeKind {
    Menu,
    Rail,
    PaletteOnly,
}

/// `[[command]]`: one command the add-on adds to the registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandDecl {
    /// Local id; the registry id is `addon.<add-on id>.<id>`.
    pub id: String,
    /// At most [`MAX_LABEL`] characters.
    pub label: String,
    #[serde(default)]
    pub description: String,
    /// Icon name: the add-on's own (`<add-on id>.<name>`), a built-in name, else the add-on glyph.
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    /// A wanted key such as `Shift+L`; a key the program already uses wins.
    #[serde(default)]
    pub key: Option<String>,
    pub home: HomeKind,
    /// For `home = "menu"`: one of [`MENU_AREAS`].
    #[serde(default)]
    pub menu: Option<String>,
    /// For `home = "rail"`: the workspaces ([`WORKSPACES`]) whose flyout lists the command.
    #[serde(default)]
    pub workspaces: Vec<String>,
    /// Opens this declared panel. Without it the command is handled by the add-on's engine.
    #[serde(default)]
    pub opens: Option<String>,
}

/// Where a panel is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Slot {
    LiveViewCard,
    Floating,
}

impl Slot {
    pub fn permission(self) -> Permission {
        match self {
            Slot::LiveViewCard => Permission::LiveViewCard,
            Slot::Floating => Permission::FloatingPanel,
        }
    }
}

/// `[[panel]]`: a panel described as data and drawn by the program's own widgets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelDecl {
    pub id: String,
    pub title: String,
    pub slot: Slot,
    /// The layout file in the package.
    pub file: String,
}

/// `[[contribute]]`: a data file for something the program already reads (controller profiles,
/// symbols, lessons, word lists). The program asks for a `kind` it understands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contribution {
    pub kind: String,
    pub file: String,
}

/// A parsed, validated `manifest.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub addon: AddonInfo,
    #[serde(default)]
    pub engine: Option<EngineDecl>,
    #[serde(default, rename = "command")]
    pub commands: Vec<CommandDecl>,
    #[serde(default, rename = "panel")]
    pub panels: Vec<PanelDecl>,
    #[serde(default, rename = "contribute")]
    pub contributions: Vec<Contribution>,
    /// Package path to lower-case SHA-256 hex, for every file except `manifest.toml` and `signature`.
    #[serde(default)]
    pub files: BTreeMap<String, String>,
}

/// Only the part that decides whether the rest can be understood.
#[derive(Deserialize)]
struct Header {
    addon: HeaderAddon,
}
#[derive(Deserialize)]
struct HeaderAddon {
    api_version: Option<u32>,
}

impl Manifest {
    /// Parses and validates. A manifest for another API generation is reported as incompatible
    /// before anything else is judged, because its other fields may mean something else.
    pub fn parse(bytes: &[u8]) -> Result<Manifest> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(AddonError::Manifest(
                "manifest.toml is larger than 256 KiB".into(),
            ));
        }
        let text = std::str::from_utf8(bytes)
            .map_err(|_| AddonError::Manifest("manifest.toml is not UTF-8 text".into()))?;
        let header: Header =
            toml::from_str(text).map_err(|e| AddonError::Manifest(e.to_string()))?;
        match header.addon.api_version {
            Some(v) if v != API_VERSION => {
                return Err(AddonError::Incompatible(format!(
                    "this add-on is built for add-on API {v}; this KoperCAD speaks API {API_VERSION}. \
                     Install a matching version of the add-on or update KoperCAD"
                )));
            }
            None => return Err(AddonError::Manifest("addon.api_version is missing".into())),
            _ => {}
        }
        let manifest: Manifest =
            toml::from_str(text).map_err(|e| AddonError::Manifest(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn version(&self) -> Version {
        Version::parse(&self.addon.version).unwrap_or_default()
    }

    pub fn requirement(&self) -> Requirement {
        Requirement::parse(&self.addon.requires_app)
            .unwrap_or_else(|| Requirement::parse("*").expect("* is a range"))
    }

    /// The registry id of one of this add-on's commands.
    pub fn command_id(&self, local: &str) -> String {
        format!("addon.{}.{local}", self.addon.id)
    }

    /// Whether the add-on declares `p`.
    pub fn allows(&self, p: Permission) -> bool {
        self.addon.permissions.contains(&p)
    }

    fn validate(&self) -> Result<()> {
        let bad = |m: String| Err(AddonError::Manifest(m));
        let a = &self.addon;
        if !valid_id(&a.id, 2, 48) {
            return bad(format!(
                "addon.id {:?} must be 2 to 48 characters: lower-case letters, digits and hyphens, starting with a letter",
                a.id
            ));
        }
        if a.name.trim().is_empty() || a.name.chars().count() > 40 {
            return bad("addon.name must be 1 to 40 characters".into());
        }
        if a.publisher.trim().is_empty() || a.publisher.chars().count() > 60 {
            return bad("addon.publisher must be 1 to 60 characters".into());
        }
        if a.description.chars().count() > 600 {
            return bad("addon.description is longer than 600 characters".into());
        }
        if Version::parse(&a.version).is_none() {
            return bad(format!(
                "addon.version {:?} must look like 1.2.3",
                a.version
            ));
        }
        if Requirement::parse(&a.requires_app).is_none() {
            return bad(format!(
                "addon.requires_app {:?} must be versions joined by commas, such as >=0.1.0, <0.3",
                a.requires_app
            ));
        }
        let mut seen = BTreeSet::new();
        for p in &a.permissions {
            if !seen.insert(*p) {
                return bad(format!("permission {} is listed twice", p.key()));
            }
        }

        for (path, hash) in &self.files {
            check_path(path)?;
            if path == "manifest.toml" || path == "signature" {
                return bad(format!("files must not list {path}"));
            }
            if hash.len() != 64
                || !hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return bad(format!(
                    "the hash of {path} must be 64 lower-case hex digits"
                ));
            }
        }
        let listed = |p: &str, what: &str| -> Result<()> {
            check_path(p)?;
            if !self.files.contains_key(p) {
                return bad(format!("{what} {p} is not listed under [files]"));
            }
            Ok(())
        };

        if let Some(e) = &self.engine {
            listed(&e.file, "the engine file")?;
            if !e.file.ends_with(".wasm") {
                return bad("engine.file must be a .wasm file".into());
            }
            if e.host_api != HOST_API_VERSION {
                return Err(AddonError::Incompatible(format!(
                    "the engine needs host API {}; this KoperCAD provides host API {HOST_API_VERSION}",
                    e.host_api
                )));
            }
        }

        let mut panel_ids = BTreeSet::new();
        for p in &self.panels {
            if !valid_id(&p.id, 1, 32) {
                return bad(format!(
                    "panel id {:?} must be lower-case letters, digits and hyphens",
                    p.id
                ));
            }
            if !panel_ids.insert(p.id.as_str()) {
                return bad(format!("panel {} is declared twice", p.id));
            }
            if p.title.trim().is_empty() || p.title.chars().count() > 40 {
                return bad(format!(
                    "panel {} needs a title of 1 to 40 characters",
                    p.id
                ));
            }
            if !self.allows(p.slot.permission()) {
                return bad(format!(
                    "panel {} needs the permission {}, which is not declared",
                    p.id,
                    p.slot.permission().key()
                ));
            }
            listed(&p.file, "the panel layout")?;
        }

        let mut command_ids = BTreeSet::new();
        let mut per_area: BTreeMap<&str, usize> = BTreeMap::new();
        let mut per_workspace: BTreeMap<&str, usize> = BTreeMap::new();
        for c in &self.commands {
            if !valid_id(&c.id, 1, 32) {
                return bad(format!(
                    "command id {:?} must be lower-case letters, digits and hyphens",
                    c.id
                ));
            }
            if !command_ids.insert(c.id.as_str()) {
                return bad(format!("command {} is declared twice", c.id));
            }
            let n = c.label.chars().count();
            if n == 0 || n > MAX_LABEL {
                return bad(format!(
                    "the label of command {} must be 1 to {MAX_LABEL} characters (it has {n}); the longer sentence belongs in description",
                    c.id
                ));
            }
            if c.description.chars().count() > 300 || c.keywords.len() > 12 {
                return bad(format!(
                    "command {} has a description over 300 characters or more than 12 keywords",
                    c.id
                ));
            }
            if !c.icon.is_empty() && !valid_icon_name(&c.icon) {
                return bad(format!(
                    "the icon name {:?} of command {} is not a plain name",
                    c.icon, c.id
                ));
            }
            if c.key
                .as_deref()
                .is_some_and(|k| k.len() > 24 || k.chars().any(char::is_control))
            {
                return bad(format!(
                    "the key of command {} is not a plain key name",
                    c.id
                ));
            }
            match c.home {
                HomeKind::Menu => {
                    if !self.allows(Permission::Menu) {
                        return bad(format!(
                            "command {} sits in a menu but the permission menu is not declared",
                            c.id
                        ));
                    }
                    match c.menu.as_deref() {
                        Some(m) if MENU_AREAS.contains(&m) => {
                            *per_area.entry(m).or_default() += 1;
                        }
                        other => {
                            return bad(format!(
                                "command {}: menu must be one of {} (got {other:?})",
                                c.id,
                                MENU_AREAS.join(", ")
                            ));
                        }
                    }
                }
                HomeKind::Rail => {
                    if !self.allows(Permission::RailItem) {
                        return bad(format!(
                            "command {} sits on a rail but the permission rail-item is not declared",
                            c.id
                        ));
                    }
                    if c.workspaces.is_empty()
                        || c.workspaces
                            .iter()
                            .any(|w| !WORKSPACES.contains(&w.as_str()))
                    {
                        return bad(format!(
                            "command {}: a rail row needs workspaces from {}",
                            c.id,
                            WORKSPACES.join(", ")
                        ));
                    }
                    let unique: BTreeSet<&str> = c.workspaces.iter().map(String::as_str).collect();
                    for w in unique {
                        *per_workspace.entry(w).or_default() += 1;
                    }
                }
                HomeKind::PaletteOnly => {}
            }
            match &c.opens {
                Some(panel) if !panel_ids.contains(panel.as_str()) => {
                    return bad(format!(
                        "command {} opens panel {panel}, which is not declared",
                        c.id
                    ));
                }
                None if self.engine.is_none() => {
                    return bad(format!(
                        "command {} opens no panel and the add-on has no engine to handle it",
                        c.id
                    ));
                }
                _ => {}
            }
        }
        for (area, n) in per_area {
            if n > MAX_MENU_ROWS_PER_AREA {
                return bad(format!(
                    "an add-on may add at most {MAX_MENU_ROWS_PER_AREA} rows to one menu area ({area} has {n}); the rest can be palette-only"
                ));
            }
        }
        for (ws, n) in per_workspace {
            if n > MAX_RAIL_ROWS_PER_WORKSPACE {
                return bad(format!(
                    "an add-on may add at most {MAX_RAIL_ROWS_PER_WORKSPACE} rows to one rail flyout ({ws} has {n}); the rest can be palette-only"
                ));
            }
        }

        for c in &self.contributions {
            if !valid_id(&c.kind, 1, 40) {
                return bad(format!(
                    "contribution kind {:?} is not a plain name",
                    c.kind
                ));
            }
            listed(&c.file, "the contributed file")?;
        }
        Ok(())
    }
}

/// `[a-z][a-z0-9-]*` within `min..=max` characters, no double or trailing hyphen.
pub fn valid_id(s: &str, min: usize, max: usize) -> bool {
    let b = s.as_bytes();
    (min..=max).contains(&b.len())
        && b[0].is_ascii_lowercase()
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
        && !s.ends_with('-')
        && !s.contains("--")
}

/// An icon name: a plain id, or `<add-on id>.<name>`.
pub fn valid_icon_name(s: &str) -> bool {
    match s.split_once('.') {
        None => valid_id(s, 1, 40),
        Some((addon, name)) => valid_id(addon, 2, 48) && valid_id(name, 1, 40),
    }
}

const RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// A package path that is safe on every platform: relative, `/` separated, no `..`, no drive
/// letters, no alternate streams, no Windows device names, no trailing dot or space.
pub fn check_path(path: &str) -> Result<()> {
    let bad = |why: &str| Err(AddonError::UnsafePath(format!("{path:?}: {why}")));
    if path.is_empty() || path.len() > 200 {
        return bad("empty or too long");
    }
    if path.starts_with('/') || path.contains('\\') || path.contains(':') {
        return bad("must be a relative path with / separators");
    }
    if path
        .chars()
        .any(|c| c.is_control() || "<>\"|?*".contains(c))
    {
        return bad("contains a character Windows cannot hold");
    }
    for part in path.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return bad("has an empty, . or .. part");
        }
        if part.ends_with('.') || part.ends_with(' ') {
            return bad("a part ends in a dot or space");
        }
        let stem = part.split('.').next().unwrap_or("").to_ascii_lowercase();
        if RESERVED.contains(&stem.as_str()) {
            return bad("uses a Windows device name");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    fn good() -> String {
        format!(
            r#"
[addon]
id = "demo"
name = "Demo"
version = "1.0.0"
publisher = "Tester"
api_version = 1
permissions = ["menu", "floating-panel"]

[[panel]]
id = "main"
title = "Main"
slot = "floating"
file = "data/main.panel.toml"

[[command]]
id = "open"
label = "Open demo"
home = "menu"
menu = "Simulate"
opens = "main"

[files]
"data/main.panel.toml" = "{ZERO}"
"#
        )
    }

    #[test]
    fn a_good_manifest_parses() {
        let m = Manifest::parse(good().as_bytes()).unwrap();
        assert_eq!(m.command_id("open"), "addon.demo.open");
        assert_eq!(m.version(), Version::new(1, 0, 0));
        assert!(m.allows(Permission::Menu));
        assert!(!m.allows(Permission::AddChecks));
    }

    fn rejected(from: &str, to: &str) -> String {
        let text = good();
        assert!(
            text.contains(from),
            "test bug: {from:?} not in the manifest"
        );
        match Manifest::parse(text.replace(from, to).as_bytes()) {
            Ok(_) => panic!("accepted a manifest with {from:?} -> {to:?}"),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn each_rule_has_a_message() {
        assert!(rejected("id = \"demo\"", "id = \"Demo\"").contains("addon.id"));
        assert!(rejected("id = \"demo\"", "id = \"../x\"").contains("addon.id"));
        assert!(rejected("version = \"1.0.0\"", "version = \"one\"").contains("addon.version"));
        assert!(
            rejected(
                "label = \"Open demo\"",
                "label = \"This label is much longer than twenty-eight\""
            )
            .contains("28")
        );
        assert!(
            rejected("menu = \"Simulate\"", "menu = \"Nowhere\"").contains("menu must be one of")
        );
        assert!(
            rejected(
                "permissions = [\"menu\", \"floating-panel\"]",
                "permissions = [\"floating-panel\"]"
            )
            .contains("permission menu")
        );
        assert!(
            rejected(
                "permissions = [\"menu\", \"floating-panel\"]",
                "permissions = [\"menu\"]"
            )
            .contains("floating-panel")
        );
        assert!(rejected("opens = \"main\"", "opens = \"gone\"").contains("not declared"));
        assert!(rejected("opens = \"main\"\n", "").contains("no engine"));
        assert!(
            rejected(
                "file = \"data/main.panel.toml\"",
                "file = \"data/other.toml\""
            )
            .contains("not listed")
        );
        assert!(
            rejected("\"data/main.panel.toml\" =", "\"../evil\" =").contains("unsafe file name")
        );
        assert!(rejected(ZERO, "xyz").contains("hex"));
        assert!(
            rejected(
                "permissions = [\"menu\", \"floating-panel\"]",
                "permissions = [\"menu\", \"root\"]"
            )
            .contains("root")
        );
        assert!(rejected("[addon]\n", "[addon]\nsurprise = 1\n").contains("surprise"));
    }

    #[test]
    fn another_api_generation_is_named_before_anything_else() {
        let text = good()
            .replace("api_version = 1", "api_version = 2")
            .replace(
                "[files]",
                "[files]\nnew_field = 3\n[brand_new_table]\nx = 1\n",
            );
        match Manifest::parse(text.as_bytes()) {
            Err(AddonError::Incompatible(m)) => assert!(m.contains("API 2"), "{m}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn paths_that_escape_or_break_windows_are_refused() {
        for p in [
            "../x",
            "a/../b",
            "/abs",
            "C:/x",
            "a\\b",
            "a//b",
            "./a",
            "a/.",
            "CON",
            "data/nul.txt",
            "a/b ",
            "a.",
            "a:b",
            "ads.txt:stream",
            "x\u{0}y",
            "a/<b>",
            "",
        ] {
            assert!(check_path(p).is_err(), "{p:?}");
        }
        for p in [
            "manifest.toml",
            "data/a-b_c.d.toml",
            "LICENSE",
            "x/y/z.wasm",
        ] {
            assert!(check_path(p).is_ok(), "{p:?}");
        }
    }

    #[test]
    fn a_rail_row_needs_workspaces_and_the_permission() {
        let text = good()
            .replace(
                "home = \"menu\"\nmenu = \"Simulate\"",
                "home = \"rail\"\nworkspaces = [\"nowhere\"]",
            )
            .replace("\"menu\", ", "\"rail-item\", ");
        assert!(
            Manifest::parse(text.as_bytes())
                .unwrap_err()
                .to_string()
                .contains("workspaces")
        );
        let ok = text.replace("nowhere", "3d");
        assert!(Manifest::parse(ok.as_bytes()).is_ok());
        let no_permission = ok.replace("\"rail-item\", ", "\"menu\", ");
        assert!(
            Manifest::parse(no_permission.as_bytes())
                .unwrap_err()
                .to_string()
                .contains("rail-item")
        );
    }

    #[test]
    fn row_budgets_per_area_and_workspace_hold() {
        let mut text = good();
        for i in 0..4 {
            text.push_str(&format!(
                "\n[[command]]\nid = \"extra{i}\"\nlabel = \"Extra {i}\"\nhome = \"menu\"\nmenu = \"Simulate\"\nopens = \"main\"\n"
            ));
        }
        // 1 + 4 rows in Simulate
        assert!(
            Manifest::parse(text.as_bytes())
                .unwrap_err()
                .to_string()
                .contains("at most 4")
        );
    }

    #[test]
    fn icon_names_are_plain_or_namespaced() {
        for ok in ["play", "controllers.radio", "din-rail"] {
            assert!(valid_icon_name(ok), "{ok}");
        }
        for bad in ["", "A", "a.b.c", "../x", "x.", ".x", "x y"] {
            assert!(!valid_icon_name(bad), "{bad}");
        }
    }
}

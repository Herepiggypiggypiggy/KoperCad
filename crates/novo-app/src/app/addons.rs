//! Add-ons: the Manage add-ons window (File > Add-ons..., Ctrl+K "Add-ons"), the rows add-ons
//! add to the menus and the command search, and the panels they declare.
//!
//! The rules are in `docs/design/visual-redesign-addons.md`: an add-on adds rows to tables the
//! program owns and never a surface of its own. This file draws the manager and hosts declared
//! panels with the program's widgets and `panel.rs`; everything about packages, trust and
//! installing is `novo-addons`, which is pure and tested without a window.
//!
//! Nothing here knows any particular add-on. With none installed the program shows one extra
//! command ("Add-ons") and nothing else.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use eframe::egui;
use novo_addons::layout::{PanelLayout, Role, Widget};
use novo_addons::manifest::Slot;
use novo_addons::store::{Change, Compat};
use novo_addons::{
    AddonSet, AppInfo, Entry, ExtCommand, Extensions, Inspection, InstallOptions, Status, Store,
    Trust, Version,
};
use novo_addons::{CallError, Done, EngineWorker, HostLimits};
use novo_theme::widgets::{self, EmptySize, Level as Tone};
use novo_theme::{icons, space};
use novo_workflow::keymap::Chord;
use novo_workflow::palette::AddonEntry;
use serde_json::{Value, json};

use super::{Level, NovoApp};

/// Everything the program keeps about add-ons while it runs.
#[derive(Default)]
pub(super) struct AddonsState {
    /// `None` when there is no profile folder to keep add-ons in (and in unit tests).
    pub(super) store: Option<Store>,
    /// What was installed and its checks, as of the last scan.
    pub(super) set: AddonSet,
    /// What the running add-ons add to the program's tables.
    pub(super) ext: Extensions,
    pub(super) manager: Option<Manager>,
    /// Floating panels the person opened, by `addon.<id>.<panel>`.
    pub(super) open_panels: BTreeSet<String>,
    /// Live view cards the person closed this run.
    pub(super) closed_cards: BTreeSet<String>,
    /// Engines that are running, by add-on id (started when first needed).
    engines: BTreeMap<String, EngineWorker>,
    /// What each request in flight was for, by add-on id and ticket.
    pub(super) in_flight: BTreeMap<(String, u64), Origin>,
    /// The window, so an engine thread can ask for a frame when it has answered.
    wake: Option<egui::Context>,
    /// Add-ons whose engine stopped or would not start this run, with the reason in words. They
    /// add nothing until the person turns them off and on again.
    pub(super) stopped: BTreeMap<String, String>,
    /// What engines last said about lamps: add-on id and lamp id to its state.
    pub(super) lamps: BTreeMap<(String, String), (Option<Role>, Option<bool>)>,
}

/// What a request to an engine was for: where its answer goes when it arrives.
#[derive(Clone, Copy)]
pub(super) enum Origin {
    Command,
    Button,
    /// A test waiting for the reply itself.
    #[cfg(test)]
    Test,
}

/// The manager window's own state.
#[derive(Default)]
pub(super) struct Manager {
    selected: Option<String>,
    filter: String,
    pub(super) prompt: Option<Prompt>,
    confirm_remove: Option<String>,
    pub(super) notice: Option<(Tone, String)>,
}

/// The install prompt: what the file is and what it asks for.
pub(super) struct Prompt {
    path: PathBuf,
    pub(super) inspection: Inspection,
    tick: bool,
}

/// The folder add-ons live in: `%LOCALAPPDATA%\KoperCAD\addons`.
pub(crate) fn addons_dir() -> Option<PathBuf> {
    novo_storage::profile::data_local_dir().map(|d| d.join(crate::DATA_DIR_NAME).join("addons"))
}

/// The program as add-ons see it.
fn app_info() -> AppInfo {
    AppInfo {
        version: Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_default(),
        api_version: novo_addons::API_VERSION,
    }
}

impl AddonsState {
    /// Palette rows for the running add-ons' commands.
    pub(super) fn palette_entries(&self) -> Vec<AddonEntry> {
        self.ext
            .commands
            .iter()
            .map(|c| AddonEntry {
                id: c.id.clone(),
                addon: c.addon_name.clone(),
                label: c.label.clone(),
                description: c.description.clone(),
                words: c.keywords.join(" ").to_lowercase(),
            })
            .collect()
    }
}

impl NovoApp {
    /// Reads the add-on folder of this profile. Called once at start.
    pub(super) fn init_addons(&mut self) {
        if let Some(root) = addons_dir() {
            self.init_addons_at(root);
        }
    }

    /// The same with a folder of the caller's choice (tests and scripts).
    pub fn init_addons_at(&mut self, root: PathBuf) {
        self.wb.addons.store = Some(Store::new(root, app_info()));
        self.refresh_addons();
    }

    /// Scans the folder again and rebuilds what add-ons add to the tables.
    pub(super) fn refresh_addons(&mut self) {
        let Some(store) = &self.wb.addons.store else {
            return;
        };
        let set = store.scan();
        let keymap = &self.wb.keymap;
        let in_use = |key: &str| {
            Chord::parse(key).is_some_and(|c| keymap.bindings().iter().any(|b| b.chord == c))
        };
        let stopped = &self.wb.addons.stopped;
        let ext = Extensions::build_excluding(&set, &in_use, &|id| stopped.contains_key(id));
        // Their icons and short labels join the tables the rail, menus and Ctrl K read.
        super::addons_look::sync(&ext);
        for warning in &set.warnings {
            log::warn!("add-ons: {warning}");
        }
        self.wb.addons.set = set;
        self.wb.addons.ext = ext;
        // Engines start again, from the files as they are now, the next time they are needed.
        self.wb
            .addons
            .engines
            .retain(|id, _| self.wb.addons.set.is_enabled(id));
        let engines = &self.wb.addons.engines;
        self.wb
            .addons
            .in_flight
            .retain(|(id, _), _| engines.contains_key(id));
        // A panel of an add-on that is gone is not open any more.
        let ext = &self.wb.addons.ext;
        self.wb
            .addons
            .open_panels
            .retain(|p| ext.panel(p).is_some());
    }

    /// Queues one request for the engine of add-on `id` on its own thread, starting the thread
    /// first when needed, and returns at once; the answer is applied by [`Self::poll_engines`]
    /// on a later frame. `None` when the add-on has no engine. A start that fails stops the
    /// add-on for this run and says why in Details.
    pub(super) fn addon_engine_send(
        &mut self,
        id: &str,
        op: &str,
        data: &Value,
        origin: Origin,
    ) -> Option<Result<u64, String>> {
        if let Some(why) = self.wb.addons.stopped.get(id) {
            return Some(Err(why.clone()));
        }
        if !self.wb.addons.engines.contains_key(id) {
            match self.wb.addons.set.engine_wasm(id) {
                Ok(None) => return None,
                Ok(Some(wasm)) => {
                    let ctx = self.wb.addons.wake.clone();
                    let worker = EngineWorker::start(wasm, HostLimits::default(), move || {
                        if let Some(ctx) = &ctx {
                            ctx.request_repaint();
                        }
                    });
                    self.wb.addons.engines.insert(id.to_owned(), worker);
                }
                Err(e) => {
                    self.stop_addon(id, e.to_string());
                    return Some(Err(e.to_string()));
                }
            }
        }
        let worker = self.wb.addons.engines.get_mut(id)?;
        let ticket = worker.submit(op, data);
        self.wb
            .addons
            .in_flight
            .insert((id.to_owned(), ticket), origin);
        Some(Ok(ticket))
    }

    /// Applies the answers engine threads have sent since the last frame. Never waits.
    pub(super) fn poll_engines(&mut self) {
        let ids: Vec<String> = self.wb.addons.engines.keys().cloned().collect();
        for id in ids {
            while let Some(done) = self
                .wb
                .addons
                .engines
                .get_mut(&id)
                .and_then(EngineWorker::try_next)
            {
                self.engine_answered(&id, done);
            }
        }
    }

    /// One answer: a stop is shown, a refusal is the engine's own words, data is applied.
    fn engine_answered(&mut self, id: &str, done: Done) -> Option<Result<Value, String>> {
        let origin = self
            .wb
            .addons
            .in_flight
            .remove(&(id.to_owned(), done.ticket));
        let result = match done.result {
            Ok(v) => Ok(v),
            Err(CallError::Refused(why)) => Err(why),
            Err(CallError::Stopped(why)) => {
                // Answers queued behind a stop find the add-on already stopped.
                if !self.wb.addons.stopped.contains_key(id) {
                    self.stop_addon(id, why.clone());
                }
                Err(why)
            }
        };
        match (&origin, &result) {
            (Some(Origin::Command | Origin::Button), Ok(reply)) => {
                self.apply_engine_reply(id, reply);
            }
            // A refusal is the engine's own words; a stop was already shown.
            (Some(Origin::Command | Origin::Button), Err(why))
                if !self.wb.addons.stopped.contains_key(id) =>
            {
                self.message(Level::Info, why.clone());
            }
            _ => {}
        }
        Some(result)
    }

    /// Sends one request and waits for its answer, applying the answers before it on the way.
    /// For tests and scripts that need the reply in hand; the window never calls it.
    #[cfg(test)]
    pub(super) fn addon_engine_call(
        &mut self,
        id: &str,
        op: &str,
        data: &Value,
    ) -> Option<Result<Value, String>> {
        let wanted = match self.addon_engine_send(id, op, data, Origin::Test)? {
            Ok(ticket) => ticket,
            Err(why) => return Some(Err(why)),
        };
        loop {
            let done = self.wb.addons.engines.get_mut(id)?.wait_next()?;
            let last = done.ticket == wanted;
            let result = self.engine_answered(id, done);
            if last {
                return result;
            }
        }
    }

    fn stop_addon(&mut self, id: &str, why: String) {
        log::warn!("add-on {id} stopped: {why}");
        let name = self.addon_name(id);
        self.wb.addons.engines.remove(id);
        self.wb.addons.in_flight.retain(|(i, _), _| i != id);
        self.wb.addons.stopped.insert(id.to_owned(), why.clone());
        self.refresh_addons();
        self.message(
            Level::Warning,
            format!("{name} stopped: {why}. Turn it off and on in Add-ons to start it again."),
        );
    }

    /// Applies what an engine answered: lamp states and a notice.
    pub(super) fn apply_engine_reply(&mut self, id: &str, reply: &Value) {
        if let Some(lamps) = reply.get("lamps").and_then(Value::as_object) {
            // Only lamps one of the add-on's own panels declares are kept, so an engine cannot
            // grow the table with ids nothing shows.
            let mut declared: Vec<String> = Vec::new();
            for panel in self
                .wb
                .addons
                .ext
                .panels
                .iter()
                .filter(|p| p.addon_id == id)
            {
                if let Ok(layout) = self
                    .wb
                    .addons
                    .set
                    .read_file(id, &panel.file)
                    .map_err(|e| e.to_string())
                    .and_then(|b| PanelLayout::parse(&b).map_err(|e| e.to_string()))
                {
                    declared.extend(layout.ids().into_iter().map(str::to_owned));
                }
            }
            for (lamp, v) in lamps.iter().filter(|(l, _)| declared.contains(l)).take(200) {
                let state = v
                    .get("state")
                    .and_then(|s| serde_json::from_value::<Role>(s.clone()).ok());
                let lit = v.get("lit").and_then(Value::as_bool);
                self.wb
                    .addons
                    .lamps
                    .insert((id.to_owned(), lamp.clone()), (state, lit));
            }
        }
        if let Some(text) = reply.get("notice").and_then(Value::as_str) {
            self.message(Level::Info, text.chars().take(200).collect::<String>());
        }
    }

    /// Whether add-on `id` is installed, intact, compatible and on. The gate the rest of the
    /// program asks before it offers anything that belongs to an add-on.
    pub fn addon_enabled(&self, id: &str) -> bool {
        self.wb.addons.set.is_enabled(id)
    }

    /// What each running engine has used so far, for Details.
    pub(super) fn engine_load(&self) -> EngineLoad {
        self.wb
            .addons
            .engines
            .iter()
            .filter_map(|(id, w)| {
                let s = w.stats()?;
                Some((id.clone(), (s.calls, s.peak_fuel, s.fuel_budget)))
            })
            .collect()
    }

    /// Opens the manager (File > Add-ons..., Ctrl+K).
    pub(super) fn open_addons(&mut self) {
        self.dismiss_toasts();
        let selected = self.wb.addons.set.entries.first().map(|e| e.id.clone());
        self.wb.addons.manager.get_or_insert_with(|| Manager {
            selected,
            ..Manager::default()
        });
    }

    /// Runs a command an add-on added: opens its panel, or hands it to the add-on's engine.
    pub(super) fn run_addon_command(&mut self, id: &str) {
        let Some(cmd) = self.wb.addons.ext.command(id).cloned() else {
            self.message(Level::Info, "That add-on command is not available any more");
            return;
        };
        match &cmd.opens {
            Some(panel) => {
                if self.wb.addons.ext.panel(panel).is_some() {
                    let live =
                        self.wb.addons.ext.panel(panel).map(|p| p.slot) == Some(Slot::LiveViewCard);
                    if live {
                        self.wb.addons.closed_cards.remove(panel);
                        if !self.wb.live.on {
                            self.message(
                                Level::Info,
                                format!("{} shows in the live view (Shift+L)", cmd.label),
                            );
                        }
                    } else if !self.wb.addons.open_panels.remove(panel) {
                        self.wb.addons.open_panels.insert(panel.clone());
                    }
                }
            }
            None => {
                let local = cmd
                    .id
                    .strip_prefix(&format!("addon.{}.", cmd.addon_id))
                    .unwrap_or(&cmd.id)
                    .to_owned();
                let sent = self.addon_engine_send(
                    &cmd.addon_id,
                    "command",
                    &json!({ "id": local }),
                    Origin::Command,
                );
                match sent {
                    Some(Ok(_)) => {}
                    Some(Err(why)) => {
                        // The add-on is stopped already; Details says why.
                        if !self.wb.addons.stopped.contains_key(&cmd.addon_id) {
                            self.message(Level::Info, why);
                        }
                    }
                    None => self.message(
                        Level::Info,
                        format!("{} has no engine to run this command", cmd.addon_name),
                    ),
                }
            }
        }
    }

    /// The rows at the foot of a menu area, under the label "Add-ons".
    pub(super) fn addon_menu_rows(&mut self, ui: &mut egui::Ui, area: &str) {
        let rows: Vec<ExtCommand> = self
            .wb
            .addons
            .ext
            .menu_rows(area)
            .into_iter()
            .cloned()
            .collect();
        if rows.is_empty() {
            return;
        }
        ui.separator();
        widgets::section_label(ui, "Add-ons");
        for cmd in rows {
            let icon = self.addon_icon_text(&cmd);
            let mut button = egui::Button::new(format!("{icon}  {}", cmd.label));
            if let Some(key) = cmd.key.as_deref().filter(|_| !cmd.key_conflict) {
                button = button.shortcut_text(key);
            }
            let response = ui.add(button);
            let response = if cmd.description.is_empty() {
                response
            } else {
                response.on_hover_text(&cmd.description)
            };
            if response.clicked() {
                self.run_addon_command(&cmd.id);
                ui.close();
            }
        }
    }

    /// The glyph for an add-on command: the add-on glyph until the drawn icon engine (lane V0)
    /// can paint add-on shapes.
    fn addon_icon_text(&self, _cmd: &ExtCommand) -> String {
        icons::ADDON_ICON.to_owned()
    }

    // ------------------------------------------------------------------
    // The window

    /// Draws the manager and the panels add-ons declared. Called every frame.
    pub(super) fn addons_windows(&mut self, ctx: &egui::Context) {
        self.wb.addons.wake.get_or_insert_with(|| ctx.clone());
        self.poll_engines();
        self.addon_panels(ctx);
        if self.wb.addons.manager.is_none() {
            return;
        }
        // A .kcaddon dropped on the window starts the install, wherever it lands.
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| {
                    p.extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("kcaddon"))
                })
                .collect()
        });
        if let Some(path) = dropped.into_iter().next() {
            self.begin_install(path);
        }

        let mut manager = self.wb.addons.manager.take().expect("checked above");
        let mut open = true;
        let mut actions: Vec<Act> = Vec::new();
        let set = self.wb.addons.set.clone();
        let root = self
            .wb
            .addons
            .store
            .as_ref()
            .map(|s| s.root().to_path_buf());
        let can_store = self.wb.addons.store.is_some();
        let stopped = self.wb.addons.stopped.clone();
        let load = self.engine_load();
        super::panel::dialog(
            ctx,
            format!("{} Add-ons", icons::ADDON_ICON),
            [960.0, 640.0],
        )
        .id(super::panel::id("addons"))
        .open(super::panel::esc(ctx, &mut open))
        .show(ctx, |ui| {
            manager_ui(
                ui,
                &mut manager,
                &set,
                root.as_deref(),
                can_store,
                &Engines {
                    stopped: &stopped,
                    load: &load,
                },
                &mut actions,
            );
        });
        self.prompt_ui(ctx, &mut manager, &mut actions);
        if !open && manager.prompt.is_none() {
            // closed with the window's x or Escape
        } else if !open {
            open = true;
        }
        if open {
            self.wb.addons.manager = Some(manager);
        }
        for act in actions {
            self.addons_act(act);
        }
    }

    fn addons_act(&mut self, act: Act) {
        let Some(store) = self.wb.addons.store.clone() else {
            return;
        };
        // The add-on a successful install should show in the details pane.
        let mut shown: Option<String> = None;
        let result: Result<String, String> = match act {
            Act::Pick(update_of) => {
                if let Some(path) = pick_package() {
                    self.begin_install_for(path, update_of);
                }
                return;
            }
            Act::Select(id) => {
                if let Some(m) = &mut self.wb.addons.manager {
                    m.selected = Some(id);
                    m.notice = None;
                }
                return;
            }
            Act::Install { path, accept } => {
                let options = InstallOptions {
                    accept_unsigned: accept,
                    enable: true,
                };
                store
                    .install_file(&path, options)
                    .map(|i| {
                        let m = &i.package.manifest.addon;
                        shown = Some(m.id.clone());
                        match i.change {
                            Change::Update { from } => {
                                format!("{} updated from {from} to {}", m.name, m.version)
                            }
                            Change::Repair => format!("{} {} repaired", m.name, m.version),
                            _ => format!("{} {} installed and turned on", m.name, m.version),
                        }
                    })
                    .map_err(|e| e.to_string())
            }
            Act::Enable(id, on) => store
                .set_enabled(&id, on)
                .map(|()| {
                    let name = self.addon_name(&id);
                    format!("{name} turned {}", if on { "on" } else { "off" })
                })
                .map_err(|e| e.to_string()),
            Act::Remove(id) => {
                let name = self.addon_name(&id);
                store
                    .remove(&id)
                    .map(|()| format!("{name} removed. Projects keep the data they stored for it."))
                    .map_err(|e| e.to_string())
            }
            Act::RollBack(id) => {
                let name = self.addon_name(&id);
                store
                    .roll_back(&id)
                    .map(|v| format!("{name} is back at {v}"))
                    .map_err(|e| e.to_string())
            }
            Act::OpenFolder => {
                if let Some(dir) = store.root().to_str() {
                    let _ = std::fs::create_dir_all(dir);
                    let _ = open_folder(store.root());
                }
                return;
            }
        };
        // Any change in the manager is a fresh start for engines that stopped.
        self.wb.addons.stopped.clear();
        self.wb.addons.lamps.clear();
        self.refresh_addons();
        if let Some(m) = &mut self.wb.addons.manager {
            m.prompt = None;
            m.confirm_remove = None;
            if shown.is_some() {
                m.selected = shown;
            }
            match result {
                Ok(text) => m.notice = Some((Tone::Success, text)),
                Err(text) => m.notice = Some((Tone::Error, text)),
            }
        }
    }

    fn addon_name(&self, id: &str) -> String {
        self.wb
            .addons
            .set
            .get(id)
            .map_or_else(|| id.to_owned(), |e| e.display_name().to_owned())
    }

    /// Reads a package and shows the install prompt.
    pub fn begin_install(&mut self, path: PathBuf) {
        self.begin_install_for(path, None);
    }

    fn begin_install_for(&mut self, path: PathBuf, update_of: Option<String>) {
        let Some(store) = self.wb.addons.store.clone() else {
            return;
        };
        self.open_addons();
        let outcome = store.inspect_file(&path);
        let Some(manager) = &mut self.wb.addons.manager else {
            return;
        };
        match outcome {
            Err(e) => manager.notice = Some((Tone::Error, e.to_string())),
            Ok(inspection) => {
                if let Some(want) = update_of
                    && inspection.package.manifest.addon.id != want
                {
                    manager.notice = Some((
                        Tone::Error,
                        format!(
                            "That file is {}, not an update of {want}",
                            inspection.package.manifest.addon.name
                        ),
                    ));
                    return;
                }
                if let Some(why) = inspection.blocked() {
                    manager.notice = Some((Tone::Warning, why));
                    return;
                }
                manager.notice = None;
                manager.prompt = Some(Prompt {
                    path,
                    inspection,
                    tick: false,
                });
            }
        }
    }

    fn prompt_ui(&mut self, ctx: &egui::Context, manager: &mut Manager, actions: &mut Vec<Act>) {
        let Some(prompt) = &mut manager.prompt else {
            return;
        };
        let mut cancel = false;
        let mut go = false;
        let title = install_prompt_title(prompt);
        // Escape and a click outside answer nothing: only the buttons do.
        let _ = super::panel::modal(
            ctx,
            "addon_install_prompt",
            title,
            [novo_theme::dialog_width::MEDIUM, 0.0],
            |ui| install_prompt_ui(ui, prompt, &mut cancel, &mut go),
        );
        if cancel {
            manager.prompt = None;
        } else if go {
            actions.push(Act::Install {
                path: prompt.path.clone(),
                accept: prompt.tick,
            });
        }
    }

    // ------------------------------------------------------------------
    // Declared panels

    fn addon_panels(&mut self, ctx: &egui::Context) {
        let panels: Vec<_> = self.wb.addons.ext.panels.clone();
        for panel in panels {
            let shown = match panel.slot {
                Slot::Floating => self.wb.addons.open_panels.contains(&panel.id),
                Slot::LiveViewCard => {
                    self.wb.live.on && !self.wb.addons.closed_cards.contains(&panel.id)
                }
            };
            if !shown {
                continue;
            }
            let layout = self
                .wb
                .addons
                .set
                .read_file(&panel.addon_id, &panel.file)
                .map_err(|e| e.to_string())
                .and_then(|b| PanelLayout::parse(&b).map_err(|e| e.to_string()));
            let mut open = true;
            let title = format!("{} {}", icons::ADDON_ICON, panel.title);
            let env = PanelEnv {
                has_engine: self
                    .wb
                    .addons
                    .set
                    .get(&panel.addon_id)
                    .and_then(|e| e.manifest.as_ref())
                    .is_some_and(|m| m.engine.is_some()),
                lamps: self
                    .wb
                    .addons
                    .lamps
                    .iter()
                    .filter(|((id, _), _)| id == &panel.addon_id)
                    .map(|((_, lamp), v)| (lamp.clone(), *v))
                    .collect(),
            };
            let mut clicked: Vec<String> = Vec::new();
            super::panel::dialog(ctx, title, [320.0, 0.0])
                .id(super::panel::id(&panel.id))
                .open(&mut open)
                .show(ctx, |ui| match &layout {
                    Ok(layout) => {
                        for w in &layout.widgets {
                            widget_ui(ui, w, &env, &mut clicked);
                        }
                        ui.add_space(space::S);
                        ui.weak(format!("From the add-on {}", panel.addon_name));
                    }
                    Err(why) => {
                        widgets::banner(ui, Tone::Error, why, |_| {});
                    }
                });
            let local = panel
                .id
                .strip_prefix(&format!("addon.{}.", panel.addon_id))
                .unwrap_or(&panel.id)
                .to_owned();
            for button in clicked {
                let sent = json!({ "panel": local, "id": button });
                if let Some(Err(why)) =
                    self.addon_engine_send(&panel.addon_id, "button", &sent, Origin::Button)
                    && !self.wb.addons.stopped.contains_key(&panel.addon_id)
                {
                    self.message(Level::Info, why);
                }
            }
            if !open {
                self.wb.addons.open_panels.remove(&panel.id);
                self.wb.addons.closed_cards.insert(panel.id.clone());
            }
        }
    }
}

/// What a panel needs to know about its add-on to draw: whether an engine can answer the
/// buttons, and the lamp states the engine last set.
struct PanelEnv {
    has_engine: bool,
    lamps: BTreeMap<String, (Option<Role>, Option<bool>)>,
}

// ----------------------------------------------------------------------
// Drawing

enum Act {
    /// Choose a file; `Some(id)` when it must be an update of that add-on.
    Pick(Option<String>),
    Select(String),
    Install {
        path: PathBuf,
        accept: bool,
    },
    Enable(String, bool),
    Remove(String),
    RollBack(String),
    OpenFolder,
}

fn pick_package() -> Option<PathBuf> {
    crate::dialogs::FileDialog::new()
        .set_title("Choose an add-on file")
        .add_filter("KoperCAD add-on", &["kcaddon"])
        .pick_file()
}

/// Opens a folder in the file manager of the platform, without elevation.
fn open_folder(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(windows)]
    let program = "explorer";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let program = "xdg-open";
    std::process::Command::new(program)
        .arg(path)
        .spawn()
        .map(|_| ())
}

/// The state shown on a row and in the details: a pill text, a tone and an icon.
fn status_look(status: &Status) -> (&'static str, Tone, &'static str) {
    match status {
        Status::Enabled => ("On", Tone::Success, icons::addon::ON),
        Status::Disabled => ("Off", Tone::Info, icons::addon::OFF),
        Status::Incompatible(_) => (
            "Needs another version",
            Tone::Warning,
            icons::addon::BLOCKED,
        ),
        Status::Failed(_) => ("Failed", Tone::Error, icons::addon::FAILED),
    }
}

fn trust_look(trust: &Trust, accepted: bool) -> (String, Tone, &'static str) {
    match trust {
        Trust::Signed { owner, .. } => (
            format!("Signed by {owner}"),
            Tone::Success,
            icons::addon::SIGNED,
        ),
        Trust::Unsigned { note } if accepted => (
            format!("Unsigned, from file (you accepted it): {note}"),
            Tone::Warning,
            icons::addon::UNSIGNED,
        ),
        Trust::Unsigned { note } => (
            format!("Unsigned, from file: {note}"),
            Tone::Warning,
            icons::addon::UNSIGNED,
        ),
    }
}

/// Per running engine: calls so far, most steps one call used, steps one call may use.
type EngineLoad = BTreeMap<String, (u64, u64, u64)>;

/// What the running engines look like to Details: why one stopped, and what each has used.
struct Engines<'a> {
    stopped: &'a BTreeMap<String, String>,
    load: &'a EngineLoad,
}

/// Id of the controller add-on (`addons/controllers`): the controller import wizard needs it.
pub(super) const CONTROLLERS_ADDON: &str = "controllers";

/// Why File > Import > Controller profile is greyed while the add-on is not on.
pub(super) const CONTROLLERS_NEEDED: &str =
    "Needs the Controllers add-on: install and turn it on in File > Add-ons...";

fn manager_ui(
    ui: &mut egui::Ui,
    m: &mut Manager,
    set: &AddonSet,
    root: Option<&std::path::Path>,
    can_store: bool,
    engines: &Engines,
    actions: &mut Vec<Act>,
) {
    let p = crate::theme::ui_palette();
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Add-ons")
                .heading()
                .color(p.text_primary),
        );
        ui.label(
            egui::RichText::new(
                "extra features you install from a file, without leaving the program",
            )
            .color(p.text_secondary),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(can_store, |ui| {
                if widgets::primary_button(
                    ui,
                    &format!("{} Install from file...", icons::addon::INSTALL),
                )
                .clicked()
                {
                    actions.push(Act::Pick(None));
                }
            });
        });
    });
    for warning in &set.warnings {
        widgets::banner(ui, Tone::Warning, warning, |_| {});
    }
    if let Some((tone, text)) = &m.notice {
        let text = text.clone();
        widgets::banner(ui, *tone, &text, |_| {});
    }
    ui.add_space(space::S);
    if !can_store {
        widgets::empty_state(
            ui,
            EmptySize::Panel,
            icons::ADDON_ICON,
            "Add-ons are not available here",
            "This session has no profile folder to keep them in.",
            |_| {},
        );
        return;
    }
    let full = ui.available_size();
    let left_width = 300.0_f32.min(full.x * 0.4);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(left_width, full.y),
            egui::Layout::top_down(egui::Align::Min),
            |ui| list_ui(ui, m, set, actions),
        );
        ui.separator();
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), full.y),
            egui::Layout::top_down(egui::Align::Min),
            |ui| match m.selected.as_ref().and_then(|id| set.get(id)) {
                Some(entry) => details_ui(ui, m, entry, root, engines.stopped.get(&entry.id).map(String::as_str), engines.load.get(&entry.id).copied(), actions),
                None if set.entries.is_empty() => widgets::empty_state(
                    ui,
                    EmptySize::Panel,
                    icons::ADDON_ICON,
                    "No add-ons installed",
                    "An add-on is a file ending in .kcaddon. Install one to add features; nothing is installed for you and nothing is sent anywhere.",
                    |ui| {
                        if widgets::primary_button(ui, "Install from file...").clicked() {
                            actions.push(Act::Pick(None));
                        }
                    },
                ),
                None => {
                    ui.weak("Choose an add-on on the left.");
                }
            },
        );
    });
}

fn list_ui(ui: &mut egui::Ui, m: &mut Manager, set: &AddonSet, actions: &mut Vec<Act>) {
    let p = crate::theme::ui_palette();
    ui.add(
        egui::TextEdit::singleline(&mut m.filter)
            .hint_text("Find an add-on")
            .desired_width(f32::INFINITY),
    );
    ui.add_space(space::S);
    let needle = m.filter.trim().to_lowercase();
    egui::ScrollArea::vertical()
        .id_salt("addon_list")
        .auto_shrink([false, false])
        .max_height((ui.available_height() - 64.0).max(80.0))
        .show(ui, |ui| {
            for entry in set.entries.iter().filter(|e| {
                needle.is_empty()
                    || e.display_name().to_lowercase().contains(&needle)
                    || e.id.contains(&needle)
            }) {
                let selected = m.selected.as_deref() == Some(entry.id.as_str());
                let (pill, tone, glyph) = status_look(&entry.status);
                let name = entry.display_name().to_owned();
                let response = widgets::row(ui, selected, |ui| {
                    ui.label(egui::RichText::new(icons::ADDON_ICON).color(p.text_secondary));
                    ui.label(egui::RichText::new(&name).color(p.text_primary));
                    ui.label(egui::RichText::new(entry.active.to_string()).color(p.text_tertiary));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!("{glyph} {pill}")).color(tone.color(p)),
                        );
                    });
                });
                if response.clicked() {
                    actions.push(Act::Select(entry.id.clone()));
                }
            }
        });
    ui.add_space(space::S);
    ui.weak("Drop a .kcaddon file on this window to install it.");
}

fn details_ui(
    ui: &mut egui::Ui,
    m: &mut Manager,
    entry: &Entry,
    root: Option<&std::path::Path>,
    stopped: Option<&str>,
    load: Option<(u64, u64, u64)>,
    actions: &mut Vec<Act>,
) {
    let p = crate::theme::ui_palette();
    egui::ScrollArea::vertical()
        .id_salt("addon_details")
        .auto_shrink([false, false])
        .max_height((ui.available_height() - 44.0).max(80.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(entry.display_name())
                        .heading()
                        .color(p.text_primary),
                );
                ui.label(egui::RichText::new(entry.active.to_string()).color(p.text_secondary));
                let on = entry.status.is_enabled();
                let can_toggle = matches!(entry.status, Status::Enabled | Status::Disabled);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Room for the scroll bar, so the label is never clipped at the minimum width.
                    ui.add_space(space::M);
                    ui.add_enabled_ui(can_toggle, |ui| {
                        let mut value = on;
                        if ui.checkbox(&mut value, "On").changed() {
                            actions.push(Act::Enable(entry.id.clone(), value));
                        }
                    });
                });
            });
            if let Some(manifest) = &entry.manifest {
                let a = &manifest.addon;
                ui.label(
                    egui::RichText::new(publisher_line(&a.publisher, entry.trust.is_signed()))
                        .color(p.text_secondary),
                );
                if !a.description.is_empty() {
                    ui.add_space(space::S);
                    ui.label(&a.description);
                }
            }
            ui.add_space(space::M);
            if let Some(why) = stopped {
                widgets::banner(
                    ui,
                    Tone::Error,
                    &format!("It stopped: {why}. Turn it off and on again to start it."),
                    |_| {},
                );
            }
            // A running engine: how much of its step budget its busiest call used, and a warning
            // when one slightly heavier call would stop it.
            if let (Some((calls, peak, budget)), None) = (load, stopped) {
                let (text, near) = novo_addons::budget_note(calls, peak, budget);
                if near {
                    widgets::banner(ui, Tone::Warning, &text, |_| {});
                } else {
                    ui.weak(text);
                }
            }
            match &entry.status {
                Status::Incompatible(why) => {
                    widgets::banner(ui, Tone::Warning, &format!("It is installed but off: {why}."), |_| {});
                }
                Status::Failed(why) => {
                    widgets::banner(ui, Tone::Error, &format!("It did not load: {why}."), |_| {});
                }
                _ => {}
            }
            let (trust_text, tone, glyph) = trust_look(&entry.trust, entry.accepted_unsigned);
            ui.label(egui::RichText::new(format!("{glyph} {trust_text}")).color(tone.color(p)));

            if let Some(manifest) = &entry.manifest {
                ui.add_space(space::M);
                widgets::section_label(ui, "What it may do");
                if manifest.addon.permissions.is_empty() {
                    ui.weak("Nothing beyond showing information. It asks for no permissions.");
                }
                for perm in &manifest.addon.permissions {
                    ui.label(format!("•  {}", perm.describe()));
                }
                if manifest.engine.is_some() {
                    ui.weak("It carries a program (engine.wasm). It runs in a sandbox: it cannot use files, the network or the clock.");
                }

                ui.add_space(space::M);
                widgets::section_label(ui, "What it adds");
                let mut any = false;
                for c in &manifest.commands {
                    any = true;
                    let home = match (c.home, c.menu.as_deref()) {
                        (novo_addons::manifest::HomeKind::Menu, Some(area)) => format!("{area} menu"),
                        (novo_addons::manifest::HomeKind::Rail, _) => {
                            format!("{} rail", c.workspaces.join(", "))
                        }
                        _ => "command search only".to_owned(),
                    };
                    let key = c
                        .key
                        .as_deref()
                        .map_or(String::new(), |k| format!(", key {k}"));
                    ui.label(format!("•  {}  ({home}{key})", c.label));
                }
                for panel in &manifest.panels {
                    any = true;
                    ui.label(format!("•  Panel: {}", panel.title));
                }
                for c in &manifest.contributions {
                    any = true;
                    ui.label(format!("•  Data: {} ({})", c.file, c.kind));
                }
                if !entry.icons.is_empty() {
                    any = true;
                    ui.label(format!("•  {} icon(s) of its own", entry.icons.len()));
                }
                if !any {
                    ui.weak("Nothing yet.");
                }

                ui.add_space(space::M);
                widgets::section_label(ui, "Files, checked at every start");
                fingerprint_row(ui, "Fingerprint", &entry.fingerprint);
                for (path, hash) in &manifest.files {
                    let ok = entry.verified.get(path).copied().unwrap_or(false);
                    ui.horizontal(|ui| {
                        let (mark, color) = if ok {
                            (icons::status::OK, p.success)
                        } else {
                            (icons::status::ERROR, p.error)
                        };
                        ui.label(egui::RichText::new(mark).color(color));
                        ui.label(path);
                        ui.label(egui::RichText::new(hash.chars().take(12).collect::<String>() + "…").color(p.text_tertiary));
                    });
                }
            }
            if let Some(root) = root {
                ui.add_space(space::M);
                ui.weak(format!("Installed in {}", root.display()));
            }
        });
    ui.add_space(space::S);
    ui.horizontal(|ui| {
        if ui.button("Update from file...").clicked() {
            actions.push(Act::Pick(Some(entry.id.clone())));
        }
        if let Some(prev) = entry.previous
            && ui.button(format!("Roll back to {prev}")).clicked()
        {
            actions.push(Act::RollBack(entry.id.clone()));
        }
        if ui.button("Show folder").clicked() {
            actions.push(Act::OpenFolder);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if m.confirm_remove.as_deref() == Some(entry.id.as_str()) {
                if widgets::danger_button(ui, "Remove for good").clicked() {
                    actions.push(Act::Remove(entry.id.clone()));
                }
                if ui.button("Keep it").clicked() {
                    m.confirm_remove = None;
                }
            } else if widgets::ghost_button(ui, "Remove").clicked() {
                m.confirm_remove = Some(entry.id.clone());
            }
        });
    });
}

fn short(hash: &str) -> String {
    hash.chars().take(16).collect::<String>() + "…"
}

/// The publisher is the add-on's own claim unless a trusted key signed the package.
fn publisher_line(publisher: &str, signed: bool) -> String {
    if signed {
        format!("by {publisher}")
    } else {
        format!("claims to be from {publisher}")
    }
}

/// A hash as its first 16 characters and a Copy button for the whole of it, so it never wraps.
fn fingerprint_row(ui: &mut egui::Ui, label: &str, hash: &str) {
    ui.horizontal(|ui| {
        ui.weak(format!("{label} {}", short(hash)));
        if widgets::ghost_button(ui, "Copy")
            .on_hover_text("Copy the whole value")
            .clicked()
        {
            ui.ctx().copy_text(hash.to_owned());
        }
    });
}

/// The title of the install prompt: what is about to happen to which add-on.
fn install_prompt_title(prompt: &Prompt) -> String {
    let a = &prompt.inspection.package.manifest.addon;
    match &prompt.inspection.change {
        Change::Update { from } => format!("Update {} from {from} to {}", a.name, a.version),
        Change::Repair => format!("Repair {} {}", a.name, a.version),
        _ => format!("Install {} {}", a.name, a.version),
    }
}

fn install_prompt_ui(ui: &mut egui::Ui, prompt: &mut Prompt, cancel: &mut bool, go: &mut bool) {
    let p = crate::theme::ui_palette();
    let package = &prompt.inspection.package;
    let a = &package.manifest.addon;
    ui.label(
        egui::RichText::new(publisher_line(
            &a.publisher,
            prompt.inspection.package.trust.is_signed(),
        ))
        .color(p.text_secondary),
    );
    if !a.description.is_empty() {
        ui.add_space(space::S);
        ui.label(&a.description);
    }
    ui.add_space(space::M);
    let (trust_text, tone, glyph) = trust_look(&package.trust, false);
    ui.label(egui::RichText::new(format!("{glyph} {trust_text}")).color(tone.color(p)));
    if let Compat::No(why) = &prompt.inspection.compat {
        widgets::banner(ui, Tone::Warning, why, |_| {});
    }
    ui.add_space(space::S);
    widgets::section_label(ui, "It asks to");
    if a.permissions.is_empty() {
        ui.weak("Nothing: it asks for no permissions.");
    }
    for perm in &a.permissions {
        ui.label(format!("•  {}", perm.describe()));
    }
    if package.manifest.engine.is_some() {
        ui.weak("It carries a program (engine.wasm). It runs in a sandbox: it cannot use files, the network or the clock.");
    }
    ui.add_space(space::S);
    fingerprint_row(ui, "Fingerprint", &package.fingerprint);
    fingerprint_row(ui, "File", &package.file_sha256);
    let needs_tick = prompt.inspection.needs_trust_tick();
    if needs_tick {
        ui.add_space(space::M);
        widgets::banner(
            ui,
            Tone::Warning,
            "Nobody you trust signed this file. Install it only if you know where it came from.",
            |_| {},
        );
        ui.checkbox(&mut prompt.tick, "I trust this file");
    }
    ui.add_space(space::L);
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(!needs_tick || prompt.tick, |ui| {
                if widgets::primary_button(ui, "Install").clicked() {
                    *go = true;
                }
            });
            if ui.button("Cancel").clicked() {
                *cancel = true;
            }
        });
    });
}

/// One widget of a declared panel, in the program's own look.
fn widget_ui(ui: &mut egui::Ui, w: &Widget, env: &PanelEnv, clicked: &mut Vec<String>) {
    let p = crate::theme::ui_palette();
    match w {
        Widget::Heading { text } => {
            ui.label(egui::RichText::new(text).heading().color(p.text_primary));
        }
        Widget::Label { text, weak } => {
            if *weak {
                ui.weak(text);
            } else {
                ui.label(text);
            }
        }
        Widget::Separator => {
            ui.separator();
        }
        Widget::Button { id, text, hint } => {
            // Without an engine nothing answers a press; say so instead of doing nothing.
            let response = ui.add_enabled(env.has_engine, egui::Button::new(text));
            let tip = if !env.has_engine {
                "This add-on has no engine to answer this button".to_owned()
            } else if hint.is_empty() {
                text.clone()
            } else {
                hint.clone()
            };
            if response.on_hover_text(tip).clicked() {
                clicked.push(id.clone());
            }
        }
        Widget::Lamp {
            id,
            text,
            state,
            lit,
        } => {
            let (set_state, set_lit) = env.lamps.get(id).copied().unwrap_or((None, None));
            let state = &set_state.unwrap_or(*state);
            let lit = &set_lit.unwrap_or(*lit);
            let color = match state {
                Role::Neutral => p.text_secondary,
                Role::Accent => p.accent,
                Role::Ok => p.success,
                Role::Warn => p.warning,
                Role::Error => p.error,
                Role::Info => p.info,
            };
            // Filled when lit, hollow when not, and the text always: never colour alone.
            let mark = if *lit { "●" } else { "○" };
            ui.label(egui::RichText::new(format!("{mark} {text}")).color(color));
        }
        Widget::Row { widgets } => {
            ui.horizontal_wrapped(|ui| {
                for child in widgets {
                    widget_ui(ui, child, env, clicked);
                }
            });
        }
    }
}

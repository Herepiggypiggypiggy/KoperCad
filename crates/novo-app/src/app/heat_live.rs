//! The heat result follows the cabinet. Every frame the Cabinet and 3D views
//! hand over the project; when the thermal scenario of the enclosure
//! differs from the one the shown result solved (a device moved, turned,
//! deleted or placed, a rail or duct edited, a loss changed), the result is
//! marked out of date at once. With auto re-solve on, a run for the new
//! scenario starts once the edits have been quiet for a moment; a run still
//! going for the old scenario is cancelled first. The run climbs Coarse,
//! Medium, Fine, so the first picture is a quick preview and the last one
//! the proper result.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use eframe::egui;
use novo_core::{Project, StructureId};
use novo_theme::tokens as t;
use novo_thermal::Scenario;
use novo_thermal::bridge::{BridgeOptions, scenario_from_project};

use super::heat::{HeatState, Meta};
use super::heat_run::Solution;

/// Quiet time after the last edit before an automatic run starts, s.
const DEBOUNCE_S: f64 = 0.4;
/// Cabinets with more devices than this start with auto re-solve off.
const AUTO_DEVICES: usize = 40;

/// Identity of a scenario for "same cabinet": a hash of its exact contents,
/// so floats never need to compare equal to themselves.
pub(super) fn scenario_key(s: &Scenario) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{s:?}").hash(&mut h);
    h.finish()
}

/// What the project gave at one revision.
struct Current {
    key: (StructureId, u64, u64, u64),
    /// `None` when the bridge cannot build one (no plate).
    scenario: Option<Arc<Scenario>>,
    scenario_key: u64,
    plate: [f64; 4],
    ambient: f64,
    messages: Vec<String>,
}

#[derive(Default)]
pub(super) struct Live {
    /// The user's choice; `None` follows the cabinet size.
    auto: Option<bool>,
    current: Option<Current>,
    /// Time of the last change of the scenario, egui seconds.
    changed_at: f64,
    stale: bool,
    /// The running run was cancelled because the cabinet changed.
    superseded: bool,
    /// A run was cancelled for the edited cabinet and none has replaced it
    /// yet, even when it left no result behind.
    rerun: bool,
    /// The scenario a run was last started for, by key. An automatic run is
    /// not started again for it, so a failed or stopped run is not retried
    /// every frame.
    started_for: Option<u64>,
    devices: usize,
}

impl Live {
    pub(super) fn auto_on(&self) -> bool {
        self.auto.unwrap_or(self.devices <= AUTO_DEVICES)
    }

    pub(super) fn set_auto(&mut self, on: bool) {
        self.auto = Some(on);
    }

    pub(super) fn is_stale(&self) -> bool {
        self.stale
    }

    /// The project revision the last Cabinet or 3D frame handed over, if any.
    pub(super) fn synced_revision(&self) -> Option<u64> {
        self.current.as_ref().map(|c| c.key.1)
    }

    /// True once after a run was cancelled for the edited cabinet.
    pub(super) fn take_superseded(&mut self) -> bool {
        std::mem::take(&mut self.superseded)
    }

    /// A run starts: nothing superseded or owed any more.
    pub(super) fn clear_superseded(&mut self) {
        self.superseded = false;
        self.rerun = false;
    }

    /// Whether `sol` no longer matches the cabinet. Without a scenario to
    /// compare (before the first sync) the layout key of the Cabinet view
    /// decides.
    pub(super) fn out_of_date(&self, sol: &Solution, layout_key: u64) -> bool {
        match self.current.as_ref().filter(|c| c.scenario.is_some()) {
            Some(now) => sol.scenario_key != now.scenario_key,
            None => sol.layout_key != layout_key,
        }
    }
}

impl HeatState {
    /// Follows the project: marks the result out of date and, with auto on,
    /// re-solves. Cheap when the project revision did not change. `dragging`
    /// holds the automatic start back while a device is in the pointer's
    /// hand.
    pub(super) fn sync_live(
        &mut self,
        project: &Project,
        structure: StructureId,
        ambient: f64,
        dragging: bool,
        now: f64,
        ctx: &egui::Context,
    ) {
        // The vents of the settings are part of the cabinet being solved.
        let vents = {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            format!("{:?}", self.settings.vents).hash(&mut h);
            h.finish()
        };
        let key = (structure, project.revision(), ambient.to_bits(), vents);
        if self.live.current.as_ref().map(|c| c.key) != Some(key) {
            let options = BridgeOptions {
                ambient_c: ambient,
                ..Default::default()
            };
            let built = scenario_from_project(project, structure, &options)
                .ok()
                .map(|mut b| {
                    let extra = self.settings.vents_for(&b.scenario.enclosure);
                    b.scenario.vents.extend(extra);
                    b
                });
            let (scenario, plate, messages) = match built {
                Some(b) => {
                    let p = b.scenario.enclosure.plate;
                    (
                        Some(Arc::new(b.scenario)),
                        [p.x, p.y, p.w, p.h],
                        b.report.messages(),
                    )
                }
                None => (None, [0.0; 4], Vec::new()),
            };
            let before = self.live.current.as_ref().and_then(|c| c.scenario.clone());
            if before.as_deref() != scenario.as_deref() {
                self.live.changed_at = now;
            }
            self.live.devices = scenario.as_ref().map_or(0, |s| s.devices.len());
            let scenario_key = scenario.as_deref().map_or(0, scenario_key);
            self.live.current = Some(Current {
                key,
                scenario,
                scenario_key,
                plate,
                ambient,
                messages,
            });
        }
        let Some(cur) = self.live.current.as_ref() else {
            return;
        };
        if cur.scenario.is_none() {
            self.live.stale = false;
            return;
        }
        let now_key = cur.scenario_key;
        let stale = self.solution().is_some_and(|s| s.scenario_key != now_key);
        let run_old = self
            .pending
            .as_ref()
            .is_some_and(|m| m.scenario_key != now_key);
        self.live.stale = stale;
        if !self.live.auto_on() || !(stale || run_old || self.live.rerun) {
            return;
        }
        if run_old {
            // Superseded: stop it; the next frames start the new one.
            if !self.live.superseded {
                self.live.superseded = true;
                self.live.rerun = true;
                self.cancel();
            }
            return;
        }
        if self.run.is_some() || dragging || self.live.started_for == Some(now_key) {
            return;
        }
        let wait = DEBOUNCE_S - (now - self.live.changed_at);
        if wait > 0.0 {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait));
            return;
        }
        self.start_current();
    }

    /// Starts a run for the cabinet as the last sync saw it (the Simulate
    /// button of the 3D view). Does nothing while one runs.
    pub(super) fn start_current(&mut self) {
        let Some(cur) = self.live.current.as_ref() else {
            return;
        };
        let Some(scenario) = cur.scenario.clone() else {
            self.fail("Open an enclosure with a mounting plate to simulate".into());
            return;
        };
        self.live.started_for = Some(cur.scenario_key);
        let meta = Meta {
            scenario_key: cur.scenario_key,
            layout_key: 0,
            ambient: cur.ambient,
            plate: cur.plate,
            messages: cur.messages.clone(),
        };
        let top = self.settings.top;
        self.start_to((*scenario).clone(), meta, top);
    }
}

/// "Out of date" chip and the auto re-solve switch, in the Heat strips of
/// the Cabinet and the 3D view.
pub(super) fn controls(ui: &mut egui::Ui, state: &mut HeatState, chip: bool) {
    if chip && state.live.is_stale() {
        let text = if state.running() && state.live.auto_on() {
            "Out of date: the cabinet changed, solving again"
        } else if state.live.auto_on() {
            "Out of date: the cabinet changed"
        } else {
            "Out of date: the cabinet changed, press Simulate"
        };
        ui.label(egui::RichText::new(text).small().strong().color(t::WARNING));
    }
    let mut auto = state.live.auto_on();
    if ui
        .checkbox(&mut auto, "Auto")
        .on_hover_text(
            "Solve again by itself when devices, rails or ducts change \
             (coarse preview first, then the full result)",
        )
        .changed()
    {
        state.live.set_auto(auto);
    }
}

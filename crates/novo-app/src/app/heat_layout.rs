//! "Cooler layout": asks the model where the devices that make or suffer the
//! heat would stand cooler on their rails, shows what each move wins and
//! applies the accepted moves as one undo step.
//!
//! Where the user finds it: Cabinet > Heat view > the "Cooler layout" button
//! next to Simulate, the plate's right-click menu, and the command palette
//! ("Suggest a cooler layout"). The search runs on a worker thread in the
//! 2D model (`novo_thermal::placement`); the window says that these are
//! planning values, not a validated result.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Instant;

use eframe::egui::{self, Color32, Pos2, Rect, Shape, Stroke, Vec2};
use novo_canvas::Viewport;
use novo_core::{Project, StructureId};
use novo_thermal::Scenario;
use novo_thermal::placement::{Control, Move, Settings, Suggestion, suggest_with};
use novo_thermal::placement_project::{plate_origin, rows};

/// Width of the window, points: it fits between the Unplaced list and the
/// plate at 1366 px.
const WIDTH: f32 = 360.0;

/// What a finished search found, and what it needs to be applied.
pub(super) struct Found {
    pub suggestion: Suggestion,
    pub structure: StructureId,
    /// Device tags by id.
    tags: BTreeMap<u64, String>,
    /// Plate origin in the interior frame, mm.
    origin: (f64, f64),
    /// Scenario the search started from; the result is out of date once the
    /// cabinet's scenario differs.
    scenario_key: u64,
}

struct Running {
    rx: mpsc::Receiver<Result<Suggestion, String>>,
    control: Arc<Control>,
    started: Instant,
    structure: StructureId,
    tags: BTreeMap<u64, String>,
    origin: (f64, f64),
    scenario_key: u64,
}

/// State of the search and its window.
#[derive(Default)]
pub(super) struct LayoutSearch {
    /// Knobs of the search; tests lower them to keep a run short.
    #[cfg(test)]
    pub(super) settings: Option<Settings>,
    requested: bool,
    run: Option<Running>,
    pub(super) found: Option<Found>,
    notice: Option<String>,
    open: bool,
}

/// What the window asks the app to do.
pub(super) enum Ask {
    /// Put these moves on the plate as one undo step.
    Apply(StructureId, Vec<Move>),
}

impl Found {
    #[cfg(test)]
    pub(super) fn scenario_key(&self) -> u64 {
        self.scenario_key
    }
}

impl LayoutSearch {
    /// `heat.cooler_layout`: search on the next frame.
    pub(super) fn request(&mut self) {
        self.requested = true;
        self.open = true;
    }

    pub(super) fn take_request(&mut self) -> bool {
        std::mem::take(&mut self.requested)
    }

    pub(super) fn running(&self) -> bool {
        self.run.is_some()
    }

    /// The reason the last search gave no finding, if it did not.
    #[cfg(test)]
    pub(super) fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    pub(super) fn close(&mut self) {
        if let Some(r) = &self.run {
            r.control.cancel.store(true, Ordering::Relaxed);
        }
        self.open = false;
        self.found = None;
        self.notice = None;
    }

    /// Starts the search from the scenario the Heat view follows. Says why
    /// when it cannot.
    pub(super) fn start(
        &mut self,
        project: &Project,
        structure: StructureId,
        scenario: Option<(Arc<Scenario>, u64)>,
        tag_of: &dyn Fn(u64) -> String,
    ) {
        if self.run.is_some() {
            return;
        }
        self.found = None;
        self.notice = None;
        let Some((scenario, scenario_key)) = scenario else {
            self.notice = Some(
                "Cannot search: set the enclosure size and the plate first (Size the enclosure)"
                    .into(),
            );
            return;
        };
        let Some(r) = rows(project, structure, &scenario) else {
            self.notice = Some("Cannot search: this enclosure has no plate".into());
            return;
        };
        if r.rows.is_empty() || r.row_of.iter().all(Option::is_none) {
            self.notice =
                Some("No device stands on a rail, so there is nothing to move along rails. Place all first.".into());
            return;
        }
        let origin = plate_origin(project, structure).unwrap_or((0.0, 0.0));
        let tags = scenario
            .devices
            .iter()
            .map(|d| (d.id, tag_of(d.id)))
            .collect();
        let control = Arc::new(Control::default());
        let (tx, rx) = mpsc::channel();
        let c = control.clone();
        let scn = (*scenario).clone();
        #[cfg(test)]
        let settings = self.settings.clone().unwrap_or_default();
        #[cfg(not(test))]
        let settings = Settings::default();
        std::thread::spawn(move || {
            let result =
                suggest_with(&scn, &r.rows, &r.row_of, &settings, &c).map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
        self.run = Some(Running {
            rx,
            control,
            started: Instant::now(),
            structure,
            tags,
            origin,
            scenario_key,
        });
    }

    /// Takes in a finished search. Called every frame.
    pub(super) fn poll(&mut self, ctx: &egui::Context) {
        let Some(run) = &self.run else {
            return;
        };
        match run.rx.try_recv() {
            Ok(result) => {
                let run = self.run.take().expect("checked above");
                match result {
                    Ok(suggestion) => {
                        self.found = Some(Found {
                            suggestion,
                            structure: run.structure,
                            tags: run.tags,
                            origin: run.origin,
                            scenario_key: run.scenario_key,
                        });
                    }
                    Err(e) => self.notice = Some(format!("The search failed: {e}")),
                }
            }
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.run = None;
                self.notice = Some("The search stopped unexpectedly".into());
            }
        }
    }

    fn tag(&self, id: u64) -> String {
        self.found
            .as_ref()
            .and_then(|f| f.tags.get(&id).cloned())
            .unwrap_or_else(|| format!("device {id}"))
    }

    /// The window. `now_key` is the scenario key of the cabinet as it is
    /// now (`None` while the Heat view does not follow it).
    pub(super) fn show(
        &mut self,
        ctx: &egui::Context,
        now_key: Option<u64>,
        at: Pos2,
    ) -> Option<Ask> {
        if !self.open {
            return None;
        }
        let mut ask = None;
        let mut close = false;
        let mut open = true;
        let pal = crate::theme::ui_palette();
        egui::Window::new("Cooler layout")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .fixed_pos(at)
            .default_width(WIDTH)
            .show(ctx, |ui| {
                ui.set_max_width(WIDTH);
                if let Some(run) = &self.run {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new());
                        ui.label(format!(
                            "Trying layouts in the model… {} so far, {:.0} s",
                            run.control.evaluated.load(Ordering::Relaxed),
                            run.started.elapsed().as_secs_f64()
                        ));
                    });
                    if ui.button("Stop and keep the best so far").clicked() {
                        run.control.cancel.store(true, Ordering::Relaxed);
                    }
                    return;
                }
                if let Some(n) = &self.notice {
                    ui.colored_label(pal.warning, n);
                }
                let Some(found) = &self.found else {
                    return;
                };
                let s = &found.suggestion;
                let stale = is_stale(now_key, found.scenario_key);
                if stale {
                    ui.colored_label(
                        pal.warning,
                        "The cabinet changed after this search. Search again before applying.",
                    );
                }
                let room = (ctx.content_rect().bottom() - at.y - 150.0).max(160.0);
                egui::ScrollArea::vertical()
                    .max_height(room)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                    ui.label(headline(s));
                    ui.add_space(4.0);
                    grid_of_numbers(ui, s);
                    if !s.moves.is_empty() {
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("Moves").strong());
                        for m in &s.moves {
                            ui.label(move_text(self, m));
                        }
                        let gains = best_gains(self, s);
                        if !gains.is_empty() {
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new("Margin gain, devices that gain most").strong());
                            for g in gains {
                                ui.label(g);
                            }
                        }
                    }
                    for n in &s.notes {
                        ui.add_space(4.0);
                        ui.colored_label(pal.text_secondary, n);
                    }
                    if !s.cooling.is_empty() {
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("Cooling that was tried on this layout").strong());
                        for r in s.cooling.iter().take(3) {
                            ui.label(&r.text);
                        }
                    }
                    });
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(
                        "Model estimate: the 2D heat model with planning values and the losses you entered. Not validated against measurements. Run Simulate after applying to see it in 3D.",
                    )
                    .small()
                    .color(pal.text_secondary),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let can = !s.moves.is_empty() && !stale;
                    if ui
                        .add_enabled(can, egui::Button::new("Apply the moves"))
                        .on_hover_text("Moves the devices on their rails as one undo step")
                        .clicked()
                    {
                        ask = Some(Ask::Apply(found.structure, s.moves.clone()));
                    }
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
            });
        if !open || close {
            self.close();
        }
        if ask.is_some() {
            self.close();
        }
        ask
    }

    /// Arrows from the old to the new place of every move, over the plate.
    pub(super) fn paint(&self, painter: &egui::Painter, rect: Rect, viewport: &Viewport) {
        let Some(found) = &self.found else {
            return;
        };
        let to_screen = |x: f64, y: f64| {
            let s = viewport.world_to_screen(novo_canvas::Vec2::new(x as f32, y as f32));
            rect.min + Vec2::new(s.x, s.y)
        };
        let pal = crate::theme::ui_palette();
        let stroke = Stroke::new(2.0, pal.accent);
        let (ox, oy) = found.origin;
        for m in &found.suggestion.moves {
            let corner = |r: &novo_layout::Rect| {
                Rect::from_min_max(
                    to_screen(r.x - ox, r.y - oy),
                    to_screen(r.right() - ox, r.bottom() - oy),
                )
            };
            let (a, b) = (corner(&m.from), corner(&m.to));
            painter.rect_stroke(
                a,
                2.0,
                Stroke::new(1.5, pal.text_tertiary),
                egui::StrokeKind::Outside,
            );
            painter.rect_stroke(b, 2.0, stroke, egui::StrokeKind::Outside);
            painter.rect_filled(b, 2.0, with_alpha(pal.accent, 48));
            arrow(painter, a.center(), b.center(), stroke);
        }
    }
}

/// A line with a small head, whatever its length (the painter's own arrow
/// grows its head with the line).
fn arrow(painter: &egui::Painter, from: Pos2, to: Pos2, stroke: Stroke) {
    let d = to - from;
    let len = d.length();
    if len < 4.0 {
        return;
    }
    let dir = d / len;
    let side = Vec2::new(-dir.y, dir.x);
    let tip = 11.0_f32.min(len * 0.5);
    painter.line_segment([from, to], stroke);
    painter.add(Shape::convex_polygon(
        vec![
            to,
            to - dir * tip + side * tip * 0.45,
            to - dir * tip - side * tip * 0.45,
        ],
        stroke.color,
        Stroke::NONE,
    ));
}

fn with_alpha(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

fn headline(s: &Suggestion) -> String {
    let n = s.moves.len();
    if n == 0 {
        return format!(
            "Tried {} layouts: no move along the rails wins enough.",
            s.evaluated
        );
    }
    let (over_a, over_b) = (s.before.devices_over, s.after.devices_over);
    let moved = if n == 1 {
        "1 device".to_owned()
    } else {
        format!("{n} devices")
    };
    let devices = |n: usize| {
        if n == 1 {
            "1 device".to_owned()
        } else {
            format!("{n} devices")
        }
    };
    if over_b < over_a {
        format!(
            "Moving {moved} takes {} of {} {} (tried {} layouts).",
            over_a - over_b,
            devices(over_a),
            if over_a == 1 {
                "under its limit"
            } else {
                "under their limit"
            },
            s.evaluated
        )
    } else if over_a == 0 {
        format!(
            "Moving {moved} widens the margins; no device is over its limit (tried {} layouts).",
            s.evaluated
        )
    } else {
        format!(
            "Moving {moved} cools the neighbours, but the {} {} (tried {} layouts).",
            devices(over_a),
            if over_a == 1 {
                "over its limit stays over"
            } else {
                "over their limit stay over"
            },
            s.evaluated
        )
    }
}

fn grid_of_numbers(ui: &mut egui::Ui, s: &Suggestion) {
    egui::Grid::new("cooler_layout_numbers")
        .num_columns(3)
        .spacing([14.0, 2.0])
        .show(ui, |ui| {
            ui.label("");
            ui.label(egui::RichText::new("Now").small());
            ui.label(egui::RichText::new("After the moves").small());
            ui.end_row();
            ui.label("Devices over their limit");
            ui.label(s.before.devices_over.to_string());
            ui.label(s.after.devices_over.to_string());
            ui.end_row();
            if let (Some(a), Some(b)) = (s.before.worst_margin_k, s.after.worst_margin_k) {
                ui.label("Tightest margin");
                ui.label(format!("{a:+.1} K"));
                ui.label(format!("{b:+.1} K"));
                ui.end_row();
            }
            ui.label("Hottest air");
            ui.label(format!("{:.1} °C", s.before.max_air_c));
            ui.label(format!("{:.1} °C", s.after.max_air_c));
            ui.end_row();
            ui.label("Distance the devices move");
            ui.label("");
            ui.label(format!("{:.1} m of travel", s.moved_mm / 1000.0));
            ui.end_row();
        });
}

fn move_text(search: &LayoutSearch, m: &Move) -> String {
    let dx = m.to.x - m.from.x;
    let dy = m.to.y - m.from.y;
    let along = if dx.abs() < 1.0 {
        String::new()
    } else if dx > 0.0 {
        format!("{dx:.0} mm right")
    } else {
        format!("{:.0} mm left", -dx)
    };
    let across = if dy.abs() < 1.0 {
        String::new()
    } else if dy > 0.0 {
        format!("{dy:.0} mm down to another rail")
    } else {
        format!("{:.0} mm up to another rail", -dy)
    };
    let parts: Vec<String> = [along, across]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    let how = parts.join(", ");
    match m.swapped_with {
        Some(other) => format!(
            "{}: {how} (swaps with {})",
            search.tag(m.id),
            search.tag(other)
        ),
        None => format!("{}: {how}", search.tag(m.id)),
    }
}

/// The three devices whose margin grows most, "tag +x.x K".
fn best_gains(search: &LayoutSearch, s: &Suggestion) -> Vec<String> {
    let mut gains: Vec<(f64, u64, f64, f64)> = s
        .scenario
        .devices
        .iter()
        .enumerate()
        .filter_map(|(i, d)| {
            let (b, a) = (s.before.margins[i]?, s.after.margins[i]?);
            Some((a - b, d.id, b, a))
        })
        .filter(|g| g.0 >= 0.5)
        .collect();
    gains.sort_by(|a, b| b.0.total_cmp(&a.0));
    gains
        .into_iter()
        .take(3)
        .map(|(g, id, b, a)| format!("{}: {b:+.1} K to {a:+.1} K ({g:+.1} K)", search.tag(id)))
        .collect()
}

/// A finding is only safe to apply while the Heat view follows the same
/// cabinet it was made from: no key (view not following) counts as stale.
pub(super) fn is_stale(now_key: Option<u64>, found_key: u64) -> bool {
    now_key != Some(found_key)
}

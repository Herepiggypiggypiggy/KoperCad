//! Kitting and stock: Reports > Bench output > Kitting tab, and
//! Reports > Export > Export kitting list (PDF).
//!
//! The tab sets the stock the shop buys and holds (bar and reel lengths, saw
//! cut, packs, what is on hand), shows the pick list for each build phase,
//! the saw plan and the merged shopping list, and writes them as one PDF next
//! to the project (also part of the Workshop pack). The plan comes from
//! `novo_reports::kitting`; the settings are a draft until saved to the
//! project (one undo step), and the exports use what the tab shows.

use std::path::{Path, PathBuf};

use eframe::egui;
use novo_reports::kitting::cutting::Source;
use novo_reports::kitting::{KitPlan, KitSettings, Kitting, Phase, SawBar, ShopLine, kit_plan};

use super::{Level, NovoApp};

use novo_reports::kitting::num_text;

/// The tab's draft settings and the plan last worked out from them.
#[derive(Default)]
pub(super) struct KitDraft {
    settings: Option<KitSettings>,
    rail_text: String,
    duct_text: String,
    cache: Option<Cache>,
}

struct Cache {
    revision: u64,
    settings: KitSettings,
    plan: KitPlan,
}

/// What the tab asks for.
pub(super) enum KitAct {
    Export,
    Save(KitSettings),
}

fn lengths_text(list: &[f64]) -> String {
    list.iter()
        .map(|l| format!("{l}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn read_lengths(text: &str) -> Option<Vec<f64>> {
    let list: Vec<f64> = text
        .split([',', ';', ' '])
        .filter(|w| !w.trim().is_empty())
        .map(|w| w.trim().replace(',', ".").parse::<f64>())
        .collect::<Result<_, _>>()
        .ok()?;
    (!list.is_empty() && list.iter().all(|l| l.is_finite() && *l > 0.0)).then_some(list)
}

impl KitDraft {
    /// Works the plan out again when the project or the draft changed.
    pub(super) fn refresh(&mut self, project: &novo_core::Project, stored: &KitSettings) {
        self.ensure(stored);
        let revision = project.revision();
        if let Some(settings) = self.current() {
            let stale = self
                .cache
                .as_ref()
                .is_none_or(|c| c.revision != revision || c.settings != settings);
            if stale {
                self.cache = Some(Cache {
                    revision,
                    plan: kit_plan(project, &settings),
                    settings,
                });
            }
        }
    }

    #[cfg(test)]
    pub(super) fn set_text_for_test(&mut self, rail: &str, duct: &str) {
        self.rail_text = rail.to_owned();
        self.duct_text = duct.to_owned();
    }

    fn ensure(&mut self, stored: &KitSettings) -> &mut KitSettings {
        if self.settings.is_none() {
            self.rail_text = lengths_text(&stored.rail_stock_mm);
            self.duct_text = lengths_text(&stored.duct_stock_mm);
            self.settings = Some(stored.clone());
        }
        self.settings.as_mut().expect("set above")
    }

    /// The draft as settings when every field reads, else `None`.
    pub(super) fn current(&self) -> Option<KitSettings> {
        let mut s = self.settings.clone()?;
        s.rail_stock_mm = read_lengths(&self.rail_text)?;
        s.duct_stock_mm = read_lengths(&self.duct_text)?;
        s.is_valid().then_some(s)
    }
}

fn bar_line(bar: &SawBar) -> String {
    let from = match bar.source {
        Source::New(l) => format!("new {} mm", num_text(l)),
        Source::Held(l) => format!("offcut {} mm", num_text(l)),
    };
    let cuts: Vec<String> = bar
        .cuts
        .iter()
        .map(|c| format!("{} {}", c.label, num_text(c.length)))
        .collect();
    let end = if bar.left <= 0.0 {
        String::new()
    } else if bar.keep {
        format!("; {} mm left, keep", num_text(bar.left))
    } else {
        format!("; {} mm left (scrap)", num_text(bar.left))
    };
    format!("{}. {from}: {}{end}", bar.number, cuts.join(", "))
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// `3 bars and 1 reel to buy, 16 lines on the shopping list`.
fn summary(plan: &KitPlan) -> String {
    format!(
        "{} and {} to buy, {} on the shopping list",
        plural(plan.bars_to_buy(), "bar", "bars"),
        plural(plan.reels_to_buy(), "reel", "reels"),
        plural(plan.shopping.len(), "line", "lines")
    )
}

fn shop_row(ui: &mut egui::Ui, l: &ShopLine) {
    use num_text as n;
    ui.label(l.shop.text());
    ui.label(&l.item);
    ui.label(format!("{} {}", n(l.need), l.unit));
    ui.label(n(l.have));
    ui.label(egui::RichText::new(n(l.buy)).strong());
    ui.end_row();
}

/// The tab. `stored` are the project's settings, `plan` is worked out here
/// from the draft when the project or the draft changed.
pub(super) fn kit_tab(
    ui: &mut egui::Ui,
    draft: &mut KitDraft,
    stored: &KitSettings,
    act: &mut Option<KitAct>,
) {
    draft.ensure(stored);
    let current = draft.current();

    ui.add_space(6.0);
    let changed = current.as_ref().is_some_and(|c| c != stored);
    ui.horizontal(|ui| {
        let export = novo_theme::widgets::primary_button(ui, "Export kitting list (PDF)")
            .on_hover_text(super::KITTING_HINT);
        super::cabinet_bar::note_ctx(
            ui.ctx(),
            "kitting.export",
            "Export kitting list button",
            &export,
        );
        if export.clicked() {
            *act = Some(KitAct::Export);
        }
        if ui
            .add_enabled(changed, egui::Button::new("Save to project"))
            .on_hover_text("Keep these settings in the project (one undo step)")
            .clicked()
            && let Some(c) = &current
        {
            *act = Some(KitAct::Save(c.clone()));
        }
    });
    ui.add_space(6.0);

    ui.add_space(4.0);
    'plan: {
        let Some(cache) = draft.cache.as_ref() else {
            break 'plan;
        };
        let plan = &cache.plan;
        if plan.is_empty() {
            ui.weak("Nothing to kit yet: give a cabinet a mounting plate, place rails and devices, and assign parts.");
            break 'plan;
        }
        ui.label(format!(
            "{} bars and {} reels to buy, {} lines on the shopping list",
            plan.bars_to_buy(),
            plan.reels_to_buy(),
            plan.shopping.len()
        ));
        for p in &plan.problems {
            ui.colored_label(crate::theme::ui_palette().warning, p);
        }
        ui.add_space(4.0);
        ui.weak("Pick lists keep the step numbers of the assembly checklist: step 2, Drill, has no pick list.");
        for phase in Phase::ALL {
            let picks: Vec<_> = plan.picks_in(phase).collect();
            if picks.is_empty() {
                continue;
            }
            egui::CollapsingHeader::new(format!(
                "Pick list {}  {} ({})",
                phase.step(),
                phase.name(),
                picks.len()
            ))
            .id_salt(("kit_phase", phase.key()))
            .default_open(phase == Phase::Prepare)
            .show(ui, |ui| {
                egui::Grid::new(("kit_pick", phase.key()))
                    .num_columns(3)
                    .spacing([10.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        for p in picks {
                            ui.label(egui::RichText::new(num_text(p.qty)).strong());
                            ui.label(p.unit);
                            ui.vertical(|ui| {
                                ui.label(&p.item);
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(format!("{}. For {}", p.from, p.for_))
                                            .weak(),
                                    )
                                    .wrap(),
                                );
                            });
                            ui.end_row();
                        }
                    });
            });
        }
        for saw in &plan.saw {
            egui::CollapsingHeader::new(format!(
                "Saw plan  {}: {} bars to buy, {} mm of offcuts to keep",
                saw.name,
                saw.plan.bars_bought(),
                num_text(saw.plan.offcuts().iter().sum::<f64>())
            ))
            .id_salt(("kit_saw", saw.key.clone()))
            .show(ui, |ui| {
                for bar in &saw.bars {
                    ui.label(bar_line(bar));
                }
                if saw.plan.budget_hit {
                    ui.weak("The search stopped early; fewer bars may be possible.");
                } else if saw.plan.proven_optimal() {
                    ui.weak(
                    "No plan can buy less: the pieces fill the bars as far as the saw cuts allow.",
                );
                }
            });
        }
        if !plan.shopping.is_empty() {
            egui::CollapsingHeader::new(format!("Shopping list ({})", plan.shopping.len()))
                .id_salt("kit_shop")
                .show(ui, |ui| {
                    egui::Grid::new("kit_shop_grid")
                        .num_columns(5)
                        .spacing([10.0, 2.0])
                        .striped(true)
                        .show(ui, |ui| {
                            for h in ["From", "Item", "Need", "Have", "Buy"] {
                                ui.weak(h);
                            }
                            ui.end_row();
                            for l in &plan.shopping {
                                shop_row(ui, l);
                            }
                        });
                });
        }
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Stock the shop buys and holds")
        .id_salt("kit_settings_open")
        .default_open(false)
        .show(ui, |ui| {
    ui.weak("Planning values: edit them to match your supplier. Counts of rails, devices, wires and holes come from the drawing.");
    {
        let rail_ok = read_lengths(&draft.rail_text).is_some();
        let duct_ok = read_lengths(&draft.duct_text).is_some();
        let s = draft.settings.as_mut().expect("ensured");
        egui::Grid::new("kit_settings")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                ui.label("DIN rail bars, mm");
                let r = ui.add(
                    egui::TextEdit::singleline(&mut draft.rail_text)
                        .desired_width(160.0)
                        .hint_text("2000, 1000"),
                );
                if !rail_ok {
                    r.on_hover_text("Lengths in mm, separated by commas");
                }
                ui.end_row();
                ui.label("Duct bars, mm");
                ui.add(
                    egui::TextEdit::singleline(&mut draft.duct_text)
                        .desired_width(160.0)
                        .hint_text("2000"),
                );
                ui.end_row();
                ui.label("Saw cut (kerf), mm");
                ui.add(
                    egui::DragValue::new(&mut s.kerf_mm)
                        .speed(0.1)
                        .range(0.0..=20.0)
                        .max_decimals(1),
                );
                ui.end_row();
                ui.label("Keep offcuts from, mm");
                ui.add(
                    egui::DragValue::new(&mut s.min_offcut_mm)
                        .speed(5.0)
                        .range(0.0..=2000.0),
                );
                ui.end_row();
                ui.label("Wire reel, m");
                ui.add(
                    egui::DragValue::new(&mut s.reel_m)
                        .speed(1.0)
                        .range(1.0..=2000.0),
                );
                ui.end_row();
                ui.label("Held back per reel, %").on_hover_text(
                    "For re-cuts and mistakes: wires are packed on the reel as if this much longer",
                );
                ui.add(
                    egui::DragValue::new(&mut s.wire_allowance_percent)
                        .speed(0.5)
                        .range(0.0..=50.0),
                );
                ui.end_row();
                ui.label("Ferrules per box");
                ui.add(egui::DragValue::new(&mut s.pack_ferrule).range(1..=5000));
                ui.end_row();
                ui.label("Lugs per bag");
                ui.add(egui::DragValue::new(&mut s.pack_lug).range(1..=5000));
                ui.end_row();
                ui.label("Screws per box");
                ui.add(egui::DragValue::new(&mut s.pack_screw).range(1..=5000));
                ui.end_row();
            });
        if !rail_ok || !duct_ok {
            ui.colored_label(
                crate::theme::ui_palette().warning,
                "A bar length is not a number above zero: the plan shows the last good settings",
            );
        }
        ui.add_space(4.0);
        ui.label("On hand (one line each)")
            .on_hover_text("rail 1200 x2\nduct 50x60 1500\nwire 1.5 BK 37.5 m\nhave A1.5-10 x40");
        ui.add(
            egui::TextEdit::multiline(&mut s.on_hand)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .hint_text("rail 1200 x2\nduct 50x60 1500\nwire 1.5 BK 37.5 m\nhave A1.5-10 x40"),
        );
        ui.label("Order numbers of stock items (Item = order number)")
            .on_hover_text(
                "Merges the stock line into the bill of materials line with that order number",
            );
        ui.add(
            egui::TextEdit::multiline(&mut s.articles)
                .desired_rows(2)
                .desired_width(f32::INFINITY)
                .hint_text("DIN rail 2000 mm bar = 1201578"),
        );
    }
        });
}

impl NovoApp {
    /// The kitting settings the exports use: the open tab's draft when it
    /// reads, else the project's.
    fn kitting_settings(&self) -> Option<KitSettings> {
        if let Some(draft) = self.wb.bench.as_ref().and_then(|b| b.kit.current()) {
            return Some(draft);
        }
        Some(KitSettings::of(self.backend.document()?.project()))
    }

    /// Reports > Export > Export kitting list (PDF), next to the project.
    pub(super) fn export_kitting(&mut self) -> Option<PathBuf> {
        let path = self.bench_path("kitting list", "pdf")?;
        self.export_kitting_to(&path)
    }

    /// The kitting PDF's bytes, or `None` when there is nothing to kit.
    pub(super) fn kitting_pdf(&mut self, quiet: bool) -> Option<(Vec<u8>, String)> {
        let settings = self.kitting_settings()?;
        let name = self.machine_title().unwrap_or_default();
        let language = self.report_translator();
        let project = self.machine()?;
        let plan = kit_plan(&project, &settings);
        // The pack carries it only when there is something to cut or wire.
        if plan.is_empty() || (quiet && plan.saw.is_empty() && plan.reels.is_empty()) {
            if quiet {
                return None;
            }
            self.message(
                Level::Info,
                "Nothing to kit yet: give a cabinet a mounting plate, place rails and devices, and assign parts",
            );
            return None;
        }
        let report = Kitting::build(&plan, project.revision());
        let options = super::bench::pdf_options(
            format!("{name}  Kitting and shopping list"),
            report.sections.iter().map(|s| s.key.clone()),
            language,
        );
        let bytes = novo_pdf::export_report_pdf(&[&report], &options);
        let what = format!("kitting list ({})", summary(&plan));
        Some((bytes, what))
    }

    /// Writes the kitting PDF to `path`.
    pub fn export_kitting_to(&mut self, path: &Path) -> Option<PathBuf> {
        let (bytes, what) = self.kitting_pdf(false)?;
        self.bench_write(path, &bytes, &what)
    }
}

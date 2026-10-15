//! The Project settings window: what belongs to the project file, not to
//! the user. Today that is the drawing standard, which picks the symbol pack.

use eframe::egui;
use novo_symbols::Pack;

use super::{Level, NovoApp};

impl NovoApp {
    /// The project's drawing standard; IEC when the project does not say.
    pub(super) fn standard(&self) -> Pack {
        self.backend
            .document()
            .and_then(|d| Pack::from_id(&d.project().info().rules.standard))
            .unwrap_or(Pack::Iec)
    }

    /// " · NFPA" for a project that is not IEC, else nothing: IEC is the
    /// default and needs no label.
    pub(super) fn standard_suffix(&self) -> String {
        match self.standard() {
            Pack::Iec => String::new(),
            pack => format!(" \u{b7} {}", pack.id().to_uppercase()),
        }
    }

    /// Saves the standard in the project (one undo step).
    pub(super) fn set_standard(&mut self, pack: Pack) {
        if pack == self.standard() {
            return;
        }
        let id = if pack == Pack::Iec { "" } else { pack.id() };
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        match doc.transact("Drawing standard", |tx| {
            tx.set_info(|i| i.rules.standard = id.to_owned())
        }) {
            Ok(((), changes)) => self.changed_outside_canvas(&changes),
            Err(e) => self.message(Level::Error, format!("Drawing standard: {e}")),
        }
    }

    /// Re-keys the placed symbols to the project's drawing standard, in one
    /// undo step; symbols with no drawing there stay as they are.
    pub(super) fn switch_placed_symbols(&mut self) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let plan = novo_workflow::switch::plan(
            doc.project(),
            novo_workflow::switch::current(doc.project()),
        );
        if plan.rekey.is_empty() {
            return;
        }
        match doc.execute(plan.command()) {
            Ok(out) => self.changed_outside_canvas(&out.changes),
            Err(e) => self.message(Level::Error, format!("Drawing standard: {e}")),
        }
    }

    pub(super) fn project_settings_dialog(&mut self, ctx: &egui::Context) {
        if !self.wb.project_settings {
            return;
        }
        if self.backend.document().is_none() {
            self.wb.project_settings = false;
            return;
        }
        let mut pack = self.standard();
        let plan = self
            .backend
            .document()
            .map(|d| novo_workflow::switch::plan(d.project(), pack));
        let stored = self
            .backend
            .document()
            .map(|d| d.project().info().rules.standard.clone())
            .unwrap_or_default();
        let mut apply = false;
        let modal = egui::Modal::new(egui::Id::new("project_settings")).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.heading("Project settings");
            ui.add_space(6.0);
            ui.strong("Drawing standard");
            ui.weak("Picks the symbol pack for new symbols.");
            for (value, label) in [
                (Pack::Iec, "IEC 60617 (default)"),
                (Pack::Nfpa, "NFPA 79 / ANSI Y32.2"),
            ] {
                ui.radio_value(&mut pack, value, label);
            }
            if !stored.is_empty() && Pack::from_id(&stored).is_none() {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    format!("This project says \"{stored}\", which this version does not know. It is shown as IEC."),
                );
            }
            if let Some(plan) = &plan {
                apply = switch_section(ui, plan);
            }
            ui.add_space(8.0);
            ui.button("Close (Esc)").clicked()
        });
        self.set_standard(pack);
        if apply {
            self.switch_placed_symbols();
        }
        if modal.inner || modal.should_close() {
            self.wb.project_settings = false;
        }
    }
}

/// The preview and the button that re-keys placed symbols to the pack the
/// project is set to. Returns whether the button was pressed.
fn switch_section(ui: &mut egui::Ui, plan: &novo_workflow::switch::SwitchPlan) -> bool {
    if plan.rekey.is_empty() && plan.without_counterpart.is_empty() {
        return false;
    }
    let name = plan.pack.standard();
    ui.add_space(8.0);
    ui.strong("Placed symbols");
    let mut pressed = false;
    if !plan.rekey.is_empty() {
        ui.label(format!(
            "{} placed symbols are drawn in another standard.",
            plan.rekey.len()
        ));
        pressed = ui
            .button(format!(
                "Switch {} placed symbols to {}",
                plan.rekey.len(),
                plan.pack.id().to_uppercase()
            ))
            .on_hover_text(format!(
                "One undo step. Connections and positions stay. Target: {name}."
            ))
            .clicked();
    }
    if !plan.without_counterpart.is_empty() {
        let devices = plan.devices_without_counterpart();
        let list = devices
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let n = plan.without_counterpart.len();
        let text = if list.is_empty() {
            format!(
                "{n} symbols have no {} drawing and stay as drawn.",
                plan.pack.id().to_uppercase()
            )
        } else {
            format!(
                "{n} symbols have no {} drawing and stay as drawn: {list}.",
                plan.pack.id().to_uppercase()
            )
        };
        ui.weak(text);
    }
    pressed
}

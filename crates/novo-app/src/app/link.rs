//! `link` (`xref.link`, DESIGN.md §6): joins a contact to the coil of a
//! device. `link -K3 -K1` names the devices; a bare `link` takes the two
//! symbols selected. novo-xref gives the edit and the text of a refusal.

use novo_core::{AnyId, DeviceTag, SymbolId};
use novo_workflow::palette::{LINK_PICK_COIL, LINK_PICK_CONTACT, link_tags};
use novo_xref::{link_command, link_to_device_command};

use super::{Level, NovoApp};

impl NovoApp {
    pub(super) fn xref_link(&mut self, argument: Option<&str>) {
        let Some(library) = self
            .canvas
            .schematic_ref()
            .map(|v| v.editor.catalog.library())
        else {
            return;
        };
        let selected: Vec<SymbolId> = self
            .selection()
            .into_iter()
            .filter_map(|id| match id {
                AnyId::Symbol(s) => Some(s),
                _ => None,
            })
            .collect();
        let Some(doc) = self.backend.document() else {
            return;
        };
        let project = doc.project();
        let text = argument.map(str::trim).unwrap_or_default();
        let command = if text.is_empty() {
            let [a, b] = selected.as_slice() else {
                self.message(
                    Level::Info,
                    format!("{LINK_PICK_CONTACT}, then {LINK_PICK_COIL}: select both (Shift+click), then link"),
                );
                return;
            };
            link_command(project, library, *a, *b)
        } else {
            let Some((contact, coil)) = link_tags(text) else {
                self.message(Level::Warning, "Link takes two device tags: link -K3 -K1");
                return;
            };
            let device = |tag: &str| {
                DeviceTag::parse(tag)
                    .ok()
                    .and_then(|t| project.devices_by_tag(&t).first().copied())
            };
            let (Some(from), Some(to)) = (device(contact), device(coil)) else {
                self.message(
                    Level::Warning,
                    format!("No device {contact} or {coil}; place it first"),
                );
                return;
            };
            let Some(symbol) = project.symbols_of_device(from).first().map(|s| s.id) else {
                self.message(
                    Level::Warning,
                    format!("{contact} has nothing drawn to link"),
                );
                return;
            };
            link_to_device_command(project, library, symbol, to)
        };
        let command = match command {
            Ok(command) => command,
            Err(e) => {
                self.message(Level::Warning, e.to_string());
                return;
            }
        };
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        match doc.execute(command) {
            Ok(outcome) => {
                self.changed_outside_canvas(&outcome.changes);
                self.message(Level::Info, "Linked");
            }
            Err(e) => self.message(Level::Error, format!("Could not link: {e}")),
        }
    }
}

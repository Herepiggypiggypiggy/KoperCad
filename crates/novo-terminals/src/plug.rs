//! Plug and connector connection lists: per connector device, every contact
//! with the wires landing on it, what they reach, the cable core and where
//! the contact is drawn. Same idea as the strip [`screen`](crate::screen),
//! so the drawer, the generated page, PDF and CSV show the same rows.

use std::collections::HashSet;

use novo_core::{DeviceId, DeviceKind, Project, natural_cmp};

use crate::occurrence::side;

/// One contact (pin name) of a connector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contact {
    /// Pin name printed on the connector (`1`, `PE`).
    pub name: String,
    /// Numbers of the wires landing on the contact.
    pub wires: Vec<String>,
    /// Designations reached through those wires (`-K1:14`).
    pub targets: Vec<String>,
    /// Cable and core of the wires (`-W2 BK`).
    pub cables: Vec<String>,
    /// `page.column` of each drawing of the connector that has this contact.
    pub places: Vec<String>,
    /// Nothing is wired to the contact.
    pub open: bool,
}

/// The connection list of one connector device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlugList {
    pub device: DeviceId,
    pub tag: String,
    pub description: String,
    pub contacts: Vec<Contact>,
}

fn distinct(items: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    items
        .into_iter()
        .filter(|item| !item.is_empty() && seen.insert(item.clone()))
        .collect()
}

/// Connector devices, sorted by tag with numbers by value.
pub fn connectors(project: &Project) -> Vec<DeviceId> {
    let mut list: Vec<_> = project
        .devices()
        .filter(|d| d.kind == DeviceKind::Connector)
        .map(|d| (d.tag.to_string(), d.id))
        .collect();
    list.sort_by(|a, b| natural_cmp(&a.0, &b.0).then(a.1.cmp(&b.1)));
    list.into_iter().map(|(_, id)| id).collect()
}

/// Builds the connection list of a connector device from the model. Pins
/// with the same name on several drawings are one contact.
pub fn plug_list(project: &Project, device: DeviceId) -> Option<PlugList> {
    let info = project.device(device)?;
    let mut names: Vec<String> = Vec::new();
    // (name, wires, targets, cables, places)
    let mut acc: Vec<Contact> = Vec::new();
    let mut symbols = project.symbols_of_device(device);
    symbols.sort_by_key(|s| s.id);
    for symbol in symbols {
        let place = project
            .location_of(symbol.id)
            .map(|l| crate::strings::place(&l))
            .unwrap_or_default();
        for (index, pin) in symbol.pins.iter().enumerate() {
            let Ok(index) = u16::try_from(index) else {
                continue;
            };
            let reach = side(project, symbol, vec![index]);
            let slot = match names.iter().position(|n| *n == pin.name) {
                Some(i) => i,
                None => {
                    names.push(pin.name.clone());
                    acc.push(Contact {
                        name: pin.name.clone(),
                        wires: Vec::new(),
                        targets: Vec::new(),
                        cables: Vec::new(),
                        places: Vec::new(),
                        open: true,
                    });
                    acc.len() - 1
                }
            };
            let contact = &mut acc[slot];
            contact.open &= reach.wires.is_empty();
            contact.places.push(place.clone());
            for &wire in &reach.wires {
                if let Some(number) = project.wire(wire).and_then(|w| w.number.clone()) {
                    contact.wires.push(number);
                }
                if let Some((cable, core)) = project.core_of_wire(wire)
                    && let Some(cable) = project.cable(cable)
                    && let Some(core) = cable.cores.get(core)
                {
                    contact.cables.push(format!("{} {}", cable.tag, core.name));
                }
            }
            contact
                .targets
                .extend(reach.targets.into_iter().map(|t| t.designation));
        }
    }
    for contact in &mut acc {
        contact.wires = distinct(std::mem::take(&mut contact.wires));
        contact.targets = distinct(std::mem::take(&mut contact.targets));
        contact.cables = distinct(std::mem::take(&mut contact.cables));
        contact.places = distinct(std::mem::take(&mut contact.places));
    }
    acc.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    Some(PlugList {
        device,
        tag: info.tag.to_string(),
        description: info.description.clone(),
        contacts: acc,
    })
}

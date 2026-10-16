//! Net trace: the whole net of what is selected.
//!
//! `net.select` selects every wire and junction of the net on the shown page.
//! `net.trace` opens a small window per net listing the potentials, the
//! devices and the pages it reaches, each with a jump button. Both read
//! the connectivity graph (novo-core `Netlist`), so a net that crosses a
//! cross-reference or an interruption point is followed too.
//!
//! Where the user finds it: Edit > Net, the right-click menu of a wire or
//! part (Select net, Pages and devices on this net), Ctrl+K "net".

use eframe::egui;
use novo_core::{AnyId, Endpoint, NetId, Netlist, PageId, Project};
use novo_xref::Place;

use super::{Level, NovoApp};

/// At most this many nets are listed for one selection.
const MAX_NETS: usize = 8;

/// One net as the window shows it, read when the window opened.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct NetInfo {
    pub title: String,
    pub wires: usize,
    /// Device tag and the place of its first symbol.
    pub devices: Vec<(String, Place)>,
    /// Page name and the page.
    pub pages: Vec<(String, PageId)>,
}

/// The open net trace window.
#[derive(Debug, Clone, Default)]
pub(super) struct NetTrace {
    pub nets: Vec<NetInfo>,
    /// Nets beyond `MAX_NETS` that the selection also touched.
    pub more: usize,
}

/// The nets the selected wires, junctions and parts belong to, in order,
/// each once. A part contributes the nets of its pins.
pub(super) fn nets_of(project: &Project, netlist: &Netlist, selection: &[AnyId]) -> Vec<NetId> {
    let mut nets: Vec<NetId> = Vec::new();
    let mut add = |net: Option<NetId>| {
        if let Some(net) = net
            && !netlist.net(net).wires.is_empty()
            && !nets.contains(&net)
        {
            nets.push(net);
        }
    };
    for &id in selection {
        match id {
            AnyId::Wire(w) => add(project.wire(w).and_then(|w| netlist.net_of_wire(w))),
            AnyId::Junction(j) => add(netlist.net_of(Endpoint::junction(j))),
            AnyId::Symbol(s) => {
                if let Some(symbol) = project.symbol(s) {
                    for pin in 0..symbol.pins.len() {
                        add(netlist.net_of(Endpoint::pin(s, u16::try_from(pin).unwrap_or(0))));
                    }
                }
            }
            _ => {}
        }
    }
    nets
}

/// What the window shows for one net.
pub(super) fn describe(project: &Project, netlist: &Netlist, id: NetId) -> NetInfo {
    let net = netlist.net(id);
    let mut names: Vec<String> = net
        .potentials
        .iter()
        .filter_map(|&p| project.potential(p).map(|p| p.name.clone()))
        .collect();
    names.dedup();
    let number = net
        .wires
        .iter()
        .find_map(|&w| project.wire(w).and_then(|w| w.number.clone()));
    let title = match (names.is_empty(), number) {
        (false, Some(n)) => format!("{} (wire {n})", names.join(", ")),
        (false, None) => names.join(", "),
        (true, Some(n)) => format!("Wire {n}"),
        (true, None) => "Net without a name".to_owned(),
    };
    let devices = netlist
        .devices_on_net(project, id)
        .into_iter()
        .filter_map(|d| {
            let device = project.device(d)?;
            let symbol = project.symbols_of_device(d).first().copied()?;
            Some((
                device.tag.to_string(),
                Place {
                    page: symbol.page,
                    symbol: Some(symbol.id),
                },
            ))
        })
        .collect();
    let pages = netlist
        .pages_of_net(project, id)
        .into_iter()
        .filter_map(|p| project.page(p).map(|page| (page.name.clone(), p)))
        .collect();
    NetInfo {
        title,
        wires: net.wires.len(),
        devices,
        pages,
    }
}

impl NovoApp {
    /// `net.select`: the whole net of the selection, on the shown page.
    pub(super) fn select_net(&mut self) {
        let (Some(doc), Some(page)) = (self.backend.document(), self.current_page) else {
            return;
        };
        let project = doc.project();
        let netlist = project.netlist();
        let nets = nets_of(project, &netlist, &self.selection());
        if nets.is_empty() {
            self.message(Level::Info, "Select a wire or a connected part first");
            return;
        }
        let mut ids: Vec<AnyId> = Vec::new();
        let mut elsewhere = false;
        for &net in &nets {
            let net = netlist.net(net);
            for &w in &net.wires {
                match project.wire(w) {
                    Some(wire) if wire.page == page => ids.push(AnyId::Wire(w)),
                    Some(_) => elsewhere = true,
                    None => {}
                }
            }
            for end in &net.endpoints {
                if let Endpoint::Junction { junction } = end
                    && project.junction(*junction).is_some_and(|j| j.page == page)
                {
                    ids.push(AnyId::Junction(*junction));
                }
            }
        }
        let count = ids.len();
        if let Some(view) = self.canvas.schematic() {
            view.select_ids(&ids);
        }
        let mut text = format!(
            "Selected the net: {count} {}",
            if count == 1 { "object" } else { "objects" }
        );
        if elsewhere {
            text += " here; it continues on other pages (Pages and devices on this net)";
        }
        self.message(Level::Info, text);
    }

    /// `net.trace`: opens the list of devices and pages on the selection's nets.
    pub(super) fn trace_net(&mut self) {
        let Some(doc) = self.backend.document() else {
            return;
        };
        let project = doc.project();
        let netlist = project.netlist();
        let nets = nets_of(project, &netlist, &self.selection());
        if nets.is_empty() {
            self.message(Level::Info, "Select a wire or a connected part first");
            return;
        }
        let more = nets.len().saturating_sub(MAX_NETS);
        let infos = nets
            .into_iter()
            .take(MAX_NETS)
            .map(|n| describe(project, &netlist, n))
            .collect();
        self.wb.net_trace = Some(NetTrace { nets: infos, more });
    }

    pub(super) fn net_trace_window(&mut self, ctx: &egui::Context) {
        let Some(trace) = self.wb.net_trace.take() else {
            return;
        };
        let mut open = true;
        let mut jump: Option<Place> = None;
        let mut page: Option<PageId> = None;
        super::panel::dialog(ctx, "Pages and devices on this net", [380.0, 0.0])
            .id(egui::Id::new("net_trace"))
            .open(&mut open)
            .show(ctx, |ui| {
                ui.weak("As of opening; select again to refresh");
                for (i, net) in trace.nets.iter().enumerate() {
                    egui::CollapsingHeader::new(
                        egui::RichText::new(format!(
                            "{} ({} {})",
                            net.title,
                            net.wires,
                            if net.wires == 1 { "wire" } else { "wires" }
                        ))
                        .strong(),
                    )
                    .id_salt(("net_trace_net", i))
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.label("Devices");
                        if net.devices.is_empty() {
                            ui.weak("None");
                        }
                        ui.horizontal_wrapped(|ui| {
                            for (tag, place) in &net.devices {
                                if ui.link(tag).on_hover_text("Go to this device").clicked() {
                                    jump = Some(*place);
                                }
                            }
                        });
                        ui.label("Pages");
                        ui.horizontal_wrapped(|ui| {
                            for (name, id) in &net.pages {
                                if ui
                                    .link(format!("Page {name}"))
                                    .on_hover_text("Open this page")
                                    .clicked()
                                {
                                    page = Some(*id);
                                }
                            }
                        });
                    });
                }
                if trace.more > 0 {
                    ui.weak(format!(
                        "{} more nets in the selection; select fewer to list them",
                        trace.more
                    ));
                }
            });
        let keep = open && !ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if keep {
            self.wb.net_trace = Some(trace);
        }
        if let Some(place) = jump {
            self.jump(place, true);
        } else if let Some(page) = page {
            if let Some(here) = self.here() {
                self.wb.nav.history.record(here);
            }
            self.show_page(page);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{Document, Pin, Placement, SymbolRole};

    #[test]
    fn a_wire_names_its_net_with_devices_and_pages() {
        let mut doc = Document::default();
        let ((a, b), _) = doc
            .transact("net", |tx| {
                let page = tx.add_page("1")?;
                let pins = || vec![Pin::new("1", 0.0, -2.5), Pin::new("2", 0.0, 2.5)];
                let sa = tx.place_symbol(
                    page,
                    "iec/contact",
                    Placement::at(novo_core::Point::new(10.0, 10.0)),
                    pins(),
                    SymbolRole::Unassigned,
                )?;
                let sb = tx.place_symbol(
                    page,
                    "iec/contact",
                    Placement::at(novo_core::Point::new(10.0, 40.0)),
                    pins(),
                    SymbolRole::Unassigned,
                )?;
                let w = tx.connect(Endpoint::pin(sa, 1), Endpoint::pin(sb, 0))?;
                Ok((w, sa))
            })
            .unwrap();
        let project = doc.project();
        let netlist = project.netlist();
        let nets = nets_of(project, &netlist, &[AnyId::Wire(a)]);
        assert_eq!(nets.len(), 1);
        // The same net through a part on it, and not twice.
        let both = nets_of(project, &netlist, &[AnyId::Wire(a), AnyId::Symbol(b)]);
        assert_eq!(both, nets);
        let info = describe(project, &netlist, nets[0]);
        assert_eq!(info.wires, 1);
        assert_eq!(info.pages.len(), 1);
        assert_eq!(info.pages[0].0, "1");
        assert!(info.title.contains("Net") || info.title.contains("Wire"));
    }
}

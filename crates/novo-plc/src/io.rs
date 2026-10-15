//! The I/O list: one row per channel, spares included, for the Outputs
//! thread's `io` report and the Devices drawer (plc.md §5, §6).

use novo_core::{AnyId, DeviceId, DeviceTag, Endpoint, Netlist, Project, SymbolRole, Value};

use crate::card::SIGNAL;
use crate::index::PlcIndex;
use crate::layout::ChannelType;

/// One channel of a card, drawn or spare.
#[derive(Debug, Clone, PartialEq)]
pub struct IoRow {
    /// The station's key (its name; empty for the unnamed station), the
    /// section key of the report.
    pub station: String,
    /// The section title: `PLC` or `PLC station <name>`.
    pub station_title: String,
    /// The address in the project's style; `None` when the card has no
    /// start in the channel's space.
    pub address: Option<String>,
    pub kind: ChannelType,
    pub card: DeviceId,
    pub card_tag: String,
    pub slot: Option<u16>,
    /// The channel number of its type (`3` in `DI 3`).
    pub channel: u16,
    /// The channel symbol's pin name; empty for a spare.
    pub pin: String,
    /// The typed `plc.signal`, else the first connected device's
    /// description, else the net's potential; empty for a spare.
    pub signal: String,
    /// Devices on the channel's net (terminals and the card itself left
    /// out), as `-K1:13`, with the pin's symbol for the row's sources.
    pub connected: Vec<(String, AnyId)>,
    /// Terminals on the channel's net, as `-X2:7`.
    pub terminals: Vec<(String, AnyId)>,
    /// Where the channel is drawn (`3.4`) and its symbol; `None` for a spare.
    pub drawn_at: Option<(String, AnyId)>,
    pub spare: bool,
}

impl PlcIndex {
    /// Every channel of every card, spares included, grouped by station in
    /// station order and in address order inside a station (inputs, then
    /// outputs, by byte and bit; channels without an address last, in card
    /// order). A channel drawn more than once is listed once, at its first
    /// symbol; the extra drawings are a problem, not rows.
    pub fn io_rows(&self, project: &Project) -> Vec<IoRow> {
        let netlist = Netlist::build(project);
        let mut rows = Vec::new();
        for (order, card) in self.cards().iter().enumerate() {
            let card_tag = project
                .device(card.device)
                .map(|d| d.tag.to_string())
                .unwrap_or_default();
            for channel in card.channels().channels() {
                let address = card.address(channel.kind, channel.number);
                let drawn = self.drawn(card.device, channel.kind, channel.number);
                let mut row = IoRow {
                    station: card.station.clone(),
                    station_title: station_title(&card.station),
                    address: address.map(|a| a.format(self.style)),
                    kind: channel.kind,
                    card: card.device,
                    card_tag: card_tag.clone(),
                    slot: card.slot,
                    channel: channel.number,
                    pin: String::new(),
                    signal: String::new(),
                    connected: Vec::new(),
                    terminals: Vec::new(),
                    drawn_at: None,
                    spare: drawn.is_empty(),
                };
                if let Some(&symbol) = drawn.first() {
                    fill_drawn(&mut row, project, &netlist, symbol);
                }
                let key = address.map(|a| (a.space(), a.byte, a.bit));
                rows.push((card.station.clone(), key.is_none(), key, order, row));
            }
        }
        rows.sort_by(|a, b| {
            (&a.0, a.1, a.2, a.3, a.4.kind, a.4.channel).cmp(&(
                &b.0,
                b.1,
                b.2,
                b.3,
                b.4.kind,
                b.4.channel,
            ))
        });
        rows.into_iter().map(|r| r.4).collect()
    }
}

fn station_title(station: &str) -> String {
    if station.is_empty() {
        "PLC".into()
    } else {
        format!("PLC station {station}")
    }
}

fn fill_drawn(row: &mut IoRow, project: &Project, netlist: &Netlist, symbol: novo_core::SymbolId) {
    let Some(sym) = project.symbol(symbol) else {
        return;
    };
    row.pin = sym.pins.first().map(|p| p.name.clone()).unwrap_or_default();
    row.drawn_at = project
        .location_of(symbol)
        .map(|l| (l.qualified(&DeviceTag::default()), symbol.into()));
    let mut first_description = None;
    let mut potential = None;
    for pin in 0..sym.pins.len() {
        let Ok(pin) = u16::try_from(pin) else { break };
        let Some(net) = netlist.net_of(Endpoint::pin(symbol, pin)) else {
            continue;
        };
        let net = netlist.net(net);
        if potential.is_none() {
            potential = net.potentials.first().copied();
        }
        for end in &net.endpoints {
            let Some(p) = end.as_pin() else { continue };
            if p.symbol == symbol {
                continue;
            }
            let Some(other) = project.symbol(p.symbol) else {
                continue;
            };
            let pin_name = other
                .pins
                .get(usize::from(p.pin))
                .map(|p| p.name.as_str())
                .unwrap_or("");
            match &other.role {
                SymbolRole::Device { device, .. } if *device != row.card => {
                    let Some(d) = project.device(*device) else {
                        continue;
                    };
                    push_once(
                        &mut row.connected,
                        format!("{}:{pin_name}", d.tag),
                        other.id.into(),
                    );
                    if first_description.is_none() && !d.description.is_empty() {
                        first_description = Some(d.description.clone());
                    }
                }
                SymbolRole::Terminal { terminals } => {
                    for t in terminals.iter().filter_map(|&t| project.terminal(t)) {
                        let Some(strip) = project.device(t.strip) else {
                            continue;
                        };
                        push_once(
                            &mut row.terminals,
                            format!("{}:{}", strip.tag, t.name),
                            t.id.into(),
                        );
                    }
                }
                _ => {}
            }
        }
    }
    row.signal = match sym.properties.get(SIGNAL) {
        Some(Value::Text(t)) if !t.trim().is_empty() => t.trim().to_owned(),
        _ => first_description
            .or_else(|| {
                potential
                    .and_then(|p| project.potential(p))
                    .map(|p| p.name.clone())
            })
            .unwrap_or_default(),
    };
}

fn push_once(list: &mut Vec<(String, AnyId)>, text: String, id: AnyId) {
    if !list.iter().any(|(t, _)| *t == text) {
        list.push((text, id));
    }
}

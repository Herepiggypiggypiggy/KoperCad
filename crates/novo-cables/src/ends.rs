//! Where a wire end is: which device (or terminal strip) it lands on and
//! in which location (`+CAB`, `+FLD`).

use novo_core::{DeviceId, DeviceTag, Endpoint, Project, SymbolRole};

/// The device a pin belongs to: the device of a device symbol, or the strip
/// of a terminal symbol. `None` for junctions and unassigned symbols.
pub fn owner(project: &Project, end: Endpoint) -> Option<DeviceId> {
    let pin = end.as_pin()?;
    let symbol = project.symbol(pin.symbol)?;
    match &symbol.role {
        SymbolRole::Device { device, .. } => Some(*device),
        SymbolRole::Terminal { .. } => {
            let terminal = symbol.terminal_of_pin(pin.pin)?;
            Some(project.terminal(terminal)?.strip)
        }
        _ => None,
    }
}

/// The location levels of a pin: its device's location, or the location of
/// the page it is drawn on when the device tag has none.
pub fn location(project: &Project, end: Endpoint) -> Option<Vec<String>> {
    let device = project.device(owner(project, end)?)?;
    if !device.tag.location.is_empty() {
        return Some(device.tag.location.clone());
    }
    let symbol = project.symbol(end.as_pin()?.symbol)?;
    Some(project.page(symbol.page)?.structure.location.clone())
}

/// A location as text: `+CAB`, `+A1+FLD`; empty for no location.
pub fn location_text(levels: &[String]) -> String {
    DeviceTag {
        location: levels.to_vec(),
        ..DeviceTag::default()
    }
    .to_string()
}

/// How many devices sit in a location. The location with more devices (or
/// with a terminal strip) counts as the cabinet side of a cable.
pub(crate) fn rank(project: &Project, levels: &[String]) -> (bool, usize) {
    let strip = project
        .devices()
        .any(|d| d.kind == novo_core::DeviceKind::TerminalStrip && d.tag.location == levels);
    let count = project
        .devices()
        .filter(|d| d.tag.location == levels)
        .count();
    (strip, count)
}

/// The designation of a pin for a connection list: `=A1+CAB-X1:1`,
/// `=A1+FLD-M1:U1`.
pub fn designation(project: &Project, end: Endpoint) -> Option<String> {
    project.pin_designation(end.as_pin()?)
}

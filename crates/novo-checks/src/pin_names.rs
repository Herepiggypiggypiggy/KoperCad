//! Pin names against the pin template of the device class
//! (`novo_symbols::pin_template`): a coil with `A1 B2`, a motor with `U V W`,
//! or an assigned part whose terminals are not named the way the class
//! usually names them. Written from generic IEC conventions only.

use novo_core::{DeviceId, SymbolInstance, SymbolRole};
use novo_symbols::{Library, Pack};

use crate::{Context, Problem, ProblemKind};

/// The function type of a placed symbol, from the built-in library of its
/// pack. `None` for a symbol the built-in packs do not have.
fn function_of(symbol: &SymbolInstance) -> Option<&'static str> {
    let pack = Pack::of(&symbol.symbol.0)?;
    Some(
        Library::shared(pack)
            .get(&symbol.symbol.0)?
            .function
            .as_str(),
    )
}

/// Pin names of every device symbol, and the terminal names of the parts of
/// each device, against the template of their class.
pub(crate) fn pin_names(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    let mut devices: Vec<(&DeviceId, &Vec<&SymbolInstance>)> = cx.by_device.iter().collect();
    devices.sort_by_key(|(id, _)| **id);
    for (&device, symbols) in devices {
        let mut class_function = None;
        for &symbol in symbols {
            if !matches!(symbol.role, SymbolRole::Device { .. }) {
                continue;
            }
            let Some(function) = function_of(symbol) else {
                continue;
            };
            class_function.get_or_insert(function);
            let pins: Vec<&str> = symbol.pins.iter().map(|p| p.name.as_str()).collect();
            if let Some(d) = novo_symbols::check_pins(function, &pins) {
                out.push(Problem::new(
                    ProblemKind::PinNames,
                    vec![symbol.id.into()],
                    format!("{}: {}", cx.symbol_label(symbol), d.message()),
                ));
            }
        }
        // The terminals of an assigned part are only compared with a class
        // when the device has a single one: a contactor with an auxiliary
        // block has parts whose terminals belong to different functions.
        let (Some(function), Some(dev)) = (class_function, project.device(device)) else {
            continue;
        };
        let [part_id] = dev.parts.as_slice() else {
            continue;
        };
        let Some(part) = project.part(*part_id) else {
            continue;
        };
        let names = novo_parts::placed_terminals(&part.properties);
        if names.is_empty() {
            continue;
        }
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        if let Some(d) = novo_symbols::check_pins(function, &names) {
            out.push(Problem::new(
                ProblemKind::PinNames,
                symbols.iter().map(|s| s.id.into()).collect(),
                format!("{}: part terminals: {}", dev.tag, d.message()),
            ));
        }
    }
}

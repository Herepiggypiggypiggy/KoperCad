//! Pin names against the pin template of the device class
//! (`novo_symbols::pin_template`): a coil with `A1 B2`, a motor with `U V W`,
//! or an assigned part whose terminals are not named the way the class
//! usually names them. Written from generic IEC conventions only.

use novo_core::{AnyId, Command, DeviceId, Project, Record, SymbolInstance, SymbolRole, Value};
use novo_symbols::{Library, Pack, PinTemplate};

/// The project property that holds the company pin templates as TOML text
/// (`novo_symbols::parse_company_templates`). Set from Project > Rules.
pub const PIN_TEMPLATES_PROPERTY: &str = "pin.templates";

/// The text of the project's company pin templates, when it has some.
fn company_text(project: &Project) -> Option<&str> {
    match project.info().properties.get(PIN_TEMPLATES_PROPERTY)? {
        Value::Text(t) if !t.trim().is_empty() => Some(t),
        _ => None,
    }
}

/// The company templates of the project, or the plain reason the stored
/// text cannot be read (a project file edited by hand).
fn company_templates(project: &Project) -> Result<Vec<PinTemplate>, String> {
    company_text(project).map_or(Ok(Vec::new()), novo_symbols::parse_company_templates)
}

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
    let company = match company_templates(project) {
        Ok(c) => c,
        Err(e) => {
            out.push(Problem::new(
                ProblemKind::PinNames,
                Vec::new(),
                format!("The company pin templates of this project cannot be read: {e}"),
            ));
            Vec::new()
        }
    };
    let mut devices: Vec<(&DeviceId, &Vec<&SymbolInstance>)> = cx.by_device.iter().collect();
    devices.sort_by_key(|(id, _)| **id);
    for (&device, symbols) in devices {
        // Every drawn function of the device, to compare a part only when
        // they all belong to one class.
        let mut functions: Vec<&'static str> = Vec::new();
        let mut unknown = false;
        for &symbol in symbols {
            if !matches!(symbol.role, SymbolRole::Device { .. }) {
                continue;
            }
            let Some(function) = function_of(symbol) else {
                unknown = true;
                continue;
            };
            functions.push(function);
            let pins: Vec<&str> = symbol.pins.iter().map(|p| p.name.as_str()).collect();
            if let Some(d) = novo_symbols::check_pins_with(&company, function, &pins) {
                out.push(Problem::new(
                    ProblemKind::PinNames,
                    vec![symbol.id.into()],
                    format!("{}: {}", cx.symbol_label(symbol), d.message()),
                ));
            }
        }
        // The terminals of an assigned part are compared with a class only
        // when every symbol of the device is of that class: a contactor is a
        // coil plus main contacts on one part, whose terminals belong to
        // several functions.
        let Some(&function) = functions.first().filter(|_| !unknown) else {
            continue;
        };
        let class = novo_symbols::template_with(&company, function).map(|t| t.class.as_str());
        if class.is_none()
            || functions.iter().any(|f| {
                novo_symbols::template_with(&company, f).map(|t| t.class.as_str()) != class
            })
        {
            continue;
        }
        let Some(dev) = project.device(device) else {
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
        if let Some(d) = novo_symbols::check_pins_with(&company, function, &names) {
            out.push(Problem::new(
                ProblemKind::PinNames,
                symbols.iter().map(|s| s.id.into()).collect(),
                format!("{}: part terminals: {}", dev.tag, d.message()),
            ));
        }
    }
}

/// The one function type all device symbols of `symbols` share (as a
/// template class), or `None` when they differ or one is unknown.
fn shared_function(company: &[PinTemplate], symbols: &[&SymbolInstance]) -> Option<&'static str> {
    let mut found: Option<(&'static str, String)> = None;
    for symbol in symbols
        .iter()
        .filter(|s| matches!(s.role, SymbolRole::Device { .. }))
    {
        let function = function_of(symbol)?;
        let class = novo_symbols::template_with(company, function)?
            .class
            .clone();
        match found {
            Some((_, ref c)) if *c != class => return None,
            Some(_) => {}
            None => found = Some((function, class)),
        }
    }
    found.map(|(f, _)| f)
}

/// A repair for a [`PinNames`](ProblemKind::PinNames) problem.
#[derive(Debug, Clone, PartialEq)]
pub struct PinFix {
    /// What the repair does, in a few plain words.
    pub label: String,
    /// One undo step.
    pub command: Command,
    /// A line to show beside the entry (a tooltip), when the user should know
    /// more before choosing: a part's terminals are renamed for every device
    /// that uses the part.
    pub note: Option<String>,
}

/// The repair for a pin-names problem, when the right names are clear:
/// renames the pins of the marked symbols to the usual names of their class,
/// or, when the pins are fine and the assigned part's terminals are not,
/// renames the part's terminals to the same names so they map onto the
/// symbol pins. `None` for other problems and for names that cannot be told
/// with certainty (see `novo_symbols::suggest_renames`).
///
/// A part is shared by every device that uses it, so renaming its terminals
/// renames them for all of them.
pub fn pin_fix(project: &Project, problem: &Problem) -> Option<PinFix> {
    if problem.kind != ProblemKind::PinNames {
        return None;
    }
    let symbols: Vec<&SymbolInstance> = problem
        .at
        .iter()
        .filter_map(|id| match id {
            AnyId::Symbol(s) => project.symbol(*s),
            _ => None,
        })
        .collect();
    let company = company_templates(project).unwrap_or_default();
    let mut commands = Vec::new();
    let mut renamed = 0;
    for &symbol in &symbols {
        let Some(function) = function_of(symbol) else {
            continue;
        };
        let names: Vec<&str> = symbol.pins.iter().map(|p| p.name.as_str()).collect();
        let Some(fix) = novo_symbols::suggest_renames_with(&company, function, &names) else {
            continue;
        };
        let mut next = symbol.clone();
        for pin in &mut next.pins {
            if let Some((_, new)) = fix.renames.iter().find(|(old, _)| *old == pin.name) {
                pin.name = new.clone();
            }
        }
        renamed += fix.renames.len();
        commands.push(Command::Replace {
            record: Record::Symbol(next),
        });
    }
    if !commands.is_empty() {
        return Some(PinFix {
            label: format!("Rename {} to the usual names", count(renamed, "pin")),
            note: None,
            command: Command::Batch {
                label: "Rename pins to the usual names".into(),
                commands,
            },
        });
    }
    // The pins follow the template: look at the terminals of the part.
    let function = shared_function(&company, &symbols)?;
    let device = symbols.iter().find_map(|s| match s.role {
        SymbolRole::Device { device, .. } => project.device(device),
        _ => None,
    })?;
    let [part_id] = device.parts.as_slice() else {
        return None;
    };
    let part = project.part(*part_id)?;
    let names = novo_parts::placed_terminals(&part.properties);
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let fix = novo_symbols::suggest_renames_with(&company, function, &names)?;
    let mut next = part.clone();
    for (old, new) in &fix.renames {
        // Every property of the terminal moves with it: the position, and
        // whatever else is stored under `pin.<name>.` (wire range, torque).
        // A suffix with a dot in it belongs to a terminal whose own name has
        // a dot (`1.1`), not to this one.
        let from = format!("pin.{old}.");
        let moved: Vec<String> = next
            .properties
            .keys()
            .filter(|k| {
                k.strip_prefix(&from)
                    .is_some_and(|rest| !rest.contains('.'))
            })
            .cloned()
            .collect();
        for key in moved {
            if let Some(value) = next.properties.remove(&key) {
                let rest = &key[from.len()..];
                next.properties.insert(format!("pin.{new}.{rest}"), value);
            }
        }
    }
    Some(PinFix {
        label: format!(
            "Rename {} to the usual names",
            count(fix.renames.len(), "part terminal")
        ),
        command: Command::Replace {
            record: Record::Part(next),
        },
        note: Some(
            "The part is shared by every device that uses it, so its terminals are renamed \
             for all of them."
                .into(),
        ),
    })
}

/// `1 pin`, `3 pins`.
fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

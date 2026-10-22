//! The edits the assistant can make. Each one runs inside a `Tx` on a
//! draft copy of the project and, when the person applies the change set,
//! again inside one transaction on the real project, so the arguments are
//! written in the names an engineer uses (page names, `-K1` tags, `tag:pin`
//! addresses), never in ids that only exist in one copy.

use super::resolve::{self, text_of};
use crate::tools::EditAction;
use novo_core::{
    AnyId, DeviceTag, ModelError, Placement, Point, PotentialKind, Project, Rotation, Tx,
    Value as ModelValue,
};
use novo_editor::edit::{self, PlaceOptions, Transform};
use novo_editor::geometry::Target;
use novo_parts::MacroFile;
use novo_symbols::Library;
use serde_json::{Value, json};

/// What an action needs besides the transaction.
pub struct Env<'a> {
    pub library: &'a Library,
    pub macros: &'a [MacroFile],
}

fn reason(action: &str, e: impl std::fmt::Display) -> ModelError {
    ModelError::Refused {
        what: action.to_string(),
        reason: e.to_string(),
    }
}

/// Runs one action. `Err` carries a sentence for the model.
pub fn run(tx: &mut Tx<'_>, env: &Env<'_>, action: &str, args: &Value) -> Result<String, String> {
    match action {
        "add_page" => add_page(tx, args),
        "rename_page" => rename_page(tx, args),
        "add_device" => add_device(tx, args),
        "set_page" => set_page(tx, args),
        "place_macro" => place_macro(tx, env, args),
        "place_symbol" => place_symbol(tx, env, args),
        "connect" => connect(tx, args),
        "terminate_pin" => terminate_pin(tx, env, args),
        "set_property" => set_property(tx, args),
        "describe_device" => describe_device(tx, args),
        "select_standards" => select_standards(tx, args),
        "rename_device" => rename_device(tx, args),
        "assign_part" => assign_part(tx, args),
        "set_wire_number" => set_wire_number(tx, args),
        "number_wires" => number_wires(tx),
        "remove_device" => remove_device(tx, args),
        "move_device" => move_device(tx, args),
        other => Err(format!("unknown action {other:?}")),
    }
}

/// Runs an action as its own step on `project` (a draft), returning what it did.
pub fn run_on(
    project: &mut Project,
    env: &Env<'_>,
    action: &str,
    args: &Value,
) -> Result<String, String> {
    project
        .transact(format!("Claude: {action}"), |tx| {
            run(tx, env, action, args).map_err(|e| reason(action, e))
        })
        .map(|(summary, _)| summary)
        .map_err(|e| match e {
            ModelError::Refused { reason, .. } => reason,
            other => other.to_string(),
        })
}

fn need(args: &Value, key: &str) -> Result<String, String> {
    text_of(&args[key]).ok_or_else(|| format!("{key} is missing"))
}

fn num(args: &Value, key: &str) -> Result<Option<f64>, String> {
    match &args[key] {
        Value::Null => Ok(None),
        v => v
            .as_f64()
            .filter(|x| x.is_finite() && x.abs() < 100_000.0)
            .map(Some)
            .ok_or_else(|| format!("{key} must be a number in millimetres")),
    }
}

/// All actions with their argument schemas, as shown to the model.
pub fn catalogue() -> Vec<EditAction> {
    let page =
        json!({"type": "string", "description": "page name, e.g. \"1\" (see project_outline)"});
    let device = json!({"type": "string", "description": "device tag such as -K1"});
    let pin = json!({"type": ["string", "object"], "description":
        "tag:pin such as -K1:A1; add #2 when the device has two symbols with that pin name; \
         terminals are -X1:3.1 (upper pin) or -X1:3.2 (lower pin)"});
    vec![
        EditAction {
            name: "add_page",
            description: "Adds an empty schematic page at the end.",
            schema: json!({"type": "object", "properties": {
                "name": {"type": "string"}, "title": {"type": "string"}}, "required": ["name"]}),
        },
        EditAction {
            name: "rename_page",
            description: "Renames a page (references print the page name).",
            schema: json!({"type": "object", "properties": {
                "page": page, "name": {"type": "string"}}, "required": ["page", "name"]}),
        },
        EditAction {
            name: "add_device",
            description: "Adds a device with a tag and description that is not drawn on a page \
                          yet (rarely what you want: place_symbol and place_macro create devices).",
            schema: json!({"type": "object", "properties": {
                "tag": {"type": "string"}, "description": {"type": "string"}}, "required": ["tag"]}),
        },
        EditAction {
            name: "set_page",
            description: "Renames a page and/or sets its title.",
            schema: json!({"type": "object", "properties": {
                "page": page, "name": {"type": "string"}, "title": {"type": "string"}},
                "required": ["page"]}),
        },
        EditAction {
            name: "place_macro",
            description: "Places a ready-made circuit from list_macros (motor starter, reversing \
                          starter, E-stop chain, PLC input card, power supply ...). It comes \
                          wired inside, with tags and wire numbers assigned. Prefer this to \
                          drawing a standard circuit symbol by symbol. Returns the tags it \
                          created and the pins left open to connect.",
            schema: json!({"type": "object", "properties": {
                "macro": {"type": "string", "description": "name from list_macros"},
                "page": page,
                "x": {"type": "number"}, "y": {"type": "number"},
                "parameters": {"type": "object", "description": "macro parameters by name"}},
                "required": ["macro", "page"]}),
        },
        EditAction {
            name: "place_symbol",
            description: "Places one library symbol (see list_symbols) as a tagged device with the \
                          next free tag for its letter. Pins that land on a wire connect.",
            schema: json!({"type": "object", "properties": {
                "symbol": {"type": "string", "description": "id such as iec/coils/relay, or an alias"},
                "page": page,
                "x": {"type": "number", "description": "mm; leave out x and y to put it in free space right of the drawing"},
                "y": {"type": "number"},
                "rotation": {"type": "integer", "enum": [0, 90, 180, 270]},
                "parent": {"type": "string", "description":
                    "give an existing tag (-K1) to add a contact or coil function to that device"},
                "potential": {"type": "string", "description": "for feed symbols: L1, L2, L3, N, PE, L+, L-"}},
                "required": ["symbol", "page"]}),
        },
        EditAction {
            name: "connect",
            description: "Draws a wire between two pins on the same page, routed at right angles.",
            schema: json!({"type": "object", "properties": {"from": pin, "to": pin,
                "via": {"type": "array", "description": "optional corner points [[x, y], ...] in mm; \
                         without them the program tries the direct route and then a route around the drawing"}},
                "required": ["from", "to"]}),
        },
        EditAction {
            name: "terminate_pin",
            description: "Ends an open pin on a terminal: places a terminal on the terminal strip \
                          just beyond the pin (in the direction the wire leaves it) and wires \
                          it. With potential (L1, L2, L3, N, PE, 24V ...) the terminal is the \
                          incoming feed of that potential. Use it to complete supply and \
                          field connections.",
            schema: json!({"type": "object", "properties": {
                "pin": pin,
                "potential": {"type": "string"},
                "distance": {"type": "number", "description": "mm from the pin, default 25"}},
                "required": ["pin"]}),
        },
        EditAction {
            name: "set_property",
            description: "Sets one named property of a device (text, number or true/false).",
            schema: json!({"type": "object", "properties": {
                "device": device, "key": {"type": "string"},
                "value": {"type": ["string", "number", "boolean"]}},
                "required": ["device", "key", "value"]}),
        },
        EditAction {
            name: "describe_device",
            description: "Sets the description text of a device (what the Device list and labels show).",
            schema: json!({"type": "object", "properties": {
                "device": device, "text": {"type": "string"}}, "required": ["device", "text"]}),
        },
        EditAction {
            name: "select_standards",
            description: "Chooses which standards the program's standards report checks the \
                          project against: any of iec60204, nfpa79, ul508a (empty list turns \
                          it off). Only change this when the person named a standard.",
            schema: json!({"type": "object", "properties": {
                "ids": {"type": "array", "items": {"type": "string", "enum": ["iec60204", "nfpa79", "ul508a"]}}},
                "required": ["ids"]}),
        },
        EditAction {
            name: "rename_device",
            description: "Gives a device a new tag everywhere it is used.",
            schema: json!({"type": "object", "properties": {
                "device": device, "tag": {"type": "string", "description": "IEC 81346 tag such as -K7"}},
                "required": ["device", "tag"]}),
        },
        EditAction {
            name: "assign_part",
            description: "Assigns a part that is already in the project (section parts) to a \
                          device. Never invent a part or order number.",
            schema: json!({"type": "object", "properties": {
                "device": device,
                "part": {"type": "string", "description": "\"Manufacturer OrderNumber\" or the order number"}},
                "required": ["device", "part"]}),
        },
        EditAction {
            name: "set_wire_number",
            description: "Sets the number printed on the wire at a pin (and its whole net); an \
                          empty number clears it. The number is then fixed against numbering \
                          runs. Give pin (tag:pin) or wire_id from section wires.",
            schema: json!({"type": "object", "properties": {
                "pin": pin, "wire_id": {"type": "integer"}, "number": {"type": "string"}},
                "required": ["number"]}),
        },
        EditAction {
            name: "number_wires",
            description: "Numbers every wire that has no number yet with the project's numbering \
                          scheme, continuing the existing series. Existing numbers are not touched.",
            schema: json!({"type": "object", "properties": {}}),
        },
        EditAction {
            name: "remove_device",
            description: "Deletes a device with all its symbols and the wires on them.",
            schema: json!({"type": "object", "properties": {"device": device}, "required": ["device"]}),
        },
        EditAction {
            name: "move_device",
            description: "Moves all symbols of a device by dx, dy millimetres (positive is right and down).",
            schema: json!({"type": "object", "properties": {
                "device": device, "dx": {"type": "number"}, "dy": {"type": "number"}},
                "required": ["device", "dx", "dy"]}),
        },
    ]
}

fn add_page(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let name = need(args, "name")?;
    if tx.project().pages().any(|p| p.name == name) {
        return Err(format!("a page {name} already exists"));
    }
    let title = text_of(&args["title"]).unwrap_or_default();
    tx.execute(novo_core::Command::AddPage {
        name: name.clone(),
        title: title.clone(),
        kind: Default::default(),
        format: Default::default(),
        structure: Default::default(),
    })
    .map_err(|e| e.to_string())?;
    Ok(if title.is_empty() {
        format!("Add page {name}")
    } else {
        format!("Add page {name} ({title})")
    })
}

fn set_page(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let page = resolve::page_arg(tx.project(), args)?;
    let old = tx
        .project()
        .page(page)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let mut said = Vec::new();
    if let Some(name) = text_of(&args["name"]) {
        if tx.project().pages().any(|p| p.name == name && p.id != page) {
            return Err(format!("a page {name} already exists"));
        }
        tx.execute(novo_core::Command::RenamePage {
            page,
            name: name.clone(),
        })
        .map_err(|e| e.to_string())?;
        said.push(format!("name {name}"));
    }
    if let Some(title) = args["title"].as_str() {
        tx.update::<novo_core::Page>(page, |p| p.title = title.trim().to_string())
            .map_err(|e| e.to_string())?;
        said.push(format!("title {:?}", title.trim()));
    }
    if said.is_empty() {
        return Err("give a name and/or a title".into());
    }
    Ok(format!("Page {old}: set {}", said.join(", ")))
}

fn origin(project: &Project, page: novo_core::PageId, args: &Value) -> Result<Point, String> {
    match (num(args, "x")?, num(args, "y")?) {
        (Some(x), Some(y)) => Ok(Point::new(x, y)),
        (None, None) => Ok(novo_workflow::macro_place::free_origin(project, page, 30.0)),
        _ => Err("give both x and y, or neither".into()),
    }
}

fn place_macro(tx: &mut Tx<'_>, env: &Env<'_>, args: &Value) -> Result<String, String> {
    let wanted = need(args, "macro")?;
    let file = env
        .macros
        .iter()
        .find(|m| m.name.eq_ignore_ascii_case(&wanted))
        .ok_or_else(|| {
            let names: Vec<&str> = env.macros.iter().map(|m| m.name.as_str()).collect();
            format!("there is no macro {wanted:?}. Macros: {names:?}")
        })?;
    let page = resolve::page_arg(tx.project(), args)?;
    let at = origin(tx.project(), page, args)?;
    let mut options = novo_workflow::macro_place::PlaceOptions {
        parts: file
            .extra
            .get("parts")
            .map(novo_workflow::macro_place::parts_from_json)
            .unwrap_or_default(),
        values: file
            .definition
            .parameters
            .iter()
            .map(|p| (p.name.clone(), p.default.clone()))
            .collect(),
        wire_numbers: novo_workflow::macro_place::WireNumbers::Auto,
    };
    if let Some(given) = args["parameters"].as_object() {
        for (k, v) in given {
            if !file.definition.parameters.iter().any(|p| &p.name == k) {
                let names: Vec<&str> = file
                    .definition
                    .parameters
                    .iter()
                    .map(|p| p.name.as_str())
                    .collect();
                return Err(format!(
                    "macro {wanted:?} has no parameter {k:?}. Parameters: {names:?}"
                ));
            }
            let value = to_model_value(v)
                .ok_or_else(|| format!("parameter {k} must be text, a number or true/false"))?;
            options.values.retain(|(n, _)| n != k);
            options.values.push((k.clone(), value));
        }
    }
    let before: std::collections::BTreeSet<u64> =
        tx.project().symbols().map(|s| s.id.raw()).collect();
    let placed =
        novo_workflow::macro_place::place_macro(tx, file.definition.clone(), page, at, &options)
            .map_err(|e| e.to_string())?;
    let mut tags: Vec<String> = placed.report.tags.iter().map(ToString::to_string).collect();
    tags.sort();
    let pname = tx
        .project()
        .page(page)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let open = open_pins_since(tx.project(), &before);
    let mut s = format!(
        "Place macro {} on page {pname}: devices {}",
        file.name,
        tags.join(", ")
    );
    if !open.is_empty() {
        s.push_str(&format!(
            ". Pins left open for you to connect: {}",
            open.join(", ")
        ));
    }
    Ok(s)
}

/// Pins on the symbols that were not there before that have no wire:
/// `-K1:A2` for a device, `{"symbol_id":57,"pin":"1"}` for a terminal or a
/// symbol without a device.
fn open_pins_since(project: &Project, before: &std::collections::BTreeSet<u64>) -> Vec<String> {
    let mut out = Vec::new();
    for s in project.symbols().filter(|s| !before.contains(&s.id.raw())) {
        for (i, p) in s.pins.iter().enumerate() {
            if p.optional
                || project
                    .wires_at(novo_core::Endpoint::pin(s.id, i as u16))
                    .next()
                    .is_some()
            {
                continue;
            }
            out.push(match s.device().and_then(|d| project.device(d)) {
                Some(d) => format!("{}:{}", d.tag, p.name),
                None => format!(
                    "{} pin {} (symbol {})",
                    super::resolve::friendly_symbol(&s.symbol.0),
                    p.name,
                    s.id.raw()
                ),
            });
        }
    }
    out
}

fn to_model_value(v: &Value) -> Option<ModelValue> {
    Some(match v {
        Value::String(s) => ModelValue::Text(s.clone()),
        Value::Bool(b) => ModelValue::Bool(*b),
        Value::Number(n) => match n.as_i64() {
            Some(i) => ModelValue::Int(i),
            None => ModelValue::Float(n.as_f64()?),
        },
        _ => return None,
    })
}

fn potential_kind(name: &str) -> PotentialKind {
    match name.to_ascii_uppercase().as_str() {
        "N" => PotentialKind::Neutral,
        "PE" => PotentialKind::ProtectiveEarth,
        "PEN" => PotentialKind::Pen,
        "L+" | "24V" | "+24V" => PotentialKind::DcPositive,
        "L-" | "M" | "0V" => PotentialKind::DcNegative,
        n if n.starts_with('L') => PotentialKind::Line,
        _ => PotentialKind::Signal,
    }
}

fn place_symbol(tx: &mut Tx<'_>, env: &Env<'_>, args: &Value) -> Result<String, String> {
    let wanted = need(args, "symbol")?;
    let symbol = env
        .library
        .get(&wanted)
        .or_else(|| env.library.by_alias(&wanted))
        .ok_or_else(|| format!("there is no symbol {wanted:?}; use list_symbols to search"))?;
    let page = resolve::page_arg(tx.project(), args)?;
    let at = origin(tx.project(), page, args)?;
    let rotation = match args["rotation"].as_u64().unwrap_or(0) {
        0 => Rotation::R0,
        90 => Rotation::R90,
        180 => Rotation::R180,
        270 => Rotation::R270,
        other => return Err(format!("rotation must be 0, 90, 180 or 270, not {other}")),
    };
    let mut options = PlaceOptions::new();
    if let Some(parent) = text_of(&args["parent"]) {
        options.parent =
            Some(DeviceTag::parse(&parent).map_err(|e| format!("parent {parent:?}: {e}"))?);
    }
    if let Some(name) = text_of(&args["potential"]) {
        let (id, _) = tx
            .ensure_potential(novo_core::PotentialSpec::new(
                &name,
                potential_kind(&name),
                None,
            ))
            .map_err(|e| e.to_string())?;
        options.potential = Some(id);
    }
    let placement = Placement {
        origin: at,
        rotation,
        mirrored: false,
    };
    let id = edit::place(tx, page, symbol, placement, &options).map_err(|e| e.to_string())?;
    let project = tx.project();
    let s = project.symbol(id).ok_or("the symbol was not placed")?;
    let who = s
        .device()
        .and_then(|d| project.device(d))
        .map(|d| d.tag.to_string())
        .unwrap_or_else(|| "no device".into());
    let pins: Vec<&str> = s.pins.iter().map(|p| p.name.as_str()).collect();
    let pname = project
        .page(page)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    Ok(format!(
        "Place {} as {who} on page {pname} at ({:.0}, {:.0}) mm; pins {}",
        symbol.name,
        at.x,
        at.y,
        pins.join(", ")
    ))
}

fn via_points(args: &Value) -> Result<Option<Vec<Point>>, String> {
    let Some(list) = args["via"].as_array() else {
        return Ok(None);
    };
    list.iter()
        .map(|p| {
            let xy = p.as_array().filter(|a| a.len() == 2);
            match xy.and_then(|a| Some((a[0].as_f64()?, a[1].as_f64()?))) {
                Some((x, y)) if x.abs() < 100_000.0 && y.abs() < 100_000.0 => Ok(Point::new(x, y)),
                _ => Err("via must be a list of [x, y] points in millimetres".to_string()),
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn connect(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let from = resolve::pin(tx.project(), &args["from"])?;
    let to = resolve::pin(tx.project(), &args["to"])?;
    if from.page != to.page {
        return Err(
            "the two pins are on different pages; wires stay on one page. Use an \
                    interruption point or a terminal, or place both on the same page"
                .into(),
        );
    }
    if from.end == to.end {
        return Err("a wire needs two different pins".into());
    }
    let (start, end) = (
        Target::Endpoint(from.end, from.at),
        Target::Endpoint(to.end, to.at),
    );
    // The straight orthogonal route first; if another wire is in the way, a
    // channel just outside the drawing. A route the person gave is the only one tried.
    let mut candidates: Vec<(Vec<Point>, Option<f64>)> = Vec::new();
    match via_points(args)? {
        Some(v) => candidates.push((v, None)),
        None => {
            candidates.push((Vec::new(), None));
            let right = novo_workflow::macro_place::free_origin(tx.project(), from.page, 0.0).x;
            for k in 1..=4 {
                let x = right + 10.0 * f64::from(k);
                candidates.push((
                    vec![Point::new(x, from.at.y), Point::new(x, to.at.y)],
                    Some(x),
                ));
            }
        }
    }
    let mut routed = None;
    for (corners, channel) in candidates {
        if edit::check_draw(tx.project(), from.page, start, &corners, end).is_ok() {
            routed = Some((corners, channel));
            break;
        }
    }
    let Some((corners, channel)) = routed else {
        return Err(
            "no clean route: every route tried would run along another wire. Give via \
                    points ([[x, y], ...] in mm) that go through free space, or move one of the \
                    devices first"
                .into(),
        );
    };
    edit::draw_wire(tx, from.page, start, &corners, end).map_err(|e| e.to_string())?;
    let name = |v: &Value| match v {
        Value::String(s) => s.clone(),
        other => format!("{} pin {}", other["device"], other["pin"]),
    };
    let how = match channel {
        Some(x) => format!(" (routed around the drawing at x = {x:.0} mm)"),
        None if !corners.is_empty() => " (via your points)".to_string(),
        None => String::new(),
    };
    Ok(format!(
        "Wire {} to {}{how}",
        name(&args["from"]),
        name(&args["to"])
    ))
}

fn set_property(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let device = resolve::device_arg(tx.project(), args)?;
    let tag = tx
        .project()
        .device(device)
        .map(|d| d.tag.to_string())
        .unwrap_or_default();
    let key = need(args, "key")?;
    if key.chars().count() > 60 {
        return Err("key is longer than 60 characters".into());
    }
    if let Value::String(s) = &args["value"]
        && s.chars().count() > 500
    {
        return Err("value is longer than 500 characters".into());
    }
    let value =
        to_model_value(&args["value"]).ok_or("value must be text, a number or true/false")?;
    let shown = match &args["value"] {
        Value::String(s) => format!("{s:?}"),
        other => other.to_string(),
    };
    tx.set_property(AnyId::Device(device), key.clone(), Some(value))
        .map_err(|e| e.to_string())?;
    Ok(format!("Set {key} of {tag} to {shown}"))
}

fn rename_device(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let device = resolve::device_arg(tx.project(), args)?;
    let old = tx
        .project()
        .device(device)
        .map(|d| d.tag.to_string())
        .unwrap_or_default();
    let tag = need(args, "tag")?;
    let parsed = DeviceTag::parse(&tag).map_err(|e| format!("tag {tag:?}: {e}"))?;
    if let Some(why) = novo_editor::naming::tag_conflict(tx.project(), device, &parsed) {
        return Err(why);
    }
    edit::rename(tx, device, parsed).map_err(|e| e.to_string())?;
    Ok(format!("Rename device {old} to {tag}"))
}

fn assign_part(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let device = resolve::device_arg(tx.project(), args)?;
    let tag = tx
        .project()
        .device(device)
        .map(|d| d.tag.to_string())
        .unwrap_or_default();
    let part = resolve::part_arg(tx.project(), args)?;
    let name = tx
        .project()
        .part(part)
        .map(|p| {
            format!("{} {}", p.manufacturer, p.order_number)
                .trim()
                .to_string()
        })
        .unwrap_or_default();
    tx.assign_part(device, part).map_err(|e| e.to_string())?;
    Ok(format!("Assign part {name} to {tag}"))
}

fn set_wire_number(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let wire = if let Some(id) = args["wire_id"].as_u64() {
        tx.project()
            .wire(id.into())
            .map(|w| w.id)
            .ok_or_else(|| format!("there is no wire with id {id} (see section wires)"))?
    } else {
        let at = resolve::pin(tx.project(), &args["pin"])?;
        let wires: Vec<_> = tx.project().wires_at(at.end).collect();
        match wires.as_slice() {
            [one] => *one,
            [] => return Err("there is no wire on that pin yet; connect it first".into()),
            _ => {
                return Err(
                    "several wires leave that pin; number the net at a pin with one wire".into(),
                );
            }
        }
    };
    let number = match &args["number"] {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => return Err("number must be text; use an empty text to clear it".into()),
    };
    if number.chars().count() > 40 {
        return Err("number is longer than 40 characters".into());
    }
    if number.chars().any(char::is_control) {
        return Err("number must not contain control characters or line breaks".into());
    }
    let name = tx
        .project()
        .wire(wire)
        .map(|w| resolve::wire_description(tx.project(), w))
        .unwrap_or_default();
    novo_numbering::set_number(tx, wire, &number).map_err(|e| e.to_string())?;
    Ok(if number.is_empty() {
        format!("Clear the number of {name} and its net")
    } else {
        format!(
            "Number {name} and its net {number}. The number is fixed: numbering runs leave it alone"
        )
    })
}

fn number_wires(tx: &mut Tx<'_>) -> Result<String, String> {
    let n = novo_numbering::number_new_wires(tx).map_err(|e| e.to_string())?;
    Ok(format!("Number {n} wires that had no number"))
}

fn device_symbols(project: &Project, device: novo_core::DeviceId) -> Vec<AnyId> {
    project
        .symbols_of_device(device)
        .into_iter()
        .map(|s| AnyId::Symbol(s.id))
        .collect()
}

fn remove_device(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let device = resolve::device_arg(tx.project(), args)?;
    let tag = tx
        .project()
        .device(device)
        .map(|d| d.tag.to_string())
        .unwrap_or_default();
    let symbols = device_symbols(tx.project(), device);
    let wires_before = tx.project().wires().count();
    if symbols.is_empty() {
        tx.remove(AnyId::Device(device))
            .map_err(|e| e.to_string())?;
    } else {
        edit::delete(tx, &symbols).map_err(|e| e.to_string())?;
    }
    let gone = wires_before.saturating_sub(tx.project().wires().count());
    Ok(format!(
        "Delete device {tag} with {} and {}",
        plural(symbols.len(), "symbol"),
        plural(gone, "wire")
    ))
}

fn move_device(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let device = resolve::device_arg(tx.project(), args)?;
    let tag = tx
        .project()
        .device(device)
        .map(|d| d.tag.to_string())
        .unwrap_or_default();
    let (dx, dy) = (
        num(args, "dx")?.ok_or("dx is missing")?,
        num(args, "dy")?.ok_or("dy is missing")?,
    );
    let symbols = device_symbols(tx.project(), device);
    if symbols.is_empty() {
        return Err(format!("{tag} is not drawn on any page"));
    }
    edit::transform(tx, &symbols, Transform::Move(Point::new(dx, dy)))
        .map_err(|e| e.to_string())?;
    Ok(format!("Move {tag} by ({dx:.0}, {dy:.0}) mm"))
}

fn opposite(d: novo_core::Direction) -> novo_core::Direction {
    use novo_core::Direction::*;
    match d {
        Up => Down,
        Down => Up,
        Left => Right,
        Right => Left,
    }
}

fn terminate_pin(tx: &mut Tx<'_>, env: &Env<'_>, args: &Value) -> Result<String, String> {
    let pin = resolve::pin(tx.project(), &args["pin"])?;
    if tx.project().wires_at(pin.end).next().is_some() {
        return Err("that pin already has a wire".into());
    }
    let leaves = novo_editor::geometry::endpoint_direction(tx.project(), pin.end)
        .ok_or("that pin has no wire direction")?;
    let distance = num(args, "distance")?.unwrap_or(25.0).clamp(10.0, 200.0);
    let v = leaves.vector();
    let snap = |x: f64| (x / 2.5).round() * 2.5;
    let spot = Point::new(
        snap(pin.at.x + v.x * distance),
        snap(pin.at.y + v.y * distance),
    );
    let potential = text_of(&args["potential"]);
    let key = match potential.as_deref().map(str::to_ascii_uppercase).as_deref() {
        Some("PE") => "iec/terminals/terminal_pe_end",
        Some("N") => "iec/terminals/terminal_n_end",
        _ => "iec/terminals/terminal_end",
    };
    let symbol = env
        .library
        .get(key)
        .ok_or_else(|| format!("symbol {key} is missing"))?;
    let mut options = PlaceOptions::new();
    options.auto_connect = false;
    let id = edit::place(tx, pin.page, symbol, Placement::at(spot), &options)
        .map_err(|e| e.to_string())?;
    for _ in 0..4 {
        let faces = tx.project().symbol(id).and_then(|s| s.pin_direction(0));
        if faces == Some(opposite(leaves)) {
            break;
        }
        edit::transform(tx, &[AnyId::Symbol(id)], Transform::Rotate(spot))
            .map_err(|e| e.to_string())?;
    }
    let end = novo_core::Endpoint::pin(id, 0);
    let to = tx
        .project()
        .endpoint_position(end)
        .ok_or("the terminal has no position")?;
    edit::draw_wire(
        tx,
        pin.page,
        Target::Endpoint(pin.end, pin.at),
        &[],
        Target::Endpoint(end, to),
    )
    .map_err(|e| e.to_string())?;
    let mut said = String::new();
    if let Some(name) = &potential {
        let (pot, _) = tx
            .ensure_potential(novo_core::PotentialSpec::new(
                name,
                potential_kind(name),
                None,
            ))
            .map_err(|e| e.to_string())?;
        let terminals = match tx.project().symbol(id).map(|s| s.role.clone()) {
            Some(novo_core::SymbolRole::Terminal { terminals }) => terminals,
            _ => Vec::new(),
        };
        for t in terminals {
            tx.set_terminal_feed(t, Some(pot))
                .map_err(|e| e.to_string())?;
        }
        said = format!(" feeding {name}");
    }
    let project = tx.project();
    let label = project
        .symbol(id)
        .and_then(|s| match &s.role {
            novo_core::SymbolRole::Terminal { terminals } => terminals.first().copied(),
            _ => None,
        })
        .and_then(|t| project.terminal_designation(t))
        .unwrap_or_else(|| "a terminal".into());
    let from = match &args["pin"] {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    Ok(format!("Terminate {from} on terminal {label}{said}"))
}

fn describe_device(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let device = resolve::device_arg(tx.project(), args)?;
    let tag = tx
        .project()
        .device(device)
        .map(|d| d.tag.to_string())
        .unwrap_or_default();
    let text = args["text"]
        .as_str()
        .ok_or("text is missing")?
        .trim()
        .to_string();
    if text.chars().count() > 200 || text.chars().any(char::is_control) {
        return Err("text is longer than 200 characters or has control characters".into());
    }
    tx.update::<novo_core::Device>(device, |d| d.description = text.clone())
        .map_err(|e| e.to_string())?;
    Ok(format!("Describe {tag} as {text:?}"))
}

fn select_standards(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let ids = args["ids"].as_array().ok_or("ids must be a list")?;
    let mut sets = Vec::new();
    for id in ids {
        let id = id.as_str().unwrap_or_default();
        sets.push(
            novo_checks::standards::StandardSet::parse(id).ok_or_else(|| {
                format!("unknown standard {id:?}; use iec60204, nfpa79 or ul508a")
            })?,
        );
    }
    novo_checks::standards::select(tx, &sets).map_err(|e| e.to_string())?;
    Ok(if sets.is_empty() {
        "Turn the standards report off".to_string()
    } else {
        format!(
            "Check the project against {}",
            sets.iter()
                .map(|s| s.short())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })
}

fn rename_page(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let page = resolve::page_arg(tx.project(), args)?;
    let old = tx
        .project()
        .page(page)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let name = need(args, "name")?;
    if tx.project().pages().any(|p| p.name == name && p.id != page) {
        return Err(format!("a page {name} already exists"));
    }
    tx.execute(novo_core::Command::RenamePage {
        page,
        name: name.clone(),
    })
    .map_err(|e| e.to_string())?;
    Ok(format!("Rename page {old} to {name}"))
}

fn add_device(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let tag = need(args, "tag")?;
    let parsed = DeviceTag::parse(&tag).map_err(|e| format!("tag {tag:?}: {e}"))?;
    if tx.project().devices().any(|d| d.tag.to_string() == tag) {
        return Err(format!("a device {tag} already exists"));
    }
    let description = args["description"].as_str().unwrap_or_default().to_string();
    tx.add_device(parsed, description.clone())
        .map_err(|e| e.to_string())?;
    Ok(if description.is_empty() {
        format!("Add device {tag}")
    } else {
        format!("Add device {tag} ({description})")
    })
}

/// What a step would replace, for the card only. Computed from the draft
/// before the step runs and never sent back to the model: old values and part
/// names can be above the agreed scope.
pub fn note_before(project: &Project, action: &str, args: &Value) -> Option<String> {
    if action == "set_wire_number" {
        let wire = args["wire_id"].as_u64().map(|id| id.into()).or_else(|| {
            let at = resolve::pin(project, &args["pin"]).ok()?;
            let mut wires = project.wires_at(at.end);
            let first = wires.next()?;
            wires.next().is_none().then_some(first)
        })?;
        let wire = project.wire(wire)?;
        let name = resolve::wire_description(project, wire);
        return Some(match &wire.number {
            Some(old) => format!("Replaces the current number of {name}: {old}"),
            None => {
                let mut c = name.chars();
                let cap: String = c
                    .next()
                    .map(|f| f.to_uppercase().chain(c).collect())
                    .unwrap_or_default();
                format!("{cap} has no number yet.")
            }
        });
    }
    let device = resolve::device_arg(project, args).ok()?;
    let device = project.device(device)?;
    match action {
        "set_property" => {
            let key = args["key"].as_str()?.trim();
            Some(match device.properties.get(key) {
                Some(old) => format!("Replaces the current value: {old}"),
                None => "There is no value yet.".into(),
            })
        }
        "assign_part" if !device.parts.is_empty() => Some(format!(
            "This device already has {} assigned.",
            plural(device.parts.len(), "part")
        )),
        "describe_device" if !device.description.is_empty() => Some(format!(
            "Replaces the current description: {:?}",
            device.description
        )),
        _ => None,
    }
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("1 {word}")
    } else {
        format!("{n} {word}s")
    }
}

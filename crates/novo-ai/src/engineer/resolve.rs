//! Finding things in the project by the names an engineer uses: page names,
//! device tags and `tag:pin` addresses. Every failure says what is there, so
//! the model can correct itself without guessing.

use novo_core::{DeviceId, Endpoint, PageId, PartId, Point, Project, SymbolId};
use serde_json::Value;

/// A string or a number as text (`"1"` and `1` both name page 1).
pub fn text_of(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.trim().to_string()).filter(|s| !s.is_empty()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn some_of(names: impl Iterator<Item = String>) -> String {
    let all: Vec<String> = names.collect();
    let shown: Vec<&str> = all.iter().take(30).map(String::as_str).collect();
    let more = all.len().saturating_sub(shown.len());
    if more > 0 {
        format!("{} (and {more} more)", shown.join(", "))
    } else if shown.is_empty() {
        "none".into()
    } else {
        shown.join(", ")
    }
}

pub fn page(project: &Project, text: &str) -> Result<PageId, String> {
    let t = text.trim();
    let pages: Vec<_> = project.pages().collect();
    pages
        .iter()
        .find(|p| p.name == t)
        .or_else(|| pages.iter().find(|p| p.name.eq_ignore_ascii_case(t)))
        .map(|p| p.id)
        .ok_or_else(|| {
            format!(
                "there is no page named {t:?}. Pages: {}",
                some_of(pages.iter().map(|p| p.name.clone()))
            )
        })
}

pub fn device(project: &Project, text: &str) -> Result<DeviceId, String> {
    let t = text.trim();
    let tags: Vec<(DeviceId, String)> = project
        .devices()
        .map(|d| (d.id, d.tag.to_string()))
        .collect();
    let exact: Vec<_> = tags.iter().filter(|(_, s)| s == t).collect();
    if let [one] = exact.as_slice() {
        return Ok(one.0);
    }
    let by_end: Vec<_> = tags
        .iter()
        .filter(|(_, s)| t.starts_with('-') && s.ends_with(t))
        .collect();
    match (exact.len(), by_end.as_slice()) {
        (0, [one]) => Ok(one.0),
        (0, []) => Err(format!(
            "there is no device {t}. Devices: {}",
            some_of(tags.iter().map(|(_, s)| s.clone()))
        )),
        _ => Err(format!(
            "{t} matches several devices ({}); give the full tag",
            some_of(exact.iter().chain(by_end.iter()).map(|(_, s)| s.clone()))
        )),
    }
}

/// A part of the project by id, `Manufacturer OrderNumber` or order number.
pub fn part(project: &Project, text: &str) -> Result<PartId, String> {
    let t = text.trim();
    let same = |a: &str| a.trim().eq_ignore_ascii_case(t);
    let hits: Vec<_> = project
        .parts()
        .filter(|p| {
            same(&p.order_number)
                || same(&format!("{} {}", p.manufacturer, p.order_number))
                || t.parse::<u64>().is_ok_and(|n| p.id.raw() == n)
        })
        .collect();
    match hits.as_slice() {
        [one] => Ok(one.id),
        [] => Err(format!(
            "the project has no part {t:?}. Parts: {}. Only parts already in the project can be \
             assigned; never invent an order number",
            some_of(
                project
                    .parts()
                    .map(|p| format!("{} {}", p.manufacturer, p.order_number))
            )
        )),
        _ => Err(format!(
            "{t:?} matches several parts; use Manufacturer OrderNumber"
        )),
    }
}

/// A pin of a drawn symbol: where it is, which page and which wire end.
#[derive(Clone, Copy, Debug)]
pub struct PinRef {
    pub end: Endpoint,
    pub at: Point,
    pub page: PageId,
    pub symbol: SymbolId,
}

/// `-K1:13`, `-K1:13#2` (the second symbol with that pin), `-X1:3.1`
/// (terminal 3 of strip -X1, upper pin) or `{"symbol_id": 12, "pin": "A1"}`.
pub fn pin(project: &Project, spec: &Value) -> Result<PinRef, String> {
    if let Some(sym) = spec["symbol_id"].as_u64() {
        let pin = text_of(&spec["pin"]).ok_or("pin is missing")?;
        return in_symbol(project, SymbolId::from(sym), &pin);
    }
    if let Some(text) = spec.as_str()
        && let Some((id, pin)) = unnamed_pin(text)
    {
        return in_symbol(project, SymbolId::from(id), &pin);
    }
    let (dev, rest) = match spec {
        Value::String(s) => {
            let (d, r) = s
                .rsplit_once(':')
                .ok_or_else(|| format!("{s:?} is not a pin address; write tag:pin, e.g. -K1:A1"))?;
            (d.trim().to_string(), r.trim().to_string())
        }
        Value::Object(_) => (
            text_of(&spec["device"]).ok_or("device is missing")?,
            text_of(&spec["pin"]).ok_or("pin is missing")?,
        ),
        _ => return Err("a pin is written as tag:pin, e.g. -K1:A1".into()),
    };
    let device = device(project, &dev)?;
    let (name, nth) = match rest.split_once('#') {
        Some((n, i)) => (
            n.to_string(),
            Some(
                i.parse::<usize>()
                    .map_err(|_| format!("bad #number in {rest:?}"))?,
            ),
        ),
        None => (rest.clone(), None),
    };
    let symbols = project.symbols_of_device(device);
    if symbols.is_empty() {
        return terminal_pin(project, device, &dev, &name);
    }
    let mut candidates: Vec<PinRef> = Vec::new();
    for s in &symbols {
        for (i, p) in s.pins.iter().enumerate() {
            if p.name == name {
                candidates.push(PinRef {
                    end: Endpoint::pin(s.id, i as u16),
                    at: s.placement.apply(p.offset),
                    page: s.page,
                    symbol: s.id,
                });
            }
        }
    }
    match (candidates.len(), nth) {
        (0, _) => {
            let pins: Vec<String> = symbols
                .iter()
                .flat_map(|s| s.pins.iter().map(|p| p.name.clone()))
                .collect();
            Err(format!(
                "{dev} has no pin {name:?}. Its pins: {}",
                some_of(pins.into_iter())
            ))
        }
        (1, None) => Ok(candidates[0]),
        (n, Some(i)) if i >= 1 && i <= n => Ok(candidates[i - 1]),
        (n, _) => Err(format!(
            "{dev} has {n} pins named {name:?} on different symbols; add #1 to #{n} \
             (in drawing order) or use symbol_id"
        )),
    }
}

fn in_symbol(project: &Project, id: SymbolId, pin: &str) -> Result<PinRef, String> {
    let s = project
        .symbol(id)
        .ok_or_else(|| format!("there is no symbol with id {}", id.raw()))?;
    let i = s
        .pins
        .iter()
        .position(|p| p.name == pin)
        .ok_or_else(|| format!("symbol {} has no pin {pin:?}", id.raw()))?;
    Ok(PinRef {
        end: Endpoint::pin(id, i as u16),
        at: s.placement.apply(s.pins[i].offset),
        page: s.page,
        symbol: id,
    })
}

fn terminal_pin(
    project: &Project,
    strip: DeviceId,
    shown: &str,
    rest: &str,
) -> Result<PinRef, String> {
    let (name, side) = rest.rsplit_once('.').ok_or_else(|| {
        format!(
            "{shown} has no symbols of its own; for a terminal write {shown}:<name>.1 (upper pin) \
             or .2 (lower pin)"
        )
    })?;
    let terminal = project
        .terminals()
        .find(|t| t.strip == strip && t.name == name)
        .ok_or_else(|| {
            format!(
                "strip {shown} has no terminal {name:?}. Terminals: {}",
                some_of(
                    project
                        .terminals()
                        .filter(|t| t.strip == strip)
                        .map(|t| t.name.clone())
                )
            )
        })?;
    let symbol = project
        .symbols_of_terminal(terminal.id)
        .first()
        .map(|s| s.id)
        .ok_or_else(|| format!("terminal {shown}:{name} is not drawn on a page"))?;
    in_symbol(project, symbol, side)
}

/// The device named by `device` (a tag) or, as older prompts wrote it, `device_id`.
pub fn device_arg(project: &Project, args: &Value) -> Result<DeviceId, String> {
    if let Some(text) = text_of(&args["device"]) {
        return device(project, &text);
    }
    match args["device_id"].as_u64() {
        Some(id) => project
            .device(id.into())
            .map(|d| d.id)
            .ok_or_else(|| format!("there is no device with id {id}")),
        None => Err("device is missing (give its tag, e.g. -K1)".into()),
    }
}

/// The page named by `page` (a name) or `page_id`.
pub fn page_arg(project: &Project, args: &Value) -> Result<PageId, String> {
    if let Some(text) = text_of(&args["page"]) {
        return page(project, &text);
    }
    match args["page_id"].as_u64() {
        Some(id) => project
            .page(id.into())
            .map(|p| p.id)
            .ok_or_else(|| format!("there is no page with id {id}")),
        None => Err("page is missing (give its name)".into()),
    }
}

/// The part named by `part` or `part_id`.
pub fn part_arg(project: &Project, args: &Value) -> Result<PartId, String> {
    if let Some(text) = text_of(&args["part"]) {
        return part(project, &text);
    }
    match args["part_id"].as_u64() {
        Some(id) => project
            .part(id.into())
            .map(|p| p.id)
            .ok_or_else(|| format!("there is no part with id {id} (see section parts)")),
        None => Err("part is missing".into()),
    }
}

/// A wire as the person sees it, without the internal id: "the wire from
/// -K1:2 to -F2:3 (page Main circuit)".
pub fn wire_description(p: &Project, w: &novo_core::Wire) -> String {
    let end = |e| novo_cables::ends::designation(p, e).unwrap_or_else(|| "an open end".into());
    let page = p.page(w.page).map_or("?", |g| g.name.as_str());
    format!(
        "the wire from {} to {} (page {page})",
        end(w.from),
        end(w.to)
    )
}

/// A person's name for a library symbol that has no device: `iec/terminals/terminal_pe`
/// is a "protective earth terminal".
pub fn friendly_symbol(library_id: &str) -> String {
    let last = library_id.rsplit('/').next().unwrap_or(library_id);
    match last {
        "terminal_pe" => "protective earth terminal".to_string(),
        "terminal_n" => "neutral terminal".to_string(),
        other => other.replace('_', " "),
    }
}

/// `protective earth terminal pin 1 (symbol 14)`, or the short `symbol 14 pin 1`.
fn unnamed_pin(text: &str) -> Option<(u64, String)> {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix("symbol ") {
        let (id, pin) = rest.split_once(" pin ")?;
        return Some((id.trim().parse().ok()?, pin.trim().to_string()));
    }
    let (head, tail) = text.rsplit_once(" (symbol ")?;
    let id = tail.strip_suffix(')')?.trim().parse().ok()?;
    let (_, pin) = head.rsplit_once(" pin ")?;
    Some((id, pin.trim().to_string()))
}

#[cfg(test)]
mod symbol_pin_tests {
    use super::*;

    #[test]
    fn an_unnamed_pin_is_written_for_people_and_read_back() {
        assert_eq!(unnamed_pin("symbol 7 pin 1"), Some((7, "1".into())));
        assert_eq!(
            unnamed_pin("protective earth terminal pin 1 (symbol 14)"),
            Some((14, "1".into()))
        );
        assert_eq!(unnamed_pin("-K1:A1"), None);
        assert_eq!(
            friendly_symbol("iec/terminals/terminal_pe"),
            "protective earth terminal"
        );
    }
}

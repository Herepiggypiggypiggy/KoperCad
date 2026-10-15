//! Mapping EPLAN part properties to catalog parts.

use std::collections::BTreeMap;

use novo_core::Value;
use novo_parts::{
    CatalogPart, Dimensions, ImportError, ImportReport, import_catalog, parse_number,
};
use thiserror::Error;

use super::xml::{self, Node, XmlError};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EplanError {
    #[error("the XML could not be read: {0}")]
    Xml(#[from] XmlError),
    #[error("no parts found: no element has an order number or part number")]
    NoParts,
    #[error(transparent)]
    Table(#[from] ImportError),
}

/// Key prefix for properties without a catalog field, kept so nothing is lost.
const KEEP: &str = "eplan.";

/// Canonical field of a property id or friendly name.
fn field(key: &str) -> Option<&'static str> {
    let k: String = key
        .to_ascii_uppercase()
        .trim_start_matches("P_ARTICLE_")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    Some(match k.as_str() {
        "ORDERNR" | "ORDERNUMBER" | "ORDERNO" | "BESTELLNUMMER" => "order",
        "PARTNR" | "PARTNUMBER" | "PARTNO" | "ARTIKELNUMMER" => "part",
        "TYPENR" | "TYPENUMBER" | "TYPE" | "TYPENUMMER" => "type",
        "MANUFACTURER" | "MANUFACTURERNAME" | "HERSTELLER" => "manufacturer",
        "DESCR1" | "DESCRIPTION1" | "DESCRIPTION" | "BESCHREIBUNG1" => "descr1",
        "DESCR2" | "DESCRIPTION2" => "descr2",
        "DESCR3" | "DESCRIPTION3" => "descr3",
        "WIDTH" => "width",
        "HEIGHT" => "height",
        "DEPTH" => "depth",
        _ => return None,
    })
}

/// Text of a property element: its own, else the first non-empty text among
/// its children (EPLAN wraps multi-language text in child elements).
fn text_of(n: &Node) -> String {
    if !n.text.is_empty() {
        return n.text.clone();
    }
    n.children
        .iter()
        .map(text_of)
        .find(|t| !t.is_empty())
        .unwrap_or_default()
}

/// The `(key, value)` pairs an element carries: attributes, leaf children,
/// and `<property id=".." value=".."/>` style children.
fn properties(n: &Node) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = n
        .attrs
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .cloned()
        .collect();
    for c in &n.children {
        let id = ["id", "name", "key"].iter().find_map(|a| c.attr(a));
        if let Some(id) = id {
            let v = c
                .attr("value")
                .map(str::to_string)
                .unwrap_or_else(|| text_of(c));
            if !v.is_empty() {
                out.push((id.to_string(), v));
            }
        } else if c.children.iter().all(|g| g.children.is_empty()) {
            let v = text_of(c);
            if !v.is_empty() {
                out.push((c.name.clone(), v));
            }
        }
    }
    out
}

fn is_part(props: &[(String, String)]) -> bool {
    props
        .iter()
        .any(|(k, _)| matches!(field(k), Some("order" | "part")))
}

/// Reads parts from EPLAN parts-management XML.
///
/// Any element that carries an order number or part number is one part, so
/// small differences between EPLAN versions do not matter. Elements without
/// a usable manufacturer fall back to `default_manufacturer`.
pub fn import_xml(text: &str, default_manufacturer: &str) -> Result<ImportReport, EplanError> {
    let root = xml::parse(text)?;
    let mut report = ImportReport::default();
    let mut seen = 0usize;
    // Iterative walk; a part's subtree is not searched again.
    let mut todo = vec![&root];
    let mut order: Vec<&Node> = Vec::new();
    while let Some(n) = todo.pop() {
        if is_part(&properties(n)) {
            order.push(n);
        } else {
            todo.extend(n.children.iter().rev());
        }
    }
    for n in order {
        seen += 1;
        match to_part(n, default_manufacturer) {
            Ok(p) => report.parts.push(p),
            Err(why) => report.skipped.push((n.line, why)),
        }
    }
    if seen == 0 {
        return Err(EplanError::NoParts);
    }
    Ok(report)
}

fn to_part(n: &Node, default_manufacturer: &str) -> Result<CatalogPart, String> {
    let mut f: BTreeMap<&'static str, String> = BTreeMap::new();
    let mut extra: Vec<(String, String)> = Vec::new();
    for (k, v) in properties(n) {
        match field(&k) {
            Some(name) => {
                f.entry(name).or_insert(v);
            }
            None => extra.push((k, v)),
        }
    }
    let manufacturer = f
        .get("manufacturer")
        .cloned()
        .unwrap_or_else(|| default_manufacturer.to_string());
    // EPLAN's part number is `<MANUFACTURER-ID>.<order number>` when the
    // order number itself is empty.
    let order = match (f.get("order"), f.get("part")) {
        (Some(o), _) => o.clone(),
        (None, Some(p)) => match p.split_once('.') {
            Some((prefix, rest))
                if !rest.is_empty()
                    && (prefix.eq_ignore_ascii_case(&manufacturer)
                        || (prefix.len() <= 4
                            && prefix.chars().all(|c| c.is_ascii_alphabetic()))) =>
            {
                rest.to_string()
            }
            _ => p.clone(),
        },
        (None, None) => return Err("no order number".into()),
    };
    if manufacturer.is_empty() {
        return Err("no manufacturer".into());
    }
    let mut part = CatalogPart::new(manufacturer, order);
    part.type_number = f.get("type").cloned().unwrap_or_default();
    let mut description = f.get("descr1").cloned().unwrap_or_default();
    for extra_line in ["descr2", "descr3"] {
        if let Some(d) = f.get(extra_line) {
            if description.is_empty() {
                description = d.clone();
            } else {
                part.data
                    .insert(format!("{KEEP}{extra_line}"), Value::Text(d.clone()));
            }
        }
    }
    part.description = description;
    if let Some(p) = f.get("part") {
        part.data
            .insert(format!("{KEEP}partnr"), Value::Text(p.clone()));
    }
    let dim = |k: &str| f.get(k).and_then(|v| parse_number(v));
    match (dim("width"), dim("height"), dim("depth")) {
        (Some(width_mm), Some(height_mm), Some(depth_mm)) => {
            part.dimensions = Some(Dimensions {
                width_mm,
                height_mm,
                depth_mm,
            });
        }
        (w, h, d) => {
            for (key, v) in [("width_mm", w), ("height_mm", h), ("depth_mm", d)] {
                if let Some(v) = v {
                    part.data.insert(key.into(), Value::Float(v));
                }
            }
        }
    }
    for (k, v) in extra {
        let key = format!(
            "{KEEP}{}",
            k.trim_start_matches("P_ARTICLE_").to_lowercase()
        );
        part.data.entry(key).or_insert(Value::Text(v));
    }
    Ok(part)
}

/// EPLAN column title to the header `novo_parts::import_catalog` knows.
fn header_for(title: &str) -> String {
    match field(title) {
        Some("order") => "Order number".into(),
        Some("part") => "Eplan part number".into(),
        Some("type") => "Type number".into(),
        Some("manufacturer") => "Manufacturer".into(),
        Some("descr1") => "Description".into(),
        Some("descr2") => "Description 2".into(),
        Some("descr3") => "Description 3".into(),
        Some("width") => "Width".into(),
        Some("height") => "Height".into(),
        Some("depth") => "Depth".into(),
        _ => title.trim().to_string(),
    }
}

/// Reads the delimited (tab, `;` or `,`) EPLAN export: the header row is
/// translated to the catalog vocabulary, then read like any catalog CSV.
/// When the file has no order-number column but a part-number column, the
/// part number is used.
pub fn import_delimited(
    text: &str,
    default_manufacturer: &str,
) -> Result<ImportReport, EplanError> {
    let text = text.trim_start_matches('\u{feff}');
    let (first, rest) = text.split_once('\n').unwrap_or((text, ""));
    let delim = [('\t', 0), (';', 0), (',', 0)]
        .map(|(d, _)| (d, first.matches(d).count()))
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map_or(',', |(d, _)| d);
    let mut headers: Vec<String> = first
        .trim_end_matches('\r')
        .split(delim)
        .map(|h| header_for(h.trim().trim_matches('"')))
        .collect();
    if !headers.iter().any(|h| h == "Order number")
        && let Some(h) = headers.iter_mut().find(|h| *h == "Eplan part number")
    {
        *h = "Order number".into();
    }
    let sep = delim.to_string();
    let rebuilt = format!("{}\n{rest}", headers.join(&sep));
    Ok(import_catalog(&rebuilt, default_manufacturer)?)
}

/// Picks XML or delimited text by the first character of the file.
pub fn import_auto(text: &str, default_manufacturer: &str) -> Result<ImportReport, EplanError> {
    if text
        .trim_start_matches('\u{feff}')
        .trim_start()
        .starts_with('<')
    {
        import_xml(text, default_manufacturer)
    } else {
        import_delimited(text, default_manufacturer)
    }
}

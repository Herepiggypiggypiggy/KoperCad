//! Ordering and designation helpers shared by the reports.

use std::cmp::Ordering;

use novo_core::{DeviceTag, Location, natural_cmp};

fn levels_cmp(a: &[String], b: &[String]) -> Ordering {
    a.iter()
        .zip(b)
        .map(|(a, b)| natural_cmp(a, b))
        .find(|o| o.is_ne())
        .unwrap_or_else(|| a.len().cmp(&b.len()))
}

/// Natural order of reference designations: function levels, then
/// location levels, then product levels, each in natural order, so `-K2`
/// comes before `-K10` and `=A1` before `=A2`.
pub fn tag_cmp(a: &DeviceTag, b: &DeviceTag) -> Ordering {
    levels_cmp(&a.function, &b.function)
        .then_with(|| levels_cmp(&a.location, &b.location))
        .then_with(|| levels_cmp(&a.product, &b.product))
}

/// The function part of a tag (`=A1`), empty when it has none.
pub fn function_of(tag: &DeviceTag) -> String {
    DeviceTag {
        function: tag.function.clone(),
        ..DeviceTag::default()
    }
    .to_string()
}

/// The location part of a tag (`+CAB`), empty when it has none.
pub fn location_of(tag: &DeviceTag) -> String {
    DeviceTag {
        location: tag.location.clone(),
        ..DeviceTag::default()
    }
    .to_string()
}

/// A `page.column` reference as printed in a report, which belongs to no
/// structure: `3.4`, or `=A2/3.4` for a page with a structure.
pub fn location_text(location: &Location) -> String {
    location.qualified(&DeviceTag::default())
}

/// Items joined with `, `, each once, in first-seen order.
pub fn join_distinct<I, S>(items: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut seen: Vec<String> = Vec::new();
    for item in items {
        let item = item.as_ref();
        if !item.is_empty() && !seen.iter().any(|s| s == item) {
            seen.push(item.to_owned());
        }
    }
    seen.join(", ")
}

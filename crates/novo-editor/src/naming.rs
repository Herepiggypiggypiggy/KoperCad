//! Device tags and connection numbers given without asking (DESIGN.md §2
//! and §7: numbering is a rule, applied on placement).

use novo_core::{DeviceId, DeviceTag, PageId, Project};

/// The function and location aspects of new devices on a page: the page's
/// structure ([`novo_core::Page::structure`], such as `=A1+CAB1`), or none.
pub fn page_structure(project: &Project, page: PageId) -> DeviceTag {
    project
        .page(page)
        .map(|p| {
            let mut t = p.structure.clone();
            t.product.clear();
            t
        })
        .unwrap_or_default()
}

/// A tag typed without function or location (`-K1`) completed with the
/// page's structure (`=A1+CAB1-K1`); typed aspects are kept.
pub fn complete_tag(project: &Project, page: PageId, tag: &DeviceTag) -> DeviceTag {
    let structure = page_structure(project, page);
    let mut tag = tag.clone();
    if tag.function.is_empty() {
        tag.function = structure.function;
    }
    if tag.location.is_empty() {
        tag.location = structure.location;
    }
    tag
}

/// How new devices are numbered: a project rule (DESIGN.md §7), so other
/// schemes such as page based `-K301` (§6) are added here without touching
/// placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Numbering {
    /// `-K1`, `-K2`, ... per letter code within the page's structure.
    #[default]
    PerLetter,
}

/// The tag a new device with `letter` gets on `page` under `scheme`.
pub fn new_device_tag(
    project: &Project,
    page: PageId,
    letter: &str,
    scheme: Numbering,
) -> DeviceTag {
    match scheme {
        Numbering::PerLetter => next_device_tag(project, page, letter),
    }
}

/// The next free tag for a letter code on a page: `-K3` when `-K1` and `-K2`
/// exist in the page's structure.
pub fn next_device_tag(project: &Project, page: PageId, letter: &str) -> DeviceTag {
    let structure = page_structure(project, page);
    let number = project.next_tag_number(&structure, letter);
    let mut tag = structure;
    tag.product.push(format!("{letter}{number}"));
    tag
}

/// Function digit of an EN 50005 auxiliary contact designation: `13` and
/// `14` are the make contact of the first contact position, `21`/`22` the
/// break contact of the second.
fn split_contact(name: &str) -> Option<(u32, char)> {
    let mut chars = name.chars();
    let last = chars.next_back()?;
    let seq: u32 = chars.as_str().parse().ok()?;
    ('1'..='9').contains(&last).then_some((seq, last))
}

/// Connection names for a new auxiliary contact of `device`: the symbol's
/// default names (`13`, `14`) moved to the next free contact position
/// (`23`, `24`, then `33`, `34`) per EN 50005. `None` when the names are not
/// auxiliary contact numbers (main contacts `1`..`6`, overload `95`/`96`),
/// which are kept as drawn.
pub fn child_pin_names(
    project: &Project,
    device: DeviceId,
    defaults: &[&str],
) -> Option<Vec<String>> {
    let parsed: Vec<(u32, char)> = defaults
        .iter()
        .map(|n| split_contact(n))
        .collect::<Option<_>>()?;
    // Only one-digit positions with function digits 1 to 4 renumber.
    if parsed
        .iter()
        .any(|&(seq, f)| !(1..=9).contains(&seq) || !('1'..='4').contains(&f))
    {
        return None;
    }
    let used: std::collections::BTreeSet<u32> = project
        .symbols_of_device(device)
        .iter()
        .flat_map(|s| s.pins.iter())
        .filter_map(|p| split_contact(&p.name))
        .filter(|&(seq, f)| (1..=9).contains(&seq) && ('1'..='4').contains(&f))
        .map(|(seq, _)| seq)
        .collect();
    let seq = (1..).find(|s| !used.contains(s))?;
    Some(parsed.iter().map(|&(_, f)| format!("{seq}{f}")).collect())
}

/// Parses what the user typed as a new tag for a device. Aspects that are
/// not typed are kept from the old tag, so typing `-K5` (or just `K5`) on
/// `=A1+CAB1-K1` gives `=A1+CAB1-K5`.
pub fn parse_tag_input(input: &str, old: &DeviceTag) -> Result<DeviceTag, String> {
    let text = input.trim();
    if text.is_empty() {
        return Err("A tag cannot be empty".into());
    }
    let text = if text.starts_with(['-', '=', '+']) {
        text.to_owned()
    } else {
        format!("-{text}")
    };
    let typed = DeviceTag::parse(&text).map_err(|e| e.to_string())?;
    let mut tag = old.clone();
    if !typed.function.is_empty() {
        tag.function = typed.function;
    }
    if !typed.location.is_empty() {
        tag.location = typed.location;
    }
    if !typed.product.is_empty() {
        tag.product = typed.product;
    }
    tag.validate().map_err(|e| e.to_string())?;
    Ok(tag)
}

/// Why a tag cannot be given to `device`, if it cannot: it is already used
/// by another device or a cable.
pub fn tag_conflict(project: &Project, device: DeviceId, tag: &DeviceTag) -> Option<String> {
    if project.devices_by_tag(tag).iter().any(|&d| d != device) {
        return Some(format!("{tag} is already used by another device"));
    }
    if project.cables().any(|c| c.tag == *tag) {
        return Some(format!("{tag} is already used by a cable"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_product_keeps_structure() {
        let old = DeviceTag::parse("=A1+CAB1-K1").unwrap();
        assert_eq!(
            parse_tag_input("-K5", &old).unwrap().to_string(),
            "=A1+CAB1-K5"
        );
        assert_eq!(
            parse_tag_input("k5", &old).unwrap().to_string(),
            "=A1+CAB1-k5"
        );
        assert_eq!(
            parse_tag_input("=B2-K5", &old).unwrap().to_string(),
            "=B2+CAB1-K5"
        );
        assert!(parse_tag_input("", &old).is_err());
        assert!(parse_tag_input("-K 5!", &old).is_err());
    }

    #[test]
    fn contact_numbers_split() {
        assert_eq!(split_contact("13"), Some((1, '3')));
        assert_eq!(split_contact("A1"), None);
        assert_eq!(split_contact("95"), Some((9, '5')));
    }
}

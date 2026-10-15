//! Moving terminals to another strip number: the Up arrow and Ctrl+Up on a
//! selection of terminals that inherited a strip they should not have.

use std::collections::BTreeMap;

use novo_core::{DeviceId, ModelError, Project, Result, Terminal, TerminalId, Tx};

use crate::naming::next_order;

/// Where the terminals go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveTo {
    /// The strip numbered one higher than their current one (`-X1` goes to
    /// `-X2`), created when it does not exist.
    NextNumber,
    /// The strip with the lowest number nobody uses yet.
    LowestFree,
}

fn strip_number(project: &Project, strip: DeviceId) -> Option<(novo_core::DeviceTag, u32)> {
    let tag = project.device(strip)?.tag.clone();
    let n = tag.number()?;
    Some((tag, n))
}

/// The lowest `-X` number free in the structure of `strip`.
fn lowest_free(project: &Project, strip: DeviceId) -> Option<u32> {
    let (tag, _) = strip_number(project, strip)?;
    let used: Vec<u32> = project
        .devices()
        .filter(|d| d.tag.same_structure(&tag) && d.tag.letter_code() == tag.letter_code())
        .filter_map(|d| d.tag.number())
        .collect();
    (1u32..).find(|n| !used.contains(n))
}

/// A name for a terminal arriving on a strip where its own is taken.
fn free_name(project: &Project, strip: DeviceId, name: &str, taken: &[String]) -> String {
    let used = |candidate: &str| {
        taken.iter().any(|t| t == candidate)
            || project
                .terminals_of_strip(strip)
                .iter()
                .any(|t| t.name == candidate)
    };
    if !used(name) {
        return name.to_owned();
    }
    if name.parse::<u32>().is_ok() {
        (1u32..)
            .map(|n| n.to_string())
            .find(|n| !used(n))
            .expect("some number is free")
    } else {
        let base = name.trim_end_matches(|c: char| c.is_ascii_digit());
        (2u32..)
            .map(|n| format!("{base}{n}"))
            .find(|n| !used(n))
            .expect("some name is free")
    }
}

/// Moves `terminals` (all of one strip) to another strip of the same
/// structure, as one edit, and returns that strip. Names stay, unless the
/// target already has one: then the terminal takes the next free number (or
/// `PE2`, ...). They land after the terminals already there, in their old
/// order. The strip they leave is deleted when nothing is left on it.
///
/// Refused with nothing changed ([`ModelError::NotFound`] or
/// [`ModelError::InvalidValue`]) for an empty list, terminals of several
/// strips, or a strip tag without a number.
pub fn move_to_strip(tx: &mut Tx<'_>, terminals: &[TerminalId], to: MoveTo) -> Result<DeviceId> {
    let mut records: Vec<Terminal> = Vec::new();
    for &id in terminals {
        records.push(
            tx.project()
                .terminal(id)
                .ok_or(ModelError::NotFound(id.into()))?
                .clone(),
        );
    }
    let Some(first) = records.first() else {
        return Err(ModelError::Format("no terminals to move".into()));
    };
    let from = first.strip;
    if records.iter().any(|t| t.strip != from) {
        return Err(ModelError::InvalidValue {
            id: first.id.into(),
            field: "strip",
        });
    }
    let invalid = || ModelError::InvalidValue {
        id: from.into(),
        field: "tag",
    };
    let (tag, number) = strip_number(tx.project(), from).ok_or_else(invalid)?;
    let target_number = match to {
        MoveTo::NextNumber => number + 1,
        MoveTo::LowestFree => lowest_free(tx.project(), from).ok_or_else(invalid)?,
    };
    if target_number == number {
        return Ok(from);
    }
    let target_tag = tag.with_number(target_number);
    let target = match tx
        .project()
        .devices_by_tag(&target_tag)
        .into_iter()
        .find(|&d| {
            tx.project()
                .device(d)
                .is_some_and(|d| d.kind == novo_core::DeviceKind::TerminalStrip)
        }) {
        Some(strip) => strip,
        None => tx.add_terminal_strip(target_tag)?,
    };
    records.sort_by(|a, b| a.order.cmp(&b.order).then(a.id.cmp(&b.id)));
    // One new name per old name, so the levels of a terminal stay together.
    let mut renamed: BTreeMap<String, String> = BTreeMap::new();
    let mut taken: Vec<String> = Vec::new();
    let mut order = next_order(tx.project(), target);
    for t in &records {
        if !renamed.contains_key(&t.name) {
            let name = free_name(tx.project(), target, &t.name, &taken);
            taken.push(name.clone());
            renamed.insert(t.name.clone(), name);
        }
        let name = renamed[&t.name].clone();
        tx.update::<Terminal>(t.id, |x| {
            x.strip = target;
            x.name = name;
            x.order = order;
        })?;
        order += 10;
    }
    if tx.project().terminals_of_strip(from).is_empty() {
        tx.remove(from)?;
    }
    Ok(target)
}

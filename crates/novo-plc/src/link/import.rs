//! Taking a tag file back into the project: the PLC programmer renamed a
//! tag or reworded a comment, and the signal text follows. Addresses never
//! come back (plc.md §6): they are derived from the cards, so a file cannot
//! make the drawing and the PLC disagree. Where a file's address differs
//! from the project's, the plan lists it and leaves it.

use std::collections::{HashMap, HashSet};

use novo_core::{Project, Result, SymbolId, Tx, Value};

use crate::card::SIGNAL;
use crate::index::PlcIndex;
use crate::layout::ChannelType;

use super::addr::{AddrKey, byte_key, parse_address, slot_key};
use super::read::{FileKind, FileTag, TagFile};
use super::tags::{TAG, TagOptions, identifier, tag_set};

/// One channel symbol the file changes.
#[derive(Debug, Clone, PartialEq)]
pub struct Update {
    pub symbol: SymbolId,
    /// `-K4 DI 3`.
    pub channel: String,
    /// The address in the project's style, as the programmer will see it.
    pub address: String,
    pub old_signal: String,
    pub new_signal: Option<String>,
    pub old_tag: String,
    pub new_tag: Option<String>,
    pub line: usize,
}

/// What reading a file into the project would do.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportPlan {
    pub kind: FileKind,
    pub updates: Vec<Update>,
    /// Tags that match no channel: `(line, text)`.
    pub unmatched: Vec<(usize, String)>,
    /// Tags for spare channels (no symbol to hold a signal): `(channel, tag)`.
    pub spare: Vec<(String, String)>,
    /// A file address that is not the project's address for the channel.
    pub address_notes: Vec<String>,
    pub warnings: Vec<String>,
    /// Drawn channels the file already agrees with.
    pub unchanged: usize,
    /// Drawn channels the file does not mention.
    pub not_in_file: usize,
}

impl ImportPlan {
    pub fn is_empty(&self) -> bool {
        self.updates.is_empty()
    }

    /// One line for the message bar and the preview's title.
    pub fn summary(&self) -> String {
        let n = self.updates.len();
        let mut s = format!(
            "{}: {n} channel{} to update, {} unchanged",
            self.kind.label(),
            if n == 1 { "" } else { "s" },
            self.unchanged
        );
        if !self.unmatched.is_empty() {
            s.push_str(&format!(
                ", {} not found in the project",
                self.unmatched.len()
            ));
        }
        if !self.address_notes.is_empty() {
            s.push_str(&format!(
                ", {} with another address",
                self.address_notes.len()
            ));
        }
        s
    }
}

/// Matches a file's tags with the project's channels. Changes nothing.
pub fn plan_import(project: &Project, index: &PlcIndex, file: &TagFile) -> ImportPlan {
    let rows = index.io_rows(project);
    let signal_of: HashMap<(novo_core::DeviceId, ChannelType, u16), String> = rows
        .iter()
        .map(|r| ((r.card, r.kind, r.channel), r.signal.clone()))
        .collect();
    // The names the last export gave, so a file that kept them (cut to a
    // shorter limit, or with a `_2` the export added) changes nothing.
    let exported: HashMap<SymbolId, String> = tag_set(
        project,
        index,
        crate::address::AddressStyle::Iec,
        &TagOptions {
            require_address: false,
            ..TagOptions::default()
        },
    )
    .tags
    .into_iter()
    .filter_map(|t| Some((t.symbol?, t.name)))
    .collect();
    let mut by_address: HashMap<AddrKey, Vec<(novo_core::DeviceId, ChannelType, u16)>> =
        HashMap::new();
    let mut by_name: HashMap<String, novo_core::DeviceId> = HashMap::new();
    for card in index.cards() {
        if let Some(d) = project.device(card.device) {
            by_name.insert(d.tag.to_string().to_ascii_lowercase(), card.device);
        }
        let layout = card.channels();
        for kind in ChannelType::ALL {
            for n in 0..layout.count(kind) {
                let Some(a) = card.address_in(kind, n, crate::address::AddressStyle::Rockwell)
                else {
                    continue;
                };
                if let Some(k) = slot_key(&a) {
                    by_address
                        .entry(k)
                        .or_default()
                        .push((card.device, kind, n));
                }
                if let Some(a) = card.address(kind, n) {
                    by_address
                        .entry(byte_key(&a))
                        .or_default()
                        .push((card.device, kind, n));
                }
            }
        }
    }
    let mut plan = ImportPlan {
        kind: file.kind,
        updates: Vec::new(),
        unmatched: Vec::new(),
        spare: Vec::new(),
        address_notes: Vec::new(),
        warnings: Vec::new(),
        unchanged: 0,
        not_in_file: 0,
    };
    let mut seen = HashSet::new();
    let mut mentioned = HashSet::new();
    for tag in &file.tags {
        let found = resolve(tag, &by_name, &by_address, &mut plan);
        let Some((card, kind, number)) = found else {
            continue;
        };
        let Some(card_ref) = index.card(card) else {
            continue;
        };
        let card_tag = project
            .device(card)
            .map(|d| d.tag.to_string())
            .unwrap_or_default();
        let label = format!("{card_tag} {} {number}", kind.code());
        if !seen.insert((card, kind, number)) {
            plan.warnings.push(format!(
                "line {}: {label} is in the file more than once; the first one counts",
                tag.line
            ));
            continue;
        }
        mentioned.insert((card, kind, number));
        if let Some(key) = parse_address(&tag.address) {
            let mine = card_ref
                .address(kind, number)
                .map(|a| byte_key(&a))
                .into_iter()
                .chain(
                    card_ref
                        .address_in(kind, number, crate::address::AddressStyle::Rockwell)
                        .and_then(|a| slot_key(&a)),
                )
                .collect::<Vec<_>>();
            if !mine.contains(&key) {
                plan.address_notes.push(format!(
                    "{label}: the file says {}, the project has {}. The address was not changed",
                    tag.address,
                    card_ref
                        .address_in(kind, number, index.style)
                        .map(|a| a.format(index.style))
                        .unwrap_or_else(|| "none".into())
                ));
            }
            if key.is_word() != kind.is_analog() {
                plan.warnings.push(format!(
                    "line {}: {} is a {} address but {label} is {}",
                    tag.line,
                    tag.address,
                    if key.is_word() { "word" } else { "bit" },
                    if kind.is_analog() {
                        "analog"
                    } else {
                        "digital"
                    },
                ));
            }
        }
        let wants_bool = matches!(tag.data_type.to_ascii_uppercase().as_str(), "BOOL" | "BIT");
        let wants_word = matches!(
            tag.data_type.to_ascii_uppercase().as_str(),
            "INT" | "WORD" | "UINT" | "DINT" | "REAL" | "SINT"
        );
        if (wants_bool && kind.is_analog()) || (wants_word && !kind.is_analog()) {
            plan.warnings.push(format!(
                "line {}: type {} does not fit {label}",
                tag.line, tag.data_type
            ));
        }
        let Some(symbol) = index.drawn(card, kind, number).first().copied() else {
            plan.spare.push((label, tag.name.clone()));
            continue;
        };
        let old_signal = signal_of
            .get(&(card, kind, number))
            .cloned()
            .unwrap_or_default();
        let old_stored = project
            .symbol(symbol)
            .and_then(|s| match s.properties.get(TAG) {
                Some(Value::Text(t)) if !t.trim().is_empty() => Some(t.trim().to_owned()),
                _ => None,
            });
        let comment = tag.comment.trim();
        let new_signal = (!comment.is_empty() && comment != old_signal).then(|| comment.to_owned());
        let signal_now = new_signal.as_deref().unwrap_or(&old_signal);
        let old_name = exported.get(&symbol).cloned().unwrap_or_default();
        let derived_after = identifier(signal_now, 128).unwrap_or_default();
        let name = tag.name.trim();
        let new_tag = (!name.is_empty()
            && !name.eq_ignore_ascii_case(&old_name)
            && !cut_to_a_limit(&old_name, name)
            && Some(name) != old_stored.as_deref()
            && !(old_stored.is_none() && name.eq_ignore_ascii_case(&derived_after)))
        .then(|| name.to_owned());
        if new_signal.is_none() && new_tag.is_none() {
            plan.unchanged += 1;
            continue;
        }
        plan.updates.push(Update {
            symbol,
            channel: label,
            address: card_ref
                .address_in(kind, number, index.style)
                .map(|a| a.format(index.style))
                .unwrap_or_default(),
            old_signal,
            new_signal,
            old_tag: old_stored.unwrap_or(old_name),
            new_tag,
            line: tag.line,
        });
    }
    plan.not_in_file = rows
        .iter()
        .filter(|r| !r.spare && !mentioned.contains(&(r.card, r.kind, r.channel)))
        .count();
    plan
}

/// The channel a file tag names: by card, type and number when it has them,
/// else by address.
fn resolve(
    tag: &FileTag,
    by_name: &HashMap<String, novo_core::DeviceId>,
    by_address: &HashMap<AddrKey, Vec<(novo_core::DeviceId, ChannelType, u16)>>,
    plan: &mut ImportPlan,
) -> Option<(novo_core::DeviceId, ChannelType, u16)> {
    if let (false, Some(kind), Some(number)) = (tag.card.is_empty(), tag.kind, tag.channel) {
        if let Some(&card) = by_name.get(&tag.card.to_ascii_lowercase()) {
            return Some((card, kind, number));
        }
        plan.unmatched
            .push((tag.line, format!("{}: no card {}", tag.name, tag.card)));
        return None;
    }
    let Some(key) = parse_address(&tag.address) else {
        plan.unmatched.push((
            tag.line,
            format!("{}: {} is not a PLC I/O address", tag.name, tag.address),
        ));
        return None;
    };
    match by_address.get(&key).map(Vec::as_slice) {
        Some([one]) => Some(*one),
        Some(many) if many.len() > 1 => {
            let mut distinct = many.to_vec();
            distinct.sort();
            distinct.dedup();
            if let [one] = distinct.as_slice() {
                return Some(*one);
            }
            plan.warnings.push(format!(
                "line {}: {} fits channels of several stations. Read the I/O list CSV, which names the card",
                tag.line, tag.address
            ));
            None
        }
        _ => {
            plan.unmatched.push((
                tag.line,
                format!(
                    "{} ({}): no channel has this address",
                    tag.name, tag.address
                ),
            ));
            None
        }
    }
}

/// Writes a plan into the project's channel symbols, as part of one undo
/// step: the signal text (`plc.signal`) and the tag name (`plc.tag`).
pub fn apply_import(tx: &mut Tx, plan: &ImportPlan) -> Result<()> {
    for u in &plan.updates {
        if let Some(signal) = &u.new_signal {
            tx.set_property(u.symbol, SIGNAL, Some(Value::from(signal.as_str())))?;
        }
        if let Some(tag) = &u.new_tag {
            tx.set_property(u.symbol, TAG, Some(Value::from(tag.as_str())))?;
        }
    }
    Ok(())
}

/// `name` is `old` cut to a tool's length limit (Studio 5000 allows 40), not
/// a rename that happens to begin the same way.
fn cut_to_a_limit(old: &str, name: &str) -> bool {
    old.len() > 40
        && name.len() <= 40
        && old
            .to_ascii_lowercase()
            .starts_with(&name.to_ascii_lowercase())
}

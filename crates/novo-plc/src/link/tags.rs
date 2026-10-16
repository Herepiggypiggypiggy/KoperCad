//! The tag list: one named, typed, commented entry per channel, which every
//! export format is written from.

use std::collections::HashSet;

use novo_core::{Project, SymbolId, Value};

use crate::address::{Address, AddressStyle};
use crate::index::PlcIndex;
use crate::layout::ChannelType;

/// On a channel symbol: the tag name the PLC program uses, when it was
/// typed or came back from the PLC tool. Absent: the name is made from the
/// signal text.
pub const TAG: &str = "plc.tag";

/// What goes into a tag list.
#[derive(Debug, Clone)]
pub struct TagOptions {
    /// Spare channels get a tag too (`Spare_K4_DI3`), so the PLC tool has a
    /// name for every point on the card.
    pub include_spares: bool,
    /// Longest tag name the target accepts (Studio 5000 allows 40).
    pub max_name: usize,
    /// Channels without an address (a card with no start) are left out.
    pub require_address: bool,
}

impl Default for TagOptions {
    fn default() -> Self {
        TagOptions {
            include_spares: false,
            max_name: 128,
            require_address: true,
        }
    }
}

/// One channel as the PLC tool sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct Tag {
    pub station: String,
    pub card_tag: String,
    pub slot: Option<u16>,
    pub kind: ChannelType,
    pub channel: u16,
    /// Unique over the whole list, valid as an IEC 61131 identifier.
    pub name: String,
    /// The name came from `plc.tag`, not from the signal.
    pub name_stored: bool,
    /// The signal text: the PLC tool's comment.
    pub comment: String,
    pub spare: bool,
    pub symbol: Option<SymbolId>,
    pub address: Option<Address>,
    /// The address as written for the target's style.
    pub address_text: String,
    pub terminal: String,
    pub connected: String,
    pub drawn_at: String,
}

impl Tag {
    /// `BOOL` for a digital channel, `INT` for an analog one.
    pub fn data_type(&self) -> &'static str {
        data_type(self.kind)
    }
}

/// The IEC data type a channel carries.
pub fn data_type(kind: ChannelType) -> &'static str {
    if kind.is_analog() { "INT" } else { "BOOL" }
}

/// A tag list and what was left out of it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TagSet {
    pub tags: Vec<Tag>,
    /// Plain-language notes: a channel left out for want of an address, a
    /// name that had to be shortened.
    pub warnings: Vec<String>,
}

impl TagSet {
    /// The stations in the list, in order, each once. The unnamed station is
    /// the empty string.
    pub fn stations(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for t in &self.tags {
            if !out.contains(&t.station) {
                out.push(t.station.clone());
            }
        }
        out
    }

    pub fn of_station<'a>(&'a self, station: &'a str) -> impl Iterator<Item = &'a Tag> + 'a {
        self.tags.iter().filter(move |t| t.station == station)
    }
}

/// Builds the tag list of a project with every address written in `style`.
pub fn tag_set(
    project: &Project,
    index: &PlcIndex,
    style: AddressStyle,
    options: &TagOptions,
) -> TagSet {
    let mut set = TagSet::default();
    let mut used: HashSet<String> = HashSet::new();
    let mut pending = Vec::new();
    for row in index.io_rows(project) {
        if row.spare && !options.include_spares {
            continue;
        }
        let Some(card) = index.card(row.card) else {
            continue;
        };
        let address = card.address_in(row.kind, row.channel, style);
        let symbol = index
            .drawn(row.card, row.kind, row.channel)
            .first()
            .copied();
        let label = format!("{} {} {}", row.card_tag, row.kind.code(), row.channel);
        if address.is_none() && options.require_address {
            set.warnings.push(format!(
                "{label} has no address in this style (the card has no start or slot), so it is not in the file"
            ));
            continue;
        }
        let stored =
            symbol
                .and_then(|s| project.symbol(s))
                .and_then(|s| match s.properties.get(TAG) {
                    Some(Value::Text(t)) if !t.trim().is_empty() => Some(t.trim().to_owned()),
                    _ => None,
                });
        let fallback = format!(
            "{}_{}{}",
            card_word(&row.card_tag),
            row.kind.code(),
            row.channel
        );
        let base = stored
            .clone()
            .or_else(|| identifier(&row.signal, options.max_name))
            .unwrap_or_else(|| {
                if row.spare {
                    format!("Spare_{fallback}")
                } else {
                    fallback.clone()
                }
            });
        pending.push((row, address, symbol, stored.is_some(), base, label));
    }
    // Names someone chose keep their name; derived names yield to them.
    let mut names: Vec<Option<String>> = vec![None; pending.len()];
    for pass_stored in [true, false] {
        for (i, p) in pending.iter().enumerate() {
            if p.3 != pass_stored {
                continue;
            }
            let name = unique(&p.4, options.max_name, &mut used);
            if name != p.4 && p.3 {
                set.warnings.push(format!(
                    "{}: the tag name {} was changed to {name} (taken or too long)",
                    p.5, p.4
                ));
            }
            names[i] = Some(name);
        }
    }
    for ((row, address, symbol, name_stored, _, _), name) in pending.into_iter().zip(names) {
        set.tags.push(Tag {
            station: row.station,
            card_tag: row.card_tag,
            slot: row.slot,
            kind: row.kind,
            channel: row.channel,
            name: name.unwrap_or_default(),
            name_stored,
            comment: row.signal,
            spare: row.spare,
            symbol,
            address,
            address_text: address.map(|a| a.format(style)).unwrap_or_default(),
            terminal: row
                .terminals
                .first()
                .map(|t| t.0.clone())
                .unwrap_or_default(),
            connected: row
                .connected
                .iter()
                .map(|c| c.0.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            drawn_at: row.drawn_at.map(|d| d.0).unwrap_or_default(),
        });
    }
    set
}

/// `-K4` as a word: `K4`.
fn card_word(tag: &str) -> String {
    identifier(tag.trim_start_matches(['-', '=', '+']), 40).unwrap_or_else(|| "PLC".into())
}

const RESERVED: &[&str] = &[
    "AND", "AT", "BOOL", "CASE", "DO", "ELSE", "END_IF", "FALSE", "FOR", "IF", "INT", "MOD", "NOT",
    "OF", "OR", "PROGRAM", "REAL", "THEN", "TO", "TRUE", "VAR", "WHILE", "XOR",
];

/// Makes text into an identifier every PLC tool accepts: ASCII letters,
/// digits and single underscores, starting with a letter, at most `max`
/// characters, not a reserved word. `None` when nothing is left.
pub fn identifier(text: &str, max: usize) -> Option<String> {
    let mut out = String::new();
    for c in text.chars() {
        let c = fold(c);
        let c = if c.is_ascii_alphanumeric() { c } else { '_' };
        if c == '_' && (out.is_empty() || out.ends_with('_')) {
            continue;
        }
        out.push(c);
    }
    let out = out.trim_end_matches('_').to_owned();
    if out.is_empty() {
        return None;
    }
    let mut out = if out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("S_{out}")
    } else {
        out
    };
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(&out)) {
        out.push_str("_x");
    }
    Some(cut(&out, max))
}

fn cut(name: &str, max: usize) -> String {
    let mut s: String = name.chars().take(max.max(1)).collect();
    while s.len() > 1 && s.ends_with('_') {
        s.pop();
    }
    s
}

/// Accented Latin letters without their accent; anything else unchanged.
fn fold(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'È' | 'É' | 'Ê' | 'Ë' => 'E',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => 'o',
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => 'O',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
        'ç' => 'c',
        'Ç' => 'C',
        'ñ' => 'n',
        'Ñ' => 'N',
        _ => c,
    }
}

/// `base`, or `base_2`, `base_3`... when it is taken (names compare without
/// regard to case), within `max` characters.
fn unique(base: &str, max: usize, used: &mut HashSet<String>) -> String {
    let base = cut(base, max);
    let mut n = 1;
    loop {
        let candidate = if n == 1 {
            base.clone()
        } else {
            let suffix = format!("_{n}");
            let room = max.saturating_sub(suffix.len()).max(1);
            format!("{}{suffix}", cut(&base, room))
        };
        if used.insert(candidate.to_ascii_lowercase()) {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers() {
        assert_eq!(
            identifier("Feed pump 1 running", 128).as_deref(),
            Some("Feed_pump_1_running")
        );
        assert_eq!(
            identifier("  Nood-stop (S1)  ", 128).as_deref(),
            Some("Nood_stop_S1")
        );
        assert_eq!(identifier("Pomp één", 128).as_deref(), Some("Pomp_een"));
        assert_eq!(
            identifier("3 phase ok", 128).as_deref(),
            Some("S_3_phase_ok")
        );
        assert_eq!(identifier("if", 128).as_deref(), Some("if_x"));
        assert_eq!(identifier("???", 128), None);
        assert_eq!(
            identifier("Feed pump 1 running", 10).as_deref(),
            Some("Feed_pump")
        );
    }

    #[test]
    fn duplicates_get_a_number_inside_the_limit() {
        let mut used = HashSet::new();
        assert_eq!(unique("Horn", 40, &mut used), "Horn");
        assert_eq!(unique("HORN", 40, &mut used), "HORN_2");
        assert_eq!(unique("Horn", 40, &mut used), "Horn_3");
        assert_eq!(unique("Horn_long", 6, &mut used), "Horn_l");
        assert_eq!(unique("Horn_long", 6, &mut used), "Horn_4");
    }
}

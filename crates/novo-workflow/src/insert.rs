//! The insert search (I, J2): find a symbol in a few letters, with no
//! separate insert centre to browse.
//!
//! - Short mnemonics rank first: `no`, `nc`, `coil`, `mcb`, `mpcb`, `ol`,
//!   `m3`, `pb`, `lamp`, `term`, `k`.
//! - Names, keywords, functions and categories from the symbol library
//!   match fuzzily; recently used symbols rank higher.
//! - A row of nine favourites; 1 to 9 picks one, so common parts are I then
//!   one digit.
//! - A device tag in front (`K1 no`) places a child symbol of that device:
//!   a contact of `-K1`.
//!
//! The mnemonic and favourite tables here are interim: once novo-symbols
//! ships `Library::by_alias` and `favorites()` (WS-05, #25), this search
//! reads them from the library instead of keeping its own.
//!
//! [`letter_code`] gives the IEC 81346-2 letter for the next free tag
//! (`-K1`, `-K2`, ...) of a placed symbol.

use novo_symbols::{Library, Symbol};

use crate::fuzzy;

/// Mnemonic to symbol id. An exact mnemonic is always the top match.
pub const MNEMONICS: &[(&str, &str)] = &[
    ("no", "iec/contacts/no"),
    ("nc", "iec/contacts/nc"),
    ("co", "iec/contacts/changeover"),
    ("coil", "iec/coils/relay"),
    ("k", "iec/contactors/main_3p"),
    ("km", "iec/contactors/main_3p"),
    ("mcb", "iec/protection/mcb_1p"),
    ("mcb3", "iec/protection/mcb_3p"),
    ("mpcb", "iec/protection/motor_protection_breaker"),
    ("ol", "iec/protection/overload_relay"),
    ("fuse", "iec/protection/fuse"),
    ("rcd", "iec/protection/rcd_4p"),
    ("m3", "iec/motors/motor_3ph"),
    ("m1", "iec/motors/motor_1ph"),
    ("pb", "iec/operators/pushbutton_no"),
    ("pbnc", "iec/operators/pushbutton_nc"),
    ("estop", "iec/operators/emergency_stop"),
    ("lamp", "iec/signalling/lamp"),
    ("term", "iec/terminals/terminal"),
    ("pe", "iec/terminals/terminal_pe"),
    ("arrow", "iec/connectivity/interruption_point"),
    ("pot", "iec/connectivity/potential_feed"),
    ("main", "iec/switches/main_switch_3p"),
    ("psu", "iec/supplies/power_supply_dc"),
    ("vfd", "iec/drives/frequency_converter"),
];

/// Favourites when the user has not chosen their own: what a control
/// cabinet designer places most.
pub const DEFAULT_FAVOURITES: [&str; 9] = [
    "iec/contacts/no",
    "iec/contacts/nc",
    "iec/coils/relay",
    "iec/contactors/main_3p",
    "iec/protection/motor_protection_breaker",
    "iec/protection/overload_relay",
    "iec/motors/motor_3ph",
    "iec/operators/pushbutton_no",
    "iec/terminals/terminal",
];

/// A parsed insert query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsertQuery<'a> {
    /// The device a child symbol belongs to, as typed (`K1`, `-K1`).
    pub parent: Option<&'a str>,
    /// What to search symbols for.
    pub text: &'a str,
}

/// Splits `K1 no` into the parent `K1` and the search `no`. The first word
/// is a parent only when it looks like a device code (letters then a
/// number, optionally with a `-`) and more words follow.
pub fn parse_query(query: &str) -> InsertQuery<'_> {
    let query = query.trim();
    if let Some((first, rest)) = query.split_once(char::is_whitespace)
        && looks_like_device_code(first)
        && !rest.trim().is_empty()
    {
        return InsertQuery {
            parent: Some(first),
            text: rest.trim(),
        };
    }
    InsertQuery {
        parent: None,
        text: query,
    }
}

fn looks_like_device_code(word: &str) -> bool {
    let word = word.strip_prefix('-').unwrap_or(word);
    let (letters, number, suffix) = novo_core::split_code(word);
    !letters.is_empty()
        && letters.chars().all(|c| c.is_ascii_alphabetic())
        && number.is_some()
        && suffix.is_empty()
}

/// One row of the insert search.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolHit<'l> {
    pub symbol: &'l Symbol,
    pub score: i32,
    /// The mnemonic that matched exactly, shown next to the name.
    pub mnemonic: Option<&'static str>,
}

/// The insert search over a symbol library.
#[derive(Debug, Clone)]
pub struct InsertSearch {
    /// Symbol ids, most recent first.
    recent: Vec<String>,
    favourites: [String; 9],
}

/// How many recent symbols are remembered.
pub const RECENT_LIMIT: usize = 20;

impl Default for InsertSearch {
    fn default() -> Self {
        InsertSearch {
            recent: Vec::new(),
            favourites: DEFAULT_FAVOURITES.map(str::to_owned),
        }
    }
}

impl InsertSearch {
    /// Notes that a symbol was placed, for the recents ranking.
    pub fn used(&mut self, symbol_id: &str) {
        self.recent.retain(|s| s != symbol_id);
        self.recent.insert(0, symbol_id.to_owned());
        self.recent.truncate(RECENT_LIMIT);
    }

    pub fn recent(&self) -> &[String] {
        &self.recent
    }

    pub fn favourites(&self) -> &[String; 9] {
        &self.favourites
    }

    /// Sets favourite `slot` (1 to 9). Out-of-range slots are ignored.
    pub fn set_favourite(&mut self, slot: usize, symbol_id: &str) {
        if let Some(f) = slot.checked_sub(1).and_then(|i| self.favourites.get_mut(i)) {
            *f = symbol_id.to_owned();
        }
    }

    /// The favourite in `slot` (1 to 9), if it is in the library.
    pub fn favourite<'l>(&self, library: &'l Library, slot: usize) -> Option<&'l Symbol> {
        let id = self.favourites.get(slot.checked_sub(1)?)?;
        library.get(id)
    }

    /// Symbols matching `text`, best first. An empty text lists recents and
    /// then favourites.
    pub fn search<'l>(&self, library: &'l Library, text: &str) -> Vec<SymbolHit<'l>> {
        let text = text.trim();
        if text.is_empty() {
            let mut out: Vec<SymbolHit<'l>> = Vec::new();
            let ids = self.recent.iter().chain(self.favourites.iter());
            for (rank, id) in ids.enumerate() {
                if out.iter().any(|h| h.symbol.id == *id) {
                    continue;
                }
                if let Some(symbol) = library.get(id) {
                    out.push(SymbolHit {
                        symbol,
                        score: -i32::try_from(rank).unwrap_or(i32::MAX),
                        mnemonic: None,
                    });
                }
            }
            return out;
        }

        let lower = text.to_lowercase();
        let mut out: Vec<SymbolHit<'l>> = library
            .iter()
            .filter_map(|symbol| {
                let mnemonic = MNEMONICS
                    .iter()
                    .find(|(m, id)| *m == lower && *id == symbol.id)
                    .map(|(m, _)| *m);
                let last = symbol.id.rsplit('/').next().unwrap_or(&symbol.id);
                let texts = [symbol.name.as_str(), last, symbol.function.as_str()]
                    .into_iter()
                    .chain(symbol.keywords.iter().map(String::as_str));
                let mut score = fuzzy::best_score(text, texts);
                // The category alone matches weakly ("protection").
                if score.is_none() {
                    score = fuzzy::score(text, &symbol.category).map(|s| s / 4);
                }
                if mnemonic.is_some() {
                    score = Some(10_000);
                }
                let mut score = score?;
                if let Some(rank) = self.recent.iter().position(|r| *r == symbol.id) {
                    score += 200 - i32::try_from(rank * 10).unwrap_or(200);
                }
                Some(SymbolHit {
                    symbol,
                    score,
                    mnemonic,
                })
            })
            .collect();
        out.sort_by(|a, b| b.score.cmp(&a.score).then(a.symbol.id.cmp(&b.symbol.id)));
        out
    }
}

/// The IEC 81346-2 letter code a new device for this symbol is tagged with
/// (the classic single letters most cabinet builders use). `None` for
/// symbols that are not devices of their own (interruption points,
/// potential feeds, bridges, earth).
pub fn letter_code(symbol: &Symbol) -> Option<&'static str> {
    let f = symbol.function.as_str();
    let has = |prefix: &str| f == prefix || f.starts_with(&format!("{prefix}."));
    Some(if has("connectivity") || has("earth") {
        return None;
    } else if has("terminal") || has("connector") {
        "X"
    } else if has("coil.valve") {
        "Y"
    } else if has("coil") || has("contact.main") {
        "K"
    } else if [".push", ".key", ".turn", ".estop"]
        .iter()
        .any(|s| f.starts_with("contact.") && f.contains(s))
    {
        "S"
    } else if [
        ".position",
        ".float",
        ".pressure",
        ".proximity",
        ".temperature",
    ]
    .iter()
    .any(|s| f.starts_with("contact.") && f.contains(s))
    {
        "B"
    } else if f.starts_with("contact.") && f.contains(".thermal") {
        "F"
    } else if has("contact") {
        "K"
    } else if has("protection") {
        "F"
    } else if has("switch") {
        "Q"
    } else if has("motor") {
        "M"
    } else if has("source.generator") || has("source") {
        "G"
    } else if has("converter") {
        if f == "converter.ac_dc" || f == "converter.dc_dc" {
            "G"
        } else {
            "T"
        }
    } else if has("transformer") {
        "T"
    } else if has("meter") {
        "P"
    } else if has("signal") {
        "H"
    } else if has("sensor") {
        "B"
    } else if has("load.heater") {
        "E"
    } else if has("plc") {
        "A"
    } else if has("passive.capacitor") {
        "C"
    } else if has("passive.inductor") || has("passive.filter") {
        "L"
    } else if ["diode", "rectifier", "transistor", "thyristor", "triac"]
        .iter()
        .any(|p| has(&format!("passive.{p}")))
    {
        "V"
    } else if has("passive") {
        "R"
    } else {
        "A"
    })
}

/// The tag a newly placed symbol's device gets: the next free number for
/// its letter code within `structure` (the page's function and location
/// aspects). `None` for symbols that are not devices of their own.
pub fn next_tag(
    project: &novo_core::Project,
    structure: &novo_core::DeviceTag,
    symbol: &Symbol,
) -> Option<novo_core::DeviceTag> {
    let code = letter_code(symbol)?;
    let number = project.next_tag_number(structure, code);
    let mut tag = structure.clone();
    tag.product.push(format!("{code}{number}"));
    Some(tag)
}

/// Whether a symbol is normally a function of another device (an auxiliary
/// contact of a contactor) rather than a device of its own. Such symbols
/// typed without a parent get a device of their own, and the checker flags
/// them as having no coil (J2).
pub fn is_child_function(symbol: &Symbol) -> bool {
    let f = symbol.function.as_str();
    f == "contact.no"
        || f == "contact.nc"
        || f == "contact.co"
        || f == "contact.main"
        || f.starts_with("contact.no.delay")
        || f.starts_with("contact.nc.delay")
        || f.starts_with("contact.no.thermal")
        || f.starts_with("contact.nc.thermal")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library() -> Library {
        Library::builtin().unwrap()
    }

    #[test]
    fn every_mnemonic_and_favourite_exists() {
        let lib = library();
        for (m, id) in MNEMONICS {
            assert!(lib.get(id).is_some(), "{m} -> {id}");
        }
        for id in DEFAULT_FAVOURITES {
            assert!(lib.get(id).is_some(), "{id}");
        }
    }

    #[test]
    fn mnemonics_are_the_top_match() {
        let lib = library();
        let search = InsertSearch::default();
        for (m, id) in MNEMONICS {
            let hits = search.search(&lib, m);
            // Several mnemonics may name the same symbol; the first hit is it.
            assert_eq!(hits[0].symbol.id, *id, "{m}");
        }
    }

    #[test]
    fn names_and_keywords_match() {
        let lib = library();
        let search = InsertSearch::default();
        // "contactor" finds the coil (the device itself) or the main contacts.
        let first = &search.search(&lib, "contactor")[0].symbol.category;
        assert!(first == "coils" || first == "contactors", "{first}");
        assert!(
            search
                .search(&lib, "normally open")
                .iter()
                .take(3)
                .any(|h| h.symbol.id == "iec/contacts/no")
        );
        assert!(search.search(&lib, "zzzz").is_empty());
    }

    #[test]
    fn recents_rank_first_and_fill_the_empty_search() {
        let lib = library();
        let mut search = InsertSearch::default();
        search.used("iec/signalling/lamp");
        let empty = search.search(&lib, "");
        assert_eq!(empty[0].symbol.id, "iec/signalling/lamp");
        assert_eq!(empty.len(), 10);
        search.used("iec/contacts/nc_delay_on");
        let hits = search.search(&lib, "nc d");
        assert_eq!(hits[0].symbol.id, "iec/contacts/nc_delay_on");
    }

    #[test]
    fn favourites_by_digit() {
        let lib = library();
        let mut search = InsertSearch::default();
        assert_eq!(search.favourite(&lib, 1).unwrap().id, "iec/contacts/no");
        search.set_favourite(9, "iec/signalling/lamp");
        assert_eq!(search.favourite(&lib, 9).unwrap().id, "iec/signalling/lamp");
        assert!(search.favourite(&lib, 0).is_none());
        assert!(search.favourite(&lib, 10).is_none());
    }

    #[test]
    fn child_query_splits_off_the_parent() {
        assert_eq!(
            parse_query("K1 no"),
            InsertQuery {
                parent: Some("K1"),
                text: "no"
            }
        );
        assert_eq!(parse_query("-K12 nc").parent, Some("-K12"));
        assert_eq!(parse_query("no").parent, None);
        assert_eq!(parse_query("K1").parent, None);
        // "m3 motor" is a search, not a child of -M3? It is a device code,
        // so it is read as one; a plain word first is never a parent.
        assert_eq!(parse_query("motor 3").parent, None);
    }

    #[test]
    fn letter_codes_match_the_gate_1_project() {
        let lib = library();
        let code = |id: &str| letter_code(lib.get(id).unwrap());
        assert_eq!(code("iec/protection/fuse_3p"), Some("F"));
        assert_eq!(code("iec/contactors/main_3p"), Some("K"));
        assert_eq!(code("iec/coils/relay"), Some("K"));
        assert_eq!(code("iec/protection/overload_relay"), Some("F"));
        assert_eq!(code("iec/terminals/terminal"), Some("X"));
        assert_eq!(code("iec/motors/motor_3ph"), Some("M"));
        assert_eq!(code("iec/protection/mcb_1p"), Some("F"));
        assert_eq!(code("iec/operators/pushbutton_no"), Some("S"));
        assert_eq!(code("iec/connectivity/interruption_point"), None);
        // Every device symbol gets some code.
        for symbol in lib.iter() {
            if let Some(c) = letter_code(symbol) {
                assert!(!c.is_empty());
            }
        }
    }

    #[test]
    fn ten_contactors_get_ten_tags_without_input() {
        let lib = library();
        let coil = lib.get("iec/coils/relay").unwrap();
        let structure = novo_core::DeviceTag::parse("=A1+CAB").unwrap();
        let mut project = novo_core::Project::new("p");
        let mut tags = Vec::new();
        for _ in 0..10 {
            let tag = next_tag(&project, &structure, coil).unwrap();
            project
                .transact("Place", |tx| tx.add_device(tag.clone(), ""))
                .unwrap();
            tags.push(tag.to_string());
        }
        assert_eq!(tags[0], "=A1+CAB-K1");
        assert_eq!(tags[9], "=A1+CAB-K10");
        let arrow = lib.get("iec/connectivity/interruption_point").unwrap();
        assert!(next_tag(&project, &structure, arrow).is_none());
    }

    #[test]
    fn auxiliary_contacts_are_child_functions() {
        let lib = library();
        assert!(is_child_function(lib.get("iec/contacts/no").unwrap()));
        assert!(is_child_function(
            lib.get("iec/contacts/thermal_nc").unwrap()
        ));
        assert!(!is_child_function(
            lib.get("iec/operators/pushbutton_no").unwrap()
        ));
        assert!(!is_child_function(lib.get("iec/coils/relay").unwrap()));
    }
}

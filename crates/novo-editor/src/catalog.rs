//! What can be placed, and how to find it from the keyboard.
//!
//! The [`Catalog`] wraps the WS-05 symbol library with the things the editor
//! needs on top: what kind of model object a symbol becomes when placed
//! ([`SymbolClass`]), the IEC 81346-2 letter code of its device, short
//! mnemonics (`no`, `nc`, `coil`, `m3`) and a search that ranks recently used
//! symbols first (DESIGN.md §4, I and the favourite digits).

use novo_core::{Direction, Pin, TerminalKind};
use novo_symbols::{Library, Pack, Symbol};

/// What a symbol turns into when it is placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolClass {
    /// The main function of a new device (`-K1` coil, `-M1` motor, `-S1`
    /// push button). Placing one creates the device with the next free tag
    /// for `letter`.
    Main { letter: &'static str },
    /// A secondary function (an auxiliary or main contact). With a parent
    /// device (`K1 no` in the search) it belongs to that device and takes
    /// the next free contact numbers; without one it gets a new device of
    /// `letter` that the checker flags as having no coil.
    Child { letter: &'static str },
    /// A terminal of a terminal strip.
    Terminal { kind: TerminalKind },
    /// A potential definition point.
    PotentialFeed,
    /// An off-page connector.
    Interruption,
    /// Drawn without a device (earth symbols, bridges).
    Plain,
}

/// Classifies a library symbol by its function type.
pub fn classify(symbol: &Symbol) -> SymbolClass {
    let f = symbol.function.as_str();
    let has = |part: &str| f.split('.').any(|p| p == part);
    if f.starts_with("terminal") {
        let kind = if has("pe") {
            TerminalKind::ProtectiveEarth
        } else if has("n") {
            TerminalKind::Neutral
        } else if has("fuse") {
            TerminalKind::Fuse
        } else if has("disconnect") {
            TerminalKind::Disconnect
        } else {
            TerminalKind::Feedthrough
        };
        return SymbolClass::Terminal { kind };
    }
    match f {
        "connectivity.potential" => return SymbolClass::PotentialFeed,
        "connectivity.interruption" => return SymbolClass::Interruption,
        _ => {}
    }
    if f.starts_with("connectivity") || f.starts_with("earth") {
        return SymbolClass::Plain;
    }
    let Some(letter) = letter_code(f) else {
        return SymbolClass::Plain;
    };
    if f.starts_with("contact.") && !matches!(letter, "S" | "B") {
        // Not operated by hand or by a process value: part of a device.
        return SymbolClass::Child { letter };
    }
    SymbolClass::Main { letter }
}

/// IEC 81346-2 letter code for a function type, from the shared table in
/// novo-symbols. `None` for symbols that are not devices of their own.
pub fn letter_code(function: &str) -> Option<&'static str> {
    novo_symbols::letter_code(function)
}

/// `id` in the pack of `library`; unchanged when the library has no single
/// pack (a company library).
fn localize(library: &Library, id: &str) -> String {
    library
        .pack()
        .and_then(|pack| Pack::counterpart(id, pack))
        .unwrap_or_else(|| id.to_owned())
}

/// Short names that rank first in the insert search (DESIGN.md §4), on
/// top of the `aliases` each library symbol carries.
pub const MNEMONICS: &[(&str, &str)] = &[
    ("no", "iec/contacts/no"),
    ("nc", "iec/contacts/nc"),
    ("co", "iec/contacts/changeover"),
    ("coil", "iec/coils/relay"),
    ("k", "iec/coils/relay"),
    ("contactor", "iec/coils/contactor"),
    ("relay", "iec/coils/relay"),
    ("timer", "iec/coils/timer_on_delay"),
    ("main", "iec/contactors/main_3p"),
    ("k3", "iec/contactors/main_3p"),
    ("mcb", "iec/protection/mcb_1p"),
    ("mcb3", "iec/protection/mcb_3p"),
    ("mpcb", "iec/protection/motor_protection_breaker"),
    ("ol", "iec/protection/overload_relay"),
    ("olnc", "iec/contacts/thermal_nc"),
    ("fuse", "iec/protection/fuse"),
    ("fuse3", "iec/protection/fuse_3p"),
    ("m", "iec/motors/motor_3ph"),
    ("m3", "iec/motors/motor_3ph"),
    ("m1", "iec/motors/motor_1ph"),
    ("pb", "iec/operators/pushbutton_no"),
    ("pbnc", "iec/operators/pushbutton_nc"),
    ("estop", "iec/operators/emergency_stop"),
    ("lamp", "iec/signalling/lamp"),
    ("term", "iec/terminals/terminal"),
    ("x", "iec/terminals/terminal_end"),
    ("pe", "iec/terminals/terminal_pe"),
    ("ms", "iec/switches/main_switch_3p"),
    ("ip", "iec/connectivity/interruption_point"),
    ("pot", "iec/connectivity/potential_feed"),
    ("psu", "iec/supplies/power_supply_dc"),
    ("vfd", "iec/drives/frequency_converter"),
];

/// The default favourites, picked with 1 to 9 while the insert search is
/// open.
pub const DEFAULT_FAVOURITES: [&str; 9] = [
    "iec/coils/relay",
    "iec/contacts/no",
    "iec/contacts/nc",
    "iec/contactors/main_3p",
    "iec/protection/motor_protection_breaker",
    "iec/protection/mcb_1p",
    "iec/operators/pushbutton_no",
    "iec/signalling/lamp",
    "iec/terminals/terminal_end",
];

/// One row of the insert search.
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    /// Library id of the symbol, e.g. `iec/contacts/no`.
    pub symbol: String,
    /// Display name.
    pub name: String,
    /// The parent device typed before the symbol words (`K1 no` gives
    /// `-K1`), for child symbols.
    pub parent: Option<String>,
    /// Why it matched, for the row's hint text (`no`, `recent`, ...).
    pub hint: String,
}

/// The symbol library plus recent and favourite symbols.
pub struct Catalog {
    library: Library,
    recent: Vec<String>,
    favourites: Vec<String>,
}

/// How many recently used symbols are remembered.
const RECENT_LIMIT: usize = 16;

impl Catalog {
    /// The catalog of the built-in IEC library.
    pub fn builtin() -> Self {
        Catalog::builtin_pack(Pack::Iec)
    }

    /// The catalog of one built-in pack (the project's drawing standard).
    pub fn builtin_pack(pack: Pack) -> Self {
        let library = Library::builtin_pack(pack).expect("the built-in symbol library is valid");
        Catalog::new(library)
    }

    pub fn new(library: Library) -> Self {
        let favourites = DEFAULT_FAVOURITES
            .iter()
            .map(|id| localize(&library, id))
            .filter(|id| library.get(id).is_some())
            .collect();
        Catalog {
            library,
            recent: Vec::new(),
            favourites,
        }
    }

    /// The id of the same device in this catalog's pack: `in_pack("iec/
    /// contacts/no")` is `nfpa/contacts/no` in an NFPA catalog. The editor's
    /// own tables (mnemonics, favourites, the potential feed) name IEC ids.
    pub fn in_pack(&self, id: &str) -> String {
        localize(&self.library, id)
    }

    pub fn library(&self) -> &Library {
        &self.library
    }

    pub fn get(&self, id: &str) -> Option<&Symbol> {
        self.library.get(id)
    }

    /// Recently used symbols, most recent first.
    pub fn recent(&self) -> &[String] {
        &self.recent
    }

    /// Records a use, so the symbol ranks first next time.
    pub fn used(&mut self, id: &str) {
        self.recent.retain(|r| r != id);
        self.recent.insert(0, id.to_owned());
        self.recent.truncate(RECENT_LIMIT);
    }

    /// The favourites row (up to nine), in key order 1 to 9.
    pub fn favourites(&self) -> &[String] {
        &self.favourites
    }

    /// Replaces the favourites; unknown ids are dropped, at most nine kept.
    pub fn set_favourites(&mut self, ids: impl IntoIterator<Item = String>) {
        self.favourites = ids
            .into_iter()
            .filter(|id| self.library.get(id).is_some())
            .take(9)
            .collect();
    }

    /// The favourite on key `n` (1 to 9).
    pub fn favourite(&self, n: usize) -> Option<&str> {
        n.checked_sub(1)
            .and_then(|i| self.favourites.get(i))
            .map(String::as_str)
    }

    /// Searches symbols. An empty query lists recent symbols, then
    /// favourites. A first word that looks like a device code (`K1`, `-K1`,
    /// `=A1-K1`) followed by more words names the parent of a child symbol:
    /// `K1 no` is a normally open contact of `-K1`.
    pub fn search(&self, query: &str) -> Vec<Match> {
        let query = query.trim().to_lowercase();
        let mut words: Vec<&str> = query.split_whitespace().collect();
        let mut parent = None;
        // A trailing device code (`km3 K1`) makes the first word the symbol,
        // even when that word also reads as a device code (`k3` is both the
        // contactor's name and -K3: `k3 no` is still a contact of -K3).
        let tag_last = words.len() >= 2
            && words
                .last()
                .is_some_and(|l| looks_like_device(l) && !self.is_symbol_word(l));
        if words.len() >= 2
            && looks_like_device(words[0])
            && !(tag_last && self.is_symbol_word(words[0]))
        {
            let code = words.remove(0);
            parent = Some(normalize_parent(code));
        } else if let [.., last] = words[..]
            && tag_last
        {
            // Verb before noun (DESIGN.md §4): `no K1`, `coil K1`.
            words.pop();
            parent = Some(normalize_parent(last));
        }
        let mut scored: Vec<(i64, Match)> = Vec::new();
        for symbol in self.library.iter() {
            let Some((score, hint)) = self.score(symbol, &words) else {
                continue;
            };
            // A device's own symbols: its main function (`coil K1` when
            // -K1 has no coil yet) and its contacts.
            if parent.is_some()
                && !matches!(
                    classify(symbol),
                    SymbolClass::Child { .. } | SymbolClass::Main { .. }
                )
            {
                continue;
            }
            scored.push((
                score,
                Match {
                    symbol: symbol.id.clone(),
                    name: symbol.name.clone(),
                    parent: parent.clone(),
                    hint,
                },
            ));
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.symbol.cmp(&b.1.symbol)));
        scored.into_iter().map(|(_, m)| m).collect()
    }

    /// Whether a typed word is a short name of a symbol (`m3`), so it is
    /// not read as a device code.
    fn is_symbol_word(&self, word: &str) -> bool {
        MNEMONICS.iter().any(|(m, _)| *m == word)
            || self
                .library
                .iter()
                .any(|s| s.aliases.iter().any(|a| a.eq_ignore_ascii_case(word)))
    }

    fn score(&self, symbol: &Symbol, words: &[&str]) -> Option<(i64, String)> {
        let recency = self
            .recent
            .iter()
            .position(|r| *r == symbol.id)
            .map_or(0, |i| 50 - i as i64);
        if words.is_empty() {
            let favourite = self.favourites.iter().position(|f| *f == symbol.id);
            return match (recency, favourite) {
                (0, None) => None,
                (0, Some(i)) => Some((20 - i as i64, format!("{}", i + 1))),
                (r, _) => Some((100 + r, "recent".into())),
            };
        }
        let joined = words.join(" ");
        let mut best = 0i64;
        let mut hint = String::new();
        // The library's own aliases (WS-05) count the same as the editor's.
        let library_aliases = symbol
            .aliases
            .iter()
            .map(|a| (a.as_str(), symbol.id.as_str()));
        for (mnemonic, id) in MNEMONICS.iter().copied().chain(library_aliases) {
            if Pack::same_device(id, &symbol.id) {
                if mnemonic == joined {
                    best = best.max(1000);
                    hint = mnemonic.into();
                } else if mnemonic.starts_with(joined.as_str()) {
                    best = best.max(400);
                    hint = mnemonic.into();
                }
            }
        }
        let name = symbol.name.to_lowercase();
        let short = symbol.id.rsplit('/').next().unwrap_or("").to_lowercase();
        let mut all_words = 0;
        for word in words {
            let hit = if short == *word {
                300
            } else if name
                .split(|c: char| !c.is_alphanumeric())
                .any(|w| w == *word)
            {
                200
            } else if symbol.keywords.iter().any(|k| k.to_lowercase() == *word) {
                180
            } else if name.contains(word) || short.contains(word) {
                100
            } else if symbol
                .keywords
                .iter()
                .any(|k| k.to_lowercase().contains(word))
            {
                80
            } else if symbol.category.contains(word) || symbol.function.contains(word) {
                50
            } else {
                return (best > 0).then(|| (best + recency, hint.clone()));
            };
            all_words += hit;
        }
        if all_words > best {
            best = all_words;
            hint.clear();
        }
        (best > 0).then(|| (best + recency, hint))
    }
}

fn looks_like_device(word: &str) -> bool {
    let code = word.rsplit('-').next().unwrap_or(word);
    let (letters, number, _) = novo_core::split_code(code);
    !letters.is_empty() && number.is_some() && letters.chars().all(|c| c.is_ascii_alphabetic())
}

/// `k1` gives `-K1`; `=a1-k1` gives `=A1-K1`.
fn normalize_parent(code: &str) -> String {
    let upper = code.to_uppercase();
    if upper.starts_with(['-', '=', '+']) {
        upper
    } else {
        format!("-{upper}")
    }
}

/// The model pins of a library symbol, in connection order.
pub fn model_pins(symbol: &Symbol) -> Vec<Pin> {
    symbol
        .connections
        .iter()
        .map(|c| {
            let pin = Pin::new(c.id.clone(), c.at[0], c.at[1]).with_direction(direction(c.dir));
            if c.optional { pin.optional() } else { pin }
        })
        .collect()
}

/// Bridges and pass-through groups of a library symbol as the model stores
/// them (pass-through without the bridges).
pub fn model_groups(symbol: &Symbol) -> (Vec<Vec<u16>>, Vec<Vec<u16>>) {
    let to16 = |g: &Vec<usize>| -> Vec<u16> {
        g.iter()
            .filter_map(|&i| u16::try_from(i).ok())
            .collect::<Vec<_>>()
    };
    let bridges: Vec<Vec<u16>> = symbol.bridges().iter().map(to16).collect();
    let pass: Vec<Vec<u16>> = symbol
        .pass_through()
        .paths()
        .iter()
        .map(to16)
        .filter(|g| !bridges.contains(g))
        .collect();
    (bridges, pass)
}

pub fn direction(d: novo_symbols::Direction) -> Direction {
    match d {
        novo_symbols::Direction::Up => Direction::Up,
        novo_symbols::Direction::Down => Direction::Down,
        novo_symbols::Direction::Left => Direction::Left,
        novo_symbols::Direction::Right => Direction::Right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mnemonics_point_at_real_symbols() {
        let catalog = Catalog::builtin();
        for (m, id) in MNEMONICS {
            assert!(
                catalog.get(id).is_some(),
                "{m} -> {id} is not in the library"
            );
        }
        assert_eq!(catalog.favourites().len(), 9);
    }

    #[test]
    fn mnemonic_ranks_first() {
        let catalog = Catalog::builtin();
        for (query, id) in [
            ("no", "iec/contacts/no"),
            ("nc", "iec/contacts/nc"),
            ("coil", "iec/coils/relay"),
            ("m3", "iec/motors/motor_3ph"),
            ("mpcb", "iec/protection/motor_protection_breaker"),
            ("con", "iec/coils/contactor"),
            ("main", "iec/contactors/main_3p"),
        ] {
            let first = &catalog.search(query)[0];
            assert_eq!(first.symbol, id, "query {query:?}");
        }
    }

    #[test]
    fn device_prefix_makes_a_child_search() {
        let catalog = Catalog::builtin();
        let hits = catalog.search("K1 no");
        assert_eq!(hits[0].symbol, "iec/contacts/no");
        assert_eq!(hits[0].parent.as_deref(), Some("-K1"));
        assert!(hits.iter().all(|h| matches!(
            classify(catalog.get(&h.symbol).unwrap()),
            SymbolClass::Child { .. } | SymbolClass::Main { .. }
        )));
    }

    #[test]
    fn a_symbol_mnemonic_with_a_tag_is_a_search_not_a_device_code() {
        // `km3` looks like a device code, but it is the contactor's name:
        // `km3 K1` is that contactor as -K1.
        let catalog = Catalog::builtin();
        let hits = catalog.search("km3 K1");
        assert!(!hits.is_empty(), "the palette advertises this string");
        assert_eq!(hits[0].parent.as_deref(), Some("-K1"));
        assert_eq!(catalog.search("km3")[0].symbol, hits[0].symbol);
    }

    #[test]
    fn a_mnemonic_that_is_also_a_device_code_still_names_the_parent_first() {
        // `k3` and `m1` are symbol names, and also the codes -K3 and -M1.
        let catalog = Catalog::builtin();
        let hit = &catalog.search("k3 no")[0];
        assert_eq!(hit.symbol, "iec/contacts/no");
        assert_eq!(hit.parent.as_deref(), Some("-K3"));
        let hit = &catalog.search("m1 coil")[0];
        assert_eq!(hit.symbol, "iec/coils/relay");
        assert_eq!(hit.parent.as_deref(), Some("-M1"));
        // With a tag after it, the same word is the symbol again.
        let hit = &catalog.search("k3 K1")[0];
        assert_eq!(hit.symbol, "iec/contactors/main_3p");
        assert_eq!(hit.parent.as_deref(), Some("-K1"));
    }

    #[test]
    fn verb_before_device_names_the_parent() {
        let catalog = Catalog::builtin();
        for (query, symbol) in [
            ("coil K1", "iec/coils/relay"),
            ("no K1", "iec/contacts/no"),
            ("nc -k1", "iec/contacts/nc"),
            ("K1 coil", "iec/coils/relay"),
        ] {
            let hit = &catalog.search(query)[0];
            assert_eq!(hit.symbol, symbol, "{query}");
            assert_eq!(hit.parent.as_deref(), Some("-K1"), "{query}");
        }
        // A symbol's short name is not a device code.
        assert!(
            catalog
                .search("motor m3")
                .iter()
                .all(|h| h.parent.is_none())
        );
        assert!(catalog.search("no m3").iter().all(|h| h.parent.is_none()));
    }

    #[test]
    fn recent_symbols_rank_first_for_empty_query() {
        let mut catalog = Catalog::builtin();
        catalog.used("iec/signalling/lamp");
        catalog.used("iec/motors/motor_3ph");
        let hits = catalog.search("");
        assert_eq!(hits[0].symbol, "iec/motors/motor_3ph");
        assert_eq!(hits[1].symbol, "iec/signalling/lamp");
    }

    #[test]
    fn spare_pins_stay_optional_in_the_model() {
        let catalog = Catalog::builtin();
        for (id, spare) in [
            ("iec/drives/frequency_converter", "PE2"),
            ("iec/transformers/emc_filter", "PE'"),
        ] {
            let pins = model_pins(catalog.get(id).unwrap());
            let optional: Vec<&str> = pins
                .iter()
                .filter(|p| p.optional)
                .map(|p| p.name.as_str())
                .collect();
            assert_eq!(optional, [spare], "{id}");
        }
        let coil = model_pins(catalog.get("iec/coils/relay").unwrap());
        assert!(coil.iter().all(|p| !p.optional));
    }

    #[test]
    fn every_symbol_classifies() {
        let catalog = Catalog::builtin();
        let classes: Vec<_> = catalog.library().iter().map(|s| (s, classify(s))).collect();
        let class_of = |id: &str| classes.iter().find(|(s, _)| s.id == id).unwrap().1;
        assert_eq!(
            class_of("iec/coils/relay"),
            SymbolClass::Main { letter: "K" }
        );
        assert_eq!(
            class_of("iec/contacts/no"),
            SymbolClass::Child { letter: "K" }
        );
        assert_eq!(
            class_of("iec/contactors/main_3p"),
            SymbolClass::Child { letter: "K" }
        );
        assert_eq!(
            class_of("iec/motors/motor_3ph"),
            SymbolClass::Main { letter: "M" }
        );
        assert_eq!(
            class_of("iec/operators/pushbutton_no"),
            SymbolClass::Main { letter: "S" }
        );
        assert_eq!(
            class_of("iec/protection/overload_relay"),
            SymbolClass::Main { letter: "F" }
        );
        // Breakers by purpose (DESIGN.md letter codes): MCB F, MPCB Q.
        assert_eq!(
            class_of("iec/protection/mcb_1p"),
            SymbolClass::Main { letter: "F" }
        );
        assert_eq!(
            class_of("iec/protection/motor_protection_breaker"),
            SymbolClass::Main { letter: "Q" }
        );
        assert_eq!(
            class_of("iec/contacts/thermal_nc"),
            SymbolClass::Child { letter: "F" }
        );
        assert_eq!(
            class_of("iec/terminals/terminal_pe"),
            SymbolClass::Terminal {
                kind: TerminalKind::ProtectiveEarth
            }
        );
        assert_eq!(
            class_of("iec/connectivity/potential_feed"),
            SymbolClass::PotentialFeed
        );
        assert_eq!(class_of("iec/earth/protective_earth"), SymbolClass::Plain);
    }
}

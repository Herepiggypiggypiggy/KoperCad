//! EPLAN symbols and functions <-> KoperCAD symbols, in both directions.
//!
//! An EPLAN device reaches us as an [`EplanSymbol`]: whatever the project says
//! about it (the symbol number, the function name, the tag letters, the pin
//! designations). [`SymbolMapper::map`] turns that into a KoperCAD symbol, the
//! pin for pin connection map and how sure the match is. The other way,
//! [`SymbolMapper::to_eplan`] gives the function name, tag letter and pin
//! designations to write for a KoperCAD symbol.
//!
//! The table (`table.toml`) is our own: device meaning in three languages, IEC
//! 81346-2 letters and EN 50005 pin designations. EPLAN's symbol library and
//! macros are not read or copied; the numeric symbol keys of a user's own EPLAN
//! installation are learnt from their export ([`SymbolMapper::learn`]) and kept
//! in a [`UserTable`] that starts empty.
//!
//! Resolution order, most certain first:
//! 1. the user's table (exact symbol key),
//! 2. the function name, then letters and pin fit to pick between look-alikes
//!    (a 3 pole and a 4 pole breaker share a name, the pins tell them apart),
//! 3. tag letters with a pin set that fits one device completely (only for
//!    devices with no function text at all),
//! 4. a generic box that keeps every pin ([`generic_symbol`]).
//!
//! Nothing is dropped silently: [`MappingReport`] counts how each device was
//! resolved and lists the ones that fell back or lost pins.

mod generic;
mod pins;
mod table;
mod user;

use std::collections::BTreeMap;

use novo_symbols::{Library, Pack, Symbol, letter_code};

use crate::report::Report;

pub use generic::generic_symbol;
pub use pins::{PinMap, match_pins};
pub use table::{Entry, Table, name_score, normalize};
pub use user::{SymbolKey, UserTable};

/// What a project says about one EPLAN device. Every field is optional: the
/// mapper uses what it is given.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EplanSymbol {
    /// The drawn symbol's library set, number and variant.
    pub key: Option<SymbolKey>,
    /// The function definition or symbol name as the project shows it.
    pub function: Option<String>,
    /// The tag letters, digits and separators removed (`K` for `-K12`).
    pub letters: Option<String>,
    /// Pin designations in the order the device has them.
    pub pins: Vec<String>,
}

impl EplanSymbol {
    /// A short label for reports.
    pub fn label(&self) -> String {
        if let Some(key) = self.key {
            let mut label = format!("symbol {key}");
            if let Some(f) = self.function.as_deref().filter(|f| !f.trim().is_empty()) {
                label.push_str(&format!(" ({})", f.trim()));
            }
            label
        } else if let Some(f) = self.function.as_deref().filter(|f| !f.trim().is_empty()) {
            f.trim().to_string()
        } else if let Some(l) = self.letters.as_deref().filter(|l| !l.trim().is_empty()) {
            format!("letters {}", l.trim())
        } else {
            "unidentified device".to_string()
        }
    }
}

/// How a device was resolved, most certain first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Basis {
    /// The user's own symbol table.
    UserTable,
    /// The function name matched.
    Name,
    /// Tag letters and a pin set that fits one device.
    Letters,
    /// Nothing matched: a generic box.
    Generic,
}

impl Basis {
    pub fn describe(self) -> &'static str {
        match self {
            Basis::UserTable => "from your symbol table",
            Basis::Name => "by function name",
            Basis::Letters => "by tag letters and pins",
            Basis::Generic => "as a generic box",
        }
    }
}

/// What the device becomes.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// A symbol of the library, by id in the mapper's pack.
    Library(String),
    /// A stand-in for the project to carry (not in any library).
    Generic(Box<Symbol>),
}

/// The outcome of mapping one device.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub target: Target,
    pub basis: Basis,
    pub pins: PinMap,
    /// Other symbols fitted equally well; the first was taken.
    pub ambiguous: bool,
}

impl Resolved {
    /// The library id, or `None` for a generic box.
    pub fn symbol_id(&self) -> Option<&str> {
        match &self.target {
            Target::Library(id) => Some(id),
            Target::Generic(_) => None,
        }
    }
}

/// What to write into EPLAN for a KoperCAD symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EplanTarget {
    /// The device name to write as function text.
    pub function: String,
    /// Every name the table knows for it.
    pub names: Vec<String>,
    /// The class letter for the tag (IEC 81346-2).
    pub letter: Option<String>,
    /// Pin designations, one per connection, in the symbol's order. EN 50005
    /// designations are the same in both programs.
    pub pins: Vec<String>,
    /// The user's own EPLAN symbol key, when their table has one.
    pub key: Option<SymbolKey>,
    /// Written from the symbol's own text because the table has no row
    /// (a user's own symbol).
    pub generic: bool,
}

/// An EPLAN device that fell back or lost pins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unmapped {
    pub label: String,
    pub count: usize,
    pub pins: Vec<String>,
}

/// Counts of how a batch of devices was mapped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MappingReport {
    pub by_basis: BTreeMap<Basis, usize>,
    /// Devices that became generic boxes, grouped by label.
    pub unmapped: Vec<Unmapped>,
    /// Devices mapped but with pins that found no connection, by label.
    pub pin_issues: Vec<Unmapped>,
    /// Devices where equally good symbols existed.
    pub ambiguous: usize,
}

impl MappingReport {
    pub fn total(&self) -> usize {
        self.by_basis.values().sum()
    }

    fn add(&mut self, source: &EplanSymbol, resolved: &Resolved) {
        *self.by_basis.entry(resolved.basis).or_default() += 1;
        if resolved.ambiguous {
            self.ambiguous += 1;
        }
        let list = if resolved.basis == Basis::Generic {
            Some(&mut self.unmapped)
        } else if !resolved.pins.unmapped.is_empty() {
            Some(&mut self.pin_issues)
        } else {
            None
        };
        if let Some(list) = list {
            let label = source.label();
            match list.iter_mut().find(|u| u.label == label) {
                Some(u) => u.count += 1,
                None => list.push(Unmapped {
                    label,
                    count: 1,
                    pins: if resolved.basis == Basis::Generic {
                        source.pins.clone()
                    } else {
                        resolved.pins.unmapped.clone()
                    },
                }),
            }
        }
    }

    /// Adds the counts to an import [`Report`].
    pub fn write_to(&self, report: &mut Report) {
        for (basis, n) in &self.by_basis {
            if *basis != Basis::Generic {
                report.moved(format!("devices mapped {}", basis.describe()), *n);
            }
        }
        let generic: usize = self.unmapped.iter().map(|u| u.count).sum();
        report.moved("devices carried as generic boxes", generic);
        for u in &self.unmapped {
            report.note(format!(
                "no KoperCAD symbol for {} ({}x, {} pins): carried as a generic box",
                u.label,
                u.count,
                u.pins.len()
            ));
        }
        for u in &self.pin_issues {
            report.lost(
                format!("pins of {}", u.label),
                u.pins.len() * u.count,
                format!(
                    "pin(s) {} match no connection of the symbol",
                    u.pins.join(", ")
                ),
            );
        }
        if self.ambiguous > 0 {
            report.note(format!(
                "{} device(s) fit several symbols equally; the first was taken, check them",
                self.ambiguous
            ));
        }
    }
}

/// Maps between EPLAN devices and the symbols of one pack.
#[derive(Debug, Clone)]
pub struct SymbolMapper {
    pack: Pack,
    user: UserTable,
}

/// A name match must reach this score; below it only letters are tried.
const MIN_NAME_SCORE: u32 = 50;

struct Candidate<'t> {
    entry: &'t Entry,
    index: usize,
    score: u32,
    pins: PinMap,
}

impl SymbolMapper {
    pub fn new(pack: Pack) -> SymbolMapper {
        SymbolMapper {
            pack,
            user: UserTable::new(),
        }
    }

    pub fn with_user_table(mut self, user: UserTable) -> SymbolMapper {
        self.user = user;
        self
    }

    pub fn pack(&self) -> Pack {
        self.pack
    }

    pub fn user_table(&self) -> &UserTable {
        &self.user
    }

    fn library(&self) -> &'static Library {
        Library::shared(self.pack)
    }

    fn symbol(&self, entry: &Entry) -> Option<&'static Symbol> {
        self.library()
            .get(&format!("{}/{}", self.pack.id(), entry.symbol))
    }

    fn pin_fit(&self, entry: &Entry, pins: &[String]) -> Option<PinMap> {
        let symbol = self.symbol(entry)?;
        let aliases = entry
            .pins
            .as_ref()
            .and_then(|set| Table::builtin().pinsets.get(set));
        Some(match_pins(pins, symbol, aliases))
    }

    /// Maps one device.
    pub fn map(&self, source: &EplanSymbol) -> Resolved {
        let table = Table::builtin();

        if let Some(path) = source.key.and_then(|k| self.user.get(k))
            && let Some(entry) = table.entry_for(path)
            && let Some(pins) = self.pin_fit(entry, &source.pins)
        {
            return Resolved {
                target: Target::Library(format!("{}/{}", self.pack.id(), entry.symbol)),
                basis: Basis::UserTable,
                pins,
                ambiguous: false,
            };
        }
        // A user row can name a symbol the table has no row for (their own
        // pack symbol): trust it with identity pins.
        if let Some(path) = source.key.and_then(|k| self.user.get(k))
            && let Some(symbol) = self.library().get(&format!("{}/{path}", self.pack.id()))
        {
            return Resolved {
                target: Target::Library(symbol.id.clone()),
                basis: Basis::UserTable,
                pins: match_pins(&source.pins, symbol, None),
                ambiguous: false,
            };
        }

        let letters = source
            .letters
            .as_deref()
            .map(|l| l.trim().to_uppercase())
            .filter(|l| !l.is_empty());

        // 2. By name.
        let input = source
            .function
            .as_deref()
            .map(table::tokens)
            .unwrap_or_default();
        if !input.is_empty() {
            let mut found: Vec<Candidate> = Vec::new();
            for (index, entry) in table.entries.iter().enumerate() {
                let best = entry
                    .tokens
                    .iter()
                    .map(|t| name_score(&input, t))
                    .max()
                    .unwrap_or(0);
                if best < MIN_NAME_SCORE {
                    continue;
                }
                let Some(pins) = self.pin_fit(entry, &source.pins) else {
                    continue;
                };
                let boost = match letters.as_deref() {
                    Some(l) if entry.letters.first().map(String::as_str) == Some(l) => 5,
                    Some(l) if entry.letters.iter().any(|e| e == l) => 2,
                    _ => 0,
                };
                found.push(Candidate {
                    entry,
                    index,
                    score: best + boost,
                    pins,
                });
            }
            if let Some(best) = pick(found, !source.pins.is_empty()) {
                return self.resolved(best.0, Basis::Name, best.1);
            }
        }

        // 3. By letters and a complete pin fit. A device that has a function
        // text nobody recognised stays generic: guessing from letters alone
        // would put a weighing amplifier (A, pins 1 2) in as a relay output.
        if let Some(l) = letters.as_deref()
            && input.is_empty()
            && !source.pins.is_empty()
        {
            let mut found: Vec<Candidate> = Vec::new();
            for (index, entry) in table.entries.iter().enumerate() {
                let rank = match entry.letters.iter().position(|e| e == l) {
                    Some(0) => 20,
                    Some(_) => 10,
                    None => continue,
                };
                let Some(pins) = self.pin_fit(entry, &source.pins) else {
                    continue;
                };
                if !pins.is_complete() || pins.positional {
                    continue;
                }
                found.push(Candidate {
                    entry,
                    index,
                    score: rank,
                    pins,
                });
            }
            if let Some(best) = pick(found, true) {
                return self.resolved(best.0, Basis::Letters, best.1);
            }
        }

        // 4. Generic.
        let label = source
            .function
            .clone()
            .filter(|f| !f.trim().is_empty())
            .or_else(|| source.letters.clone())
            .unwrap_or_default();
        let symbol = generic_symbol(&source.pins, &label);
        let pins = PinMap {
            pairs: source
                .pins
                .iter()
                .cloned()
                .zip(symbol.connections.iter().map(|c| c.id.clone()))
                .collect(),
            ..PinMap::default()
        };
        Resolved {
            target: Target::Generic(Box::new(symbol)),
            basis: Basis::Generic,
            pins,
            ambiguous: false,
        }
    }

    fn resolved(&self, entry: &Entry, basis: Basis, (pins, ambiguous): (PinMap, bool)) -> Resolved {
        Resolved {
            target: Target::Library(format!("{}/{}", self.pack.id(), entry.symbol)),
            basis,
            pins,
            ambiguous,
        }
    }

    /// Maps a batch and counts how it went.
    pub fn map_all<'a>(
        &self,
        devices: impl IntoIterator<Item = &'a EplanSymbol>,
    ) -> (Vec<Resolved>, MappingReport) {
        let mut report = MappingReport::default();
        let out = devices
            .into_iter()
            .map(|d| {
                let r = self.map(d);
                report.add(d, &r);
                r
            })
            .collect();
        (out, report)
    }

    /// The EPLAN side of a library symbol of either pack. `None` for a
    /// symbol the table has no row for; use [`SymbolMapper::to_eplan_symbol`]
    /// for those.
    pub fn to_eplan(&self, symbol_id: &str) -> Option<EplanTarget> {
        let path = Pack::path_of(symbol_id);
        let entry = Table::builtin().entry_for(path)?;
        let symbol = Library::shared(Pack::Iec).get(&format!("iec/{path}"))?;
        Some(EplanTarget {
            function: entry.names.first().cloned().unwrap_or_default(),
            names: entry.names.clone(),
            letter: letter_code(&symbol.function)
                .map(str::to_string)
                .or_else(|| entry.letters.first().cloned()),
            pins: symbol.connections.iter().map(|c| c.id.clone()).collect(),
            key: self.user.key_for(path),
            generic: false,
        })
    }

    /// Like [`SymbolMapper::to_eplan`], but for any symbol: a user's own
    /// symbol is written from its own name, function letter and connections.
    pub fn to_eplan_symbol(&self, symbol: &Symbol) -> EplanTarget {
        self.to_eplan(&symbol.id).unwrap_or_else(|| EplanTarget {
            function: symbol.name.clone(),
            names: vec![symbol.name.clone()],
            letter: letter_code(&symbol.function).map(str::to_string),
            pins: symbol.connections.iter().map(|c| c.id.clone()).collect(),
            key: None,
            generic: true,
        })
    }

    /// Builds a user table from devices whose EPLAN symbol key is known:
    /// each one that maps by name or letters records its key. Devices that
    /// would only become generic boxes are returned for the user to map by
    /// hand. This is the user-run import of their own symbol library: the
    /// caller reads the names and pins from the user's export and hands them
    /// over; nothing of it is kept but the key and our symbol path.
    pub fn learn<'a>(
        &self,
        devices: impl IntoIterator<Item = &'a EplanSymbol>,
    ) -> (UserTable, Vec<EplanSymbol>) {
        let blank = SymbolMapper::new(self.pack);
        let mut table = self.user.clone();
        let mut open = Vec::new();
        for device in devices {
            let Some(key) = device.key else { continue };
            if table.get(key).is_some() {
                continue;
            }
            let r = blank.map(device);
            match r.symbol_id() {
                Some(id) => table.insert(key, id),
                None => open.push(device.clone()),
            }
        }
        (table, open)
    }

    /// Library symbols that have no row in the table. Empty when the table
    /// covers the pack; a test keeps it empty as the library grows.
    pub fn gaps(&self) -> Vec<String> {
        let table = Table::builtin();
        self.library()
            .iter()
            .filter(|s| table.entry_for(Pack::path_of(&s.id)).is_none())
            .map(|s| s.id.clone())
            .collect()
    }
}

/// The best candidate: highest score, then (with pins given) all pins mapped,
/// most pins matched, fewest spare connections, then table order.
fn pick<'t>(mut found: Vec<Candidate<'t>>, with_pins: bool) -> Option<(&'t Entry, (PinMap, bool))> {
    if found.is_empty() {
        return None;
    }
    let key = |c: &Candidate| {
        let (complete, matched, spare) = if with_pins {
            (
                c.pins.unmapped.is_empty(),
                c.pins.pairs.len() as i64,
                c.pins.spare.len() as i64,
            )
        } else {
            (true, 0, 0)
        };
        // Name score is compared in bands of 10 so a letter boost does not
        // outrank a better name; pins decide within a band.
        (c.score / 10, complete, matched, -spare, -(c.index as i64))
    };
    found.sort_by_key(|c| std::cmp::Reverse(key(c)));
    let best_key = key(&found[0]);
    let ambiguous = found.iter().skip(1).any(|c| {
        let k = key(c);
        (k.0, k.1, k.2, k.3) == (best_key.0, best_key.1, best_key.2, best_key.3)
    });
    let best = found.swap_remove(0);
    Some((best.entry, (best.pins, ambiguous)))
}

//! The symbol and function definitions a project export embeds, read for
//! two facts only: where a symbol's connection points are, and what the
//! symbol is called.
//!
//! A project export carries the library its pages were drawn with: `O20` is
//! a symbol, `O45` one of its drawn variants (orientations), `S48x823` a
//! connection point of that variant (index, offset from the insertion
//! point in millimetres, the directions it faces), and `O60` a function
//! definition (name, tag letters, pin designations). The graphics of those
//! symbols belong to the user's EPLAN installation and are neither read nor
//! kept: this module keeps names, pin positions and directions, in memory,
//! for the length of one import.
//!
//! How a placement finds its symbol is the observed model of the crane
//! configurator's own project checker: function definition (`A1262` on the
//! placement, `A242` on the symbol), library id (`A1261` / `A247`) and the
//! key `A1381 A1382 A1383`, looser step by step, then the drawn variant
//! (`A1263` / `A822`).

use std::collections::{BTreeMap, HashMap};

use super::text::MlText;

/// Directions a connection point faces, as the file writes them.
pub const UP: u8 = 1;
pub const RIGHT: u8 = 2;
pub const DOWN: u8 = 4;
pub const LEFT: u8 = 8;

/// A connection point of one variant of a library symbol.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LibPort {
    /// Index in the symbol; the placement's pin designations follow it.
    pub index: u32,
    /// Offset from the insertion point, millimetres, y up.
    pub x: f64,
    pub y: f64,
    /// Bit set of [`UP`], [`RIGHT`], [`DOWN`], [`LEFT`].
    pub dirs: u8,
}

/// A symbol of the embedded library.
#[derive(Debug, Clone, PartialEq)]
pub struct LibSymbol {
    /// `A242`.
    pub function_def: i64,
    /// `A247`: which of the file's symbol libraries.
    pub library_id: i64,
    /// `A1381`, `A1382`, `A1383`.
    pub key: (i64, i64, i64),
    /// `A241`, the symbol's short code in its library.
    pub code: String,
    /// `P16011`.
    pub name: MlText,
    /// Variant (`A822` of the `O45`) to its connection points.
    pub variants: BTreeMap<u32, Vec<LibPort>>,
}

/// A function definition (`O60`).
#[derive(Debug, Clone, PartialEq)]
pub struct LibFunction {
    pub key: (i64, i64, i64),
    /// `A1041`.
    pub name: MlText,
    /// The tag letters of the first function row (`A1044`).
    pub letters: String,
    /// The pin designations, row after row (`A1045`).
    pub pins: Vec<String>,
}

/// How exactly a placement was matched to a library symbol. Lower is more
/// certain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Match {
    /// Function definition, library id and the whole key agree.
    Exact,
    /// The third key number differs.
    SameSymbol,
    /// Only the first key number agrees (another variant of the family).
    SameFamily,
    /// Function definition and first key number, any library.
    OtherLibrary,
    /// Function definition only.
    FunctionOnly,
}

/// The embedded library.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PxfLibrary {
    pub(super) symbols: Vec<LibSymbol>,
    by_function_def: HashMap<i64, Vec<usize>>,
    pub(super) functions: Vec<LibFunction>,
    by_key: HashMap<(i64, i64, i64), usize>,
}

impl PxfLibrary {
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    pub fn symbol_count(&self) -> usize {
        self.symbols.len()
    }

    pub fn function_count(&self) -> usize {
        self.functions.len()
    }

    pub(super) fn add_symbol(&mut self, s: LibSymbol) {
        self.by_function_def
            .entry(s.function_def)
            .or_default()
            .push(self.symbols.len());
        self.symbols.push(s);
    }

    pub(super) fn add_function(&mut self, f: LibFunction) {
        self.by_key.entry(f.key).or_insert(self.functions.len());
        self.functions.push(f);
    }

    /// The function definition behind a placement's key.
    pub fn function(&self, key: (i64, i64, i64)) -> Option<&LibFunction> {
        self.by_key.get(&key).map(|&i| &self.functions[i])
    }

    /// The symbol a placement is drawn with, and how well it matched.
    /// `pins` is how many pin designations the placement has: of several
    /// equal candidates the one with that many connection points wins.
    pub fn symbol(
        &self,
        function_def: i64,
        library_id: i64,
        key: (i64, i64, i64),
        variant: i64,
        pins: usize,
    ) -> Option<(&LibSymbol, Match)> {
        let candidates = self.by_function_def.get(&function_def)?;
        type Step<'a> = (Match, &'a dyn Fn(&LibSymbol) -> bool);
        let steps: [Step; 5] = [
            (Match::Exact, &|s| {
                s.library_id == library_id && s.key == key
            }),
            (Match::SameSymbol, &|s| {
                s.library_id == library_id && s.key.0 == key.0 && s.key.1 == key.1
            }),
            (Match::SameFamily, &|s| {
                s.library_id == library_id && s.key.0 == key.0
            }),
            (Match::OtherLibrary, &|s| s.key.0 == key.0),
            (Match::FunctionOnly, &|_| true),
        ];
        let variant = u32::try_from(variant).ok()?;
        for (quality, ok) in steps {
            let found: Vec<&LibSymbol> = candidates
                .iter()
                .map(|&i| &self.symbols[i])
                .filter(|s| ok(s) && s.variants.get(&variant).is_some_and(|p| !p.is_empty()))
                .collect();
            if found.is_empty() {
                continue;
            }
            let pick = if pins > 0 {
                found
                    .iter()
                    .find(|s| s.variants[&variant].len() == pins)
                    .copied()
            } else {
                None
            };
            return Some((pick.unwrap_or(found[0]), quality));
        }
        None
    }
}

/// `x/y` as the file writes a point.
pub(super) fn parse_point(s: &str) -> Option<(f64, f64)> {
    let (x, y) = s.split_once('/')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

/// Splits a `;`-separated designation list, keeping empty entries so the
/// index of every pin stays.
pub(super) fn split_pins(s: &str) -> Vec<String> {
    if s.is_empty() {
        return Vec::new();
    }
    s.split(';').map(|p| p.trim().to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym(fd: i64, lib: i64, key: (i64, i64, i64), n: usize) -> LibSymbol {
        let ports = (0..n as u32)
            .map(|i| LibPort {
                index: i,
                x: 0.0,
                y: f64::from(i),
                dirs: UP,
            })
            .collect();
        LibSymbol {
            function_def: fd,
            library_id: lib,
            key,
            code: "T".into(),
            name: MlText::default(),
            variants: BTreeMap::from([(0, ports)]),
        }
    }

    #[test]
    fn matching_loosens_step_by_step() {
        let mut l = PxfLibrary::default();
        l.add_symbol(sym(0, 1, (201, 1, 3), 2));
        l.add_symbol(sym(0, 2, (300, 1, 1), 4));
        let (s, q) = l.symbol(0, 1, (201, 1, 3), 0, 2).unwrap();
        assert_eq!((s.key, q), ((201, 1, 3), Match::Exact));
        let (_, q) = l.symbol(0, 1, (201, 1, 9), 0, 0).unwrap();
        assert_eq!(q, Match::SameSymbol);
        let (_, q) = l.symbol(0, 1, (201, 4, 9), 0, 0).unwrap();
        assert_eq!(q, Match::SameFamily);
        let (s, q) = l.symbol(0, 7, (300, 9, 9), 0, 0).unwrap();
        assert_eq!((s.key, q), ((300, 1, 1), Match::OtherLibrary));
        let (_, q) = l.symbol(0, 7, (999, 9, 9), 0, 0).unwrap();
        assert_eq!(q, Match::FunctionOnly);
        assert!(l.symbol(5, 1, (201, 1, 3), 0, 0).is_none());
        assert!(
            l.symbol(0, 1, (201, 1, 3), 3, 0).is_none(),
            "variant 3 does not exist"
        );
    }

    #[test]
    fn the_pin_count_picks_between_equal_candidates() {
        let mut l = PxfLibrary::default();
        l.add_symbol(sym(0, 1, (201, 1, 3), 2));
        l.add_symbol(sym(0, 1, (201, 1, 3), 6));
        let (s, _) = l.symbol(0, 1, (201, 1, 3), 0, 6).unwrap();
        assert_eq!(s.variants[&0].len(), 6);
    }

    #[test]
    fn designation_lists_keep_their_empty_entries() {
        assert_eq!(split_pins("1;2"), ["1", "2"]);
        assert_eq!(split_pins(";"), ["", ""]);
        assert!(split_pins("").is_empty());
        assert_eq!(parse_point("-4/0"), Some((-4.0, 0.0)));
    }
}

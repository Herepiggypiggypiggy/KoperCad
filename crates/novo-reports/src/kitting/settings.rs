//! The stock the shop buys and holds: bar and reel lengths, saw kerf, pack
//! sizes, and what is on hand. Stored in the project (JSON text in a project
//! property), so a project carries its own stock. Every number here is a
//! planning value the person edits; none is read from a catalogue.

use novo_core::{Project, Value};
use serde::{Deserialize, Serialize};

/// Project property that holds changed kitting settings.
pub const SETTINGS_KEY: &str = "kitting.settings";

/// What the shop buys and holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KitSettings {
    /// DIN rail bar lengths on sale, mm.
    pub rail_stock_mm: Vec<f64>,
    /// Duct bar lengths on sale, mm.
    pub duct_stock_mm: Vec<f64>,
    /// Length the saw takes per cut, mm.
    pub kerf_mm: f64,
    /// A leftover bar piece at least this long is kept, mm.
    pub min_offcut_mm: f64,
    /// Wire reel length, m.
    pub reel_m: f64,
    /// Reel lengths that differ by cross-section: `(mm², metres)`.
    pub reel_overrides: Vec<(f64, f64)>,
    /// Share of every reel held back for re-cuts and mistakes, percent.
    pub wire_allowance_percent: f64,
    /// Ferrules per box.
    pub pack_ferrule: u32,
    /// Lugs and other end sleeves per bag.
    pub pack_lug: u32,
    /// Screws per box.
    pub pack_screw: u32,
    /// What is on hand, one line each (see [`OnHand::parse`]).
    pub on_hand: String,
    /// Order numbers for stock items, `Item name = order number`, so a stock
    /// line merges with the same article in the bill of materials.
    pub articles: String,
}

impl Default for KitSettings {
    fn default() -> Self {
        KitSettings {
            rail_stock_mm: vec![2000.0],
            duct_stock_mm: vec![2000.0],
            kerf_mm: 2.0,
            min_offcut_mm: 100.0,
            reel_m: 100.0,
            reel_overrides: Vec::new(),
            wire_allowance_percent: 5.0,
            pack_ferrule: 100,
            pack_lug: 25,
            pack_screw: 100,
            on_hand: String::new(),
            articles: String::new(),
        }
    }
}

impl KitSettings {
    /// The settings stored in `project`, else the defaults.
    pub fn of(project: &Project) -> KitSettings {
        match project.info().properties.get(SETTINGS_KEY) {
            Some(Value::Text(text)) => serde_json::from_str::<KitSettings>(text)
                .ok()
                .filter(KitSettings::is_valid)
                .unwrap_or_default(),
            _ => KitSettings::default(),
        }
    }

    /// Stores the settings; the defaults remove the property.
    pub fn store(tx: &mut novo_core::Tx<'_>, settings: &KitSettings) -> novo_core::Result<()> {
        let text = (*settings != KitSettings::default())
            .then(|| serde_json::to_string(settings).ok())
            .flatten();
        tx.set_info(|info| match text {
            Some(text) => {
                info.properties
                    .insert(SETTINGS_KEY.to_owned(), Value::Text(text));
            }
            None => {
                info.properties.remove(SETTINGS_KEY);
            }
        })
    }

    /// Lengths positive, allowance in range, packs at least one.
    pub fn is_valid(&self) -> bool {
        let pos = |v: &f64| v.is_finite() && *v > 0.0;
        !self.rail_stock_mm.is_empty()
            && self.rail_stock_mm.iter().all(pos)
            && !self.duct_stock_mm.is_empty()
            && self.duct_stock_mm.iter().all(pos)
            && self.kerf_mm.is_finite()
            && self.kerf_mm >= 0.0
            && self.min_offcut_mm.is_finite()
            && self.min_offcut_mm >= 0.0
            && pos(&self.reel_m)
            && self.reel_overrides.iter().all(|(s, m)| pos(s) && pos(m))
            && self.wire_allowance_percent.is_finite()
            && (0.0..=50.0).contains(&self.wire_allowance_percent)
            && self.pack_ferrule >= 1
            && self.pack_lug >= 1
            && self.pack_screw >= 1
    }

    /// Reel length for a cross-section, metres.
    pub fn reel_m_for(&self, mm2: Option<f64>) -> f64 {
        mm2.and_then(|s| {
            self.reel_overrides
                .iter()
                .find(|(size, _)| (size - s).abs() < 1e-9)
                .map(|(_, m)| *m)
        })
        .unwrap_or(self.reel_m)
    }

    /// The order number given for a stock item name, if any.
    pub fn article_of(&self, name: &str) -> Option<String> {
        self.articles.lines().find_map(|line| {
            let (n, order) = line.split_once('=')?;
            (n.trim().eq_ignore_ascii_case(name) && !order.trim().is_empty())
                .then(|| order.trim().to_owned())
        })
    }
}

/// The stock on hand, read from the settings' text.
///
/// One line each, `#` starts a comment:
///
/// ```text
/// rail 1200 x2            DIN rail, 1200 mm, two pieces
/// duct 50x60 1500         duct 50 x 60, one 1500 mm piece
/// wire 1.5 BK 37.5 m      a part reel of 37.5 m, 1.5 mm², colour BK
/// have A1.5-10 x40        any other item by name or order number, 40 pieces
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OnHand {
    /// DIN rail pieces, mm (a count expands to one entry each).
    pub rails: Vec<f64>,
    /// Duct pieces: `((width, height), length)`, mm.
    pub ducts: Vec<((f64, f64), f64)>,
    /// Part reels: `(mm², colour lower case, mm)`.
    pub wires: Vec<(f64, String, f64)>,
    /// Other items: `(name lower case, count)`.
    pub items: Vec<(String, u32)>,
}

fn number(text: &str) -> Option<f64> {
    text.trim()
        .replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v > 0.0)
}

/// `x3` or `X3` or `*3`: the count.
fn count(text: &str) -> Option<u32> {
    let t = text.trim();
    let t = t.strip_prefix(['x', 'X', '*', '×'])?;
    t.trim()
        .parse::<u32>()
        .ok()
        .filter(|n| (1..=100_000).contains(n))
}

impl OnHand {
    /// Reads the text; the second value lists lines that could not be read
    /// (`line 3: ...`). Lines that do not read are skipped.
    pub fn parse(text: &str) -> (OnHand, Vec<String>) {
        let mut out = OnHand::default();
        let mut problems = Vec::new();
        for (n, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let bad = |why: &str| format!("Stock on hand, line {}: {why}", n + 1);
            let (kind, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
            let words: Vec<&str> = rest.split_whitespace().collect();
            match kind.to_lowercase().as_str() {
                "rail" | "duct" => {
                    let is_duct = kind.eq_ignore_ascii_case("duct");
                    let (section, rest) = if is_duct {
                        let Some((first, rest)) = words.split_first() else {
                            problems.push(bad("write `duct 50x60 1500 x2`"));
                            continue;
                        };
                        let dims: Vec<f64> =
                            first.to_lowercase().split('x').filter_map(number).collect();
                        if dims.len() != 2 {
                            problems.push(bad("the duct size is written like 50x60"));
                            continue;
                        }
                        (Some((dims[0], dims[1])), rest)
                    } else {
                        (None, &words[..])
                    };
                    let Some(length) = rest.first().and_then(|w| number(w)) else {
                        problems.push(bad("the length in mm is missing"));
                        continue;
                    };
                    let n = match rest.get(1) {
                        None => 1,
                        Some(w) => match count(w) {
                            Some(n) => n,
                            None => {
                                problems.push(bad("the count is written like x2"));
                                continue;
                            }
                        },
                    };
                    for _ in 0..n {
                        match section {
                            Some(s) => out.ducts.push((s, length)),
                            None => out.rails.push(length),
                        }
                    }
                }
                "wire" => {
                    let mut w = words.clone();
                    if w.last().is_some_and(|x| x.eq_ignore_ascii_case("m")) {
                        w.pop();
                    }
                    let metres = w
                        .last()
                        .filter(|_| w.len() >= 2)
                        .and_then(|x| number(x.trim_end_matches(['m', 'M'])));
                    let size = w.first().and_then(|x| number(x));
                    let (Some(size), Some(metres)) = (size, metres) else {
                        problems.push(bad("write `wire 1.5 BK 37.5 m`"));
                        continue;
                    };
                    let colour = if w.len() >= 3 { w[1] } else { "" };
                    out.wires
                        .push((size, colour.to_lowercase(), metres * 1000.0));
                }
                "have" => {
                    let mut name = rest.trim().to_owned();
                    let mut n = 1;
                    if let Some((head, tail)) = name.rsplit_once(char::is_whitespace)
                        && let Some(c) = count(tail)
                    {
                        n = c;
                        name = head.trim().to_owned();
                    }
                    if name.is_empty() {
                        problems.push(bad("write `have A1.5-10 x40`"));
                        continue;
                    }
                    out.items.push((name.to_lowercase(), n));
                }
                _ => problems.push(bad("a line starts with rail, duct, wire or have")),
            }
        }
        (out, problems)
    }

    /// How many of an item are on hand: names compared without case.
    pub fn item(&self, names: &[&str]) -> u32 {
        self.items
            .iter()
            .filter(|(n, _)| {
                names
                    .iter()
                    .any(|w| !w.is_empty() && w.eq_ignore_ascii_case(n))
            })
            .map(|(_, c)| *c)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_on_hand_text_reads_every_kind_of_line() {
        let (h, problems) = OnHand::parse(
            "# shelf\nrail 1200 x2\nrail 800\nduct 50x60 1500 x3 # left over\nwire 1.5 BK 37.5 m\nwire 2,5 BU 12\nhave A1.5-10 x40\nhave Phoenix 3044076\n",
        );
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(h.rails, vec![1200.0, 1200.0, 800.0]);
        assert_eq!(h.ducts.len(), 3);
        assert_eq!(h.ducts[0], ((50.0, 60.0), 1500.0));
        assert_eq!(h.wires[0], (1.5, "bk".to_owned(), 37500.0));
        assert_eq!(h.wires[1], (2.5, "bu".to_owned(), 12000.0));
        assert_eq!(h.item(&["a1.5-10"]), 40);
        assert_eq!(h.item(&["PHOENIX 3044076"]), 1);
        assert_eq!(h.item(&["nothing", ""]), 0);
    }

    #[test]
    fn a_line_that_does_not_read_is_reported_with_its_number() {
        let (h, problems) = OnHand::parse("rail\nduct 50 1500\nbolt 4\nwire x\nrail 500 y\n");
        assert!(h.rails.is_empty() && h.ducts.is_empty());
        assert_eq!(problems.len(), 5);
        assert!(problems[0].contains("line 1"));
        assert!(problems[2].contains("line 3"));
    }

    #[test]
    fn settings_validate_and_find_their_articles() {
        assert!(KitSettings::default().is_valid());
        let mut s = KitSettings {
            articles: "DIN rail 2000 mm = NS35-7.5\n".into(),
            ..KitSettings::default()
        };
        assert_eq!(
            s.article_of("din rail 2000 mm").as_deref(),
            Some("NS35-7.5")
        );
        assert_eq!(s.article_of("duct"), None);
        s.rail_stock_mm.clear();
        assert!(!s.is_valid());
        let s = KitSettings {
            wire_allowance_percent: 80.0,
            ..KitSettings::default()
        };
        assert!(!s.is_valid());
        let s = KitSettings {
            reel_overrides: vec![(1.5, 50.0)],
            ..KitSettings::default()
        };
        assert_eq!(s.reel_m_for(Some(1.5)), 50.0);
        assert_eq!(s.reel_m_for(Some(2.5)), 100.0);
        assert_eq!(s.reel_m_for(None), 100.0);
    }
}

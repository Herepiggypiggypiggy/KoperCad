//! The standard's tables as data, with a visible version and provenance.
//!
//! Ampacity, ambient-temperature and grouping factors are not constants of
//! the program: they are a [`TableSet`] that every calculation takes by
//! reference, so the table behind a number can be named in the report and
//! replaced by the user's own copy of the standard.
//!
//! # Where the numbers come from
//!
//! [`TableSet::builtin`] holds *planning values* for copper, PVC and XLPE,
//! 1.5 to 120 mm², reference methods B1, B2 and C, in the layout of IEC
//! 60364-5-52 Tables B.52.2 to B.52.5 and the factor Tables B.52.14 and
//! B.52.17. They were entered without access to the standard. Public
//! secondary sources (installation guides, maker catalogues) agree with them
//! for the cells marked in [`CellStatus`], but they disagree with each other
//! on column labels for some cells and none of them is the standard. So:
//!
//! - every cell is [`CellStatus::Unverified`] until the user loads their
//!   licensed copy ([`TableSet::with_csv`]), after which those cells are
//!   [`CellStatus::Licensed`];
//! - a report built on a set with unverified cells says so
//!   ([`TableSet::disclosure`]);
//! - a combination with no value (for example XLPE method B2 with three
//!   loaded conductors) returns `None`, never a guess.
//!
//! # CSV format for the user's own table
//!
//! One record per line, `#` starts a comment, fields separated by commas:
//!
//! ```text
//! # edition: IEC 60364-5-52:2009
//! ampacity,PVC,B1,2,1.5,15.5
//! ambient,PVC,40,0.87
//! grouping,3,0.70
//! ```
//!
//! `ampacity,<PVC|XLPE>,<B1|B2|C>,<2|3>,<section mm²>,<amperes>`;
//! `ambient,<PVC|XLPE>,<°C>,<factor>`; `grouping,<circuits>,<factor>`.
//! Ampacity must rise with the section and factors must not rise with
//! temperature or circuit count, or the file is refused with the line
//! number. The file is read from the user's profile; nothing is shipped.

use std::collections::{HashMap, HashSet};

use super::Insulation;
use super::cable::{Loaded, Method};

/// Whether a table value was checked against the standard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellStatus {
    /// Entered from secondary sources; not checked against the standard.
    Unverified,
    /// Loaded by the user from their own copy of the standard.
    Licensed,
}

type AmpKey = (Insulation, Method, Loaded);

/// A versioned set of installation tables.
#[derive(Debug, Clone)]
pub struct TableSet {
    /// Short name shown in reports, e.g. `IEC 60364-5-52 (built-in)`.
    pub name: String,
    /// Edition the values claim to follow.
    pub edition: String,
    ampacity: HashMap<AmpKey, Vec<(f64, f64)>>,
    ambient: HashMap<Insulation, Vec<(f64, f64)>>,
    grouping: Vec<(u32, f64)>,
    licensed_ampacity: HashSet<AmpKey>,
    licensed_ambient: HashSet<Insulation>,
    licensed_grouping: bool,
}

const SECTIONS: [f64; 12] = [
    1.5, 2.5, 4.0, 6.0, 10.0, 16.0, 25.0, 35.0, 50.0, 70.0, 95.0, 120.0,
];

fn column(values: [f64; 12]) -> Vec<(f64, f64)> {
    SECTIONS.iter().copied().zip(values).collect()
}

impl TableSet {
    /// The built-in planning values (see the module documentation).
    pub fn builtin() -> TableSet {
        use Insulation::{Pvc, Xlpe};
        use Loaded::{Three, Two};
        use Method::{B1, B2, C};
        let mut ampacity = HashMap::new();
        let mut put = |i, m, l, v: [f64; 12]| {
            ampacity.insert((i, m, l), column(v));
        };
        // PVC, two loaded conductors.
        put(
            Pvc,
            B1,
            Two,
            [
                15.5, 21.0, 28.0, 36.0, 50.0, 68.0, 89.0, 110.0, 134.0, 171.0, 207.0, 239.0,
            ],
        );
        put(
            Pvc,
            B2,
            Two,
            [
                15.0, 20.0, 27.0, 34.0, 46.0, 62.0, 80.0, 99.0, 118.0, 149.0, 179.0, 206.0,
            ],
        );
        put(
            Pvc,
            C,
            Two,
            [
                19.5, 27.0, 36.0, 46.0, 63.0, 85.0, 112.0, 138.0, 168.0, 213.0, 258.0, 299.0,
            ],
        );
        // PVC, three loaded conductors.
        put(
            Pvc,
            B1,
            Three,
            [
                13.5, 18.0, 24.0, 31.0, 42.0, 56.0, 73.0, 89.0, 108.0, 136.0, 164.0, 188.0,
            ],
        );
        put(
            Pvc,
            B2,
            Three,
            [
                13.0, 17.5, 23.0, 29.0, 39.0, 52.0, 68.0, 83.0, 99.0, 125.0, 150.0, 172.0,
            ],
        );
        put(
            Pvc,
            C,
            Three,
            [
                17.5, 24.0, 32.0, 41.0, 57.0, 76.0, 96.0, 119.0, 144.0, 184.0, 223.0, 259.0,
            ],
        );
        // XLPE, two loaded conductors.
        put(
            Xlpe,
            B1,
            Two,
            [
                19.0, 26.0, 35.0, 45.0, 61.0, 81.0, 106.0, 131.0, 158.0, 200.0, 241.0, 278.0,
            ],
        );
        put(
            Xlpe,
            B2,
            Two,
            [
                18.5, 25.0, 34.0, 43.0, 60.0, 80.0, 101.0, 126.0, 153.0, 196.0, 238.0, 276.0,
            ],
        );
        put(
            Xlpe,
            C,
            Two,
            [
                24.0, 33.0, 45.0, 58.0, 80.0, 107.0, 138.0, 171.0, 209.0, 269.0, 328.0, 382.0,
            ],
        );
        // XLPE, three loaded conductors. Method B2 is left out: the
        // sources found disagree on the column.
        put(
            Xlpe,
            B1,
            Three,
            [
                17.0, 23.0, 31.0, 40.0, 54.0, 73.0, 95.0, 117.0, 141.0, 179.0, 216.0, 249.0,
            ],
        );
        put(
            Xlpe,
            C,
            Three,
            [
                22.0, 30.0, 40.0, 52.0, 71.0, 96.0, 119.0, 147.0, 179.0, 229.0, 278.0, 322.0,
            ],
        );

        let ambient = HashMap::from([
            (
                Pvc,
                vec![
                    (10.0, 1.22),
                    (15.0, 1.17),
                    (20.0, 1.12),
                    (25.0, 1.06),
                    (30.0, 1.0),
                    (35.0, 0.94),
                    (40.0, 0.87),
                    (45.0, 0.79),
                    (50.0, 0.71),
                    (55.0, 0.61),
                    (60.0, 0.50),
                ],
            ),
            (
                Xlpe,
                vec![
                    (10.0, 1.15),
                    (15.0, 1.12),
                    (20.0, 1.08),
                    (25.0, 1.04),
                    (30.0, 1.0),
                    (35.0, 0.96),
                    (40.0, 0.91),
                    (45.0, 0.87),
                    (50.0, 0.82),
                    (55.0, 0.76),
                    (60.0, 0.71),
                    (65.0, 0.65),
                    (70.0, 0.58),
                    (75.0, 0.50),
                    (80.0, 0.41),
                ],
            ),
        ]);
        let grouping = vec![
            (1, 1.0),
            (2, 0.80),
            (3, 0.70),
            (4, 0.65),
            (5, 0.60),
            (6, 0.57),
            (7, 0.54),
            (8, 0.52),
            (9, 0.50),
            (12, 0.45),
            (16, 0.41),
            (20, 0.38),
        ];
        TableSet {
            name: "IEC 60364-5-52 (built-in planning values)".into(),
            edition: "layout of IEC 60364-5-52:2009, values unverified".into(),
            ampacity,
            ambient,
            grouping,
            licensed_ampacity: HashSet::new(),
            licensed_ambient: HashSet::new(),
            licensed_grouping: false,
        }
    }

    /// The sections the ampacity tables list, mm².
    pub fn sections(&self) -> &'static [f64] {
        &SECTIONS
    }

    /// `Iz0` in amperes at 30 °C, one circuit, or `None` when the section
    /// or the combination is not in the table.
    pub fn ampacity(
        &self,
        section_mm2: f64,
        insulation: Insulation,
        method: Method,
        loaded: Loaded,
    ) -> Option<f64> {
        self.ampacity
            .get(&(insulation, method, loaded))?
            .iter()
            .find(|(s, _)| (s - section_mm2).abs() < 1e-6)
            .map(|(_, a)| *a)
    }

    /// Whether the table has a column for the combination at all.
    pub fn has_column(&self, insulation: Insulation, method: Method, loaded: Loaded) -> bool {
        self.ampacity.contains_key(&(insulation, method, loaded))
    }

    /// Ambient temperature factor `k1`, interpolated; `None` outside the
    /// listed range.
    pub fn ambient_factor(&self, celsius: f64, insulation: Insulation) -> Option<f64> {
        let table = self.ambient.get(&insulation)?;
        let (first, last) = (*table.first()?, *table.last()?);
        if celsius.is_nan() || celsius < first.0 || celsius > last.0 {
            return None;
        }
        let upper = table.iter().position(|(t, _)| *t >= celsius)?;
        if upper == 0 {
            return Some(first.1);
        }
        let ((t0, k0), (t1, k1)) = (table[upper - 1], table[upper]);
        Some(k0 + (k1 - k0) * (celsius - t0) / (t1 - t0))
    }

    /// Grouping factor `k2` for `circuits` bunched circuits; between listed
    /// counts the next higher count's factor applies (safe side), beyond the last
    /// listed count the last factor applies.
    pub fn grouping_factor(&self, circuits: u32) -> f64 {
        let n = circuits.max(1);
        self.grouping
            .iter()
            .find(|(listed, _)| *listed >= n)
            .or(self.grouping.last())
            .map_or(1.0, |(_, k)| *k)
    }

    /// Status of one ampacity column.
    pub fn status(&self, insulation: Insulation, method: Method, loaded: Loaded) -> CellStatus {
        if self
            .licensed_ampacity
            .contains(&(insulation, method, loaded))
        {
            CellStatus::Licensed
        } else {
            CellStatus::Unverified
        }
    }

    /// True when nothing in the set is unverified: every ampacity column,
    /// both ambient tables and the grouping table came from a licensed copy.
    pub fn fully_licensed(&self) -> bool {
        self.ampacity
            .keys()
            .all(|k| self.licensed_ampacity.contains(k))
            && self
                .ambient
                .keys()
                .all(|k| self.licensed_ambient.contains(k))
            && self.licensed_grouping
    }

    /// The sentence a report prints under its numbers.
    pub fn disclosure(&self) -> String {
        if self.fully_licensed() {
            format!(
                "Tables: {} ({}), loaded from your licensed copy.",
                self.name, self.edition
            )
        } else {
            let unverified = self
                .ampacity
                .keys()
                .filter(|k| !self.licensed_ampacity.contains(k))
                .count();
            format!(
                "Tables: {} ({}). {} ampacity columns are planning values not checked against the standard; load your licensed copy (`kopercad-cli calc --tables`; a Project settings entry is planned) before quoting these numbers.",
                self.name, self.edition, unverified
            )
        }
    }

    /// A copy with the records of `csv` laid over this set. Every column,
    /// ambient table or grouping table that the file touches is replaced
    /// completely and marked [`CellStatus::Licensed`]. `name` is shown in
    /// reports (for example `IEC 60364-5-52:2009, licensed copy`).
    pub fn with_csv(&self, name: &str, csv: &str) -> Result<TableSet, String> {
        let mut out = self.clone();
        out.name = name.to_owned();
        let mut edition = None;
        let mut amp: HashMap<AmpKey, Vec<(f64, f64)>> = HashMap::new();
        let mut amb: HashMap<Insulation, Vec<(f64, f64)>> = HashMap::new();
        let mut grp: Vec<(u32, f64)> = Vec::new();
        for (index, raw) in csv.lines().enumerate() {
            let line_no = index + 1;
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(comment) = line.strip_prefix('#') {
                if let Some(e) = comment.trim().strip_prefix("edition:") {
                    edition = Some(e.trim().to_owned());
                }
                continue;
            }
            let fields: Vec<&str> = line.split(',').map(str::trim).collect();
            let err = |what: &str| format!("line {line_no}: {what}");
            let num = |s: &str| {
                s.parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite() && *v > 0.0)
                    .ok_or_else(|| err(&format!("'{s}' is not a positive number")))
            };
            match fields.as_slice() {
                ["ampacity", ins, method, loaded, section, value] => {
                    let ins =
                        parse_insulation(ins).ok_or_else(|| err("insulation is PVC or XLPE"))?;
                    let method =
                        Method::parse(method).ok_or_else(|| err("method is B1, B2 or C"))?;
                    let loaded = match *loaded {
                        "2" => Loaded::Two,
                        "3" => Loaded::Three,
                        _ => return Err(err("loaded conductors is 2 or 3")),
                    };
                    amp.entry((ins, method, loaded))
                        .or_default()
                        .push((num(section)?, num(value)?));
                }
                ["ambient", ins, temp, factor] => {
                    let ins =
                        parse_insulation(ins).ok_or_else(|| err("insulation is PVC or XLPE"))?;
                    let t: f64 = temp
                        .parse()
                        .map_err(|_| err("temperature is not a number"))?;
                    amb.entry(ins).or_default().push((t, num(factor)?));
                }
                ["grouping", n, factor] => {
                    let n: u32 = n
                        .parse()
                        .map_err(|_| err("circuit count is not a whole number"))?;
                    grp.push((n, num(factor)?));
                }
                _ => return Err(err("record not recognised")),
            }
        }
        for (key, mut cells) in amp {
            cells.sort_by(|a, b| a.0.total_cmp(&b.0));
            if cells
                .windows(2)
                .any(|w| w[1].1 <= w[0].1 || w[1].0 <= w[0].0)
            {
                return Err(format!(
                    "{:?} {:?} {:?}: ampacity must rise with the section",
                    key.0, key.1, key.2
                ));
            }
            out.ampacity.insert(key, cells);
            out.licensed_ampacity.insert(key);
        }
        for (ins, mut cells) in amb {
            cells.sort_by(|a, b| a.0.total_cmp(&b.0));
            if cells.windows(2).any(|w| w[1].1 > w[0].1) {
                return Err(format!(
                    "{ins:?}: the ambient factor must not rise with temperature"
                ));
            }
            out.ambient.insert(ins, cells);
            out.licensed_ambient.insert(ins);
        }
        if !grp.is_empty() {
            grp.sort_by_key(|g| g.0);
            if grp.windows(2).any(|w| w[1].1 > w[0].1) {
                return Err("the grouping factor must not rise with the circuit count".into());
            }
            out.grouping = grp;
            out.licensed_grouping = true;
        }
        if let Some(e) = edition {
            out.edition = e;
        }
        Ok(out)
    }
}

fn parse_insulation(text: &str) -> Option<Insulation> {
    match text.to_ascii_uppercase().as_str() {
        "PVC" => Some(Insulation::Pvc),
        "XLPE" | "EPR" => Some(Insulation::Xlpe),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouping_between_listed_counts_uses_the_next_higher_count() {
        let t = TableSet::builtin();
        assert!(t.grouping_factor(10) <= t.grouping_factor(9));
        assert!(t.grouping_factor(7) <= t.grouping_factor(6));
    }

    #[test]
    fn builtin_is_unverified_and_says_so() {
        let t = TableSet::builtin();
        assert!(!t.fully_licensed());
        assert_eq!(
            t.status(Insulation::Pvc, Method::C, Loaded::Three),
            CellStatus::Unverified
        );
        assert!(t.disclosure().contains("not checked against the standard"));
    }

    #[test]
    fn missing_cells_are_none_not_guesses() {
        let t = TableSet::builtin();
        assert!(!t.has_column(Insulation::Xlpe, Method::B2, Loaded::Three));
        assert_eq!(
            t.ampacity(16.0, Insulation::Xlpe, Method::B2, Loaded::Three),
            None
        );
        assert_eq!(
            t.ampacity(3.0, Insulation::Pvc, Method::C, Loaded::Three),
            None
        );
        assert_eq!(
            t.ampacity(16.0, Insulation::Xlpe, Method::B2, Loaded::Two),
            Some(80.0)
        );
    }

    #[test]
    fn csv_overrides_and_marks_licensed() {
        let t = TableSet::builtin()
            .with_csv(
                "IEC 60364-5-52:2009, my copy",
                "# edition: IEC 60364-5-52:2009\n\
                 ampacity,PVC,C,3,1.5,18\n\
                 ampacity,PVC,C,3,2.5,25\n\
                 ambient,PVC,30,1\n\
                 ambient,PVC,40,0.8\n\
                 grouping,1,1\ngrouping,2,0.8\n",
            )
            .unwrap();
        assert_eq!(
            t.ampacity(1.5, Insulation::Pvc, Method::C, Loaded::Three),
            Some(18.0)
        );
        // The replaced column has only the rows of the file.
        assert_eq!(
            t.ampacity(4.0, Insulation::Pvc, Method::C, Loaded::Three),
            None
        );
        assert_eq!(
            t.status(Insulation::Pvc, Method::C, Loaded::Three),
            CellStatus::Licensed
        );
        assert_eq!(
            t.status(Insulation::Pvc, Method::C, Loaded::Two),
            CellStatus::Unverified
        );
        assert_eq!(t.ambient_factor(35.0, Insulation::Pvc), Some(0.9));
        assert_eq!(t.grouping_factor(5), 0.8);
        assert_eq!(t.edition, "IEC 60364-5-52:2009");
        assert!(!t.fully_licensed());
    }

    #[test]
    fn csv_errors_name_the_line() {
        let b = TableSet::builtin();
        let e = b
            .with_csv("x", "ampacity,PVC,C,3,1.5,18\nampacity,PVC,Q,3,2.5,25\n")
            .unwrap_err();
        assert!(e.starts_with("line 2:"), "{e}");
        let e = b
            .with_csv("x", "ampacity,PVC,C,3,1.5,30\nampacity,PVC,C,3,2.5,25\n")
            .unwrap_err();
        assert!(e.contains("rise with the section"), "{e}");
        let e = b
            .with_csv("x", "ambient,PVC,30,0.9\nambient,PVC,40,1.0\n")
            .unwrap_err();
        assert!(e.contains("must not rise"), "{e}");
        assert!(b.with_csv("x", "ampacity,PVC,C,3,abc,5\n").is_err());
        assert!(b.with_csv("x", "nonsense\n").is_err());
    }
}

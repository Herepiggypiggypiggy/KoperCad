//! The record of a chosen enclosure (`handoff/cabinet-start/DESIGN.md`).
//!
//! One record per enclosure, saved as properties of its `Structure` (keys
//! start with `encl.`), read by every feature that needs the enclosure
//! (`handoff/cabinet-start/CHAIN.md`). Every number carries where it came
//! from, so heat results and reports can say how far to trust it.
//! Pure: no file or UI access. Millimetres.

use novo_core::{Properties, Value};

/// Where a number came from, best first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    /// Read from the maker's STEP file.
    Step,
    /// Marked by the user as measured on the real cabinet.
    Measured,
    /// From a catalogue row.
    Catalogue,
    /// Typed by the user.
    Typed,
    /// A program default.
    Default,
}

impl Source {
    pub fn key(self) -> &'static str {
        match self {
            Source::Step => "step",
            Source::Measured => "measured",
            Source::Catalogue => "catalogue",
            Source::Typed => "typed",
            Source::Default => "default",
        }
    }

    pub fn from_key(key: &str) -> Option<Source> {
        Some(match key {
            "step" => Source::Step,
            "measured" => Source::Measured,
            "catalogue" => Source::Catalogue,
            "typed" => Source::Typed,
            "default" => Source::Default,
            _ => return None,
        })
    }

    /// The one-line state shown next to a value.
    pub fn words(self) -> &'static str {
        match self {
            Source::Step => "from your STEP",
            Source::Measured => "measured by you",
            Source::Catalogue => "catalogue",
            Source::Typed => "typed by you",
            Source::Default => "default",
        }
    }
}

/// A number and its source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Field {
    pub value: f64,
    pub source: Source,
}

impl Field {
    pub fn new(value: f64, source: Source) -> Field {
        Field { value, source }
    }
}

/// Shown while any value is a catalogue or default value.
pub const ESTIMATE_NOTICE: &str =
    "Estimate from catalogue data, not a measurement of your cabinet.";

/// Two values for the same thing differ by more than 2 mm or 1 %, whichever
/// is larger.
pub fn disagree(a: f64, b: f64) -> bool {
    (a - b).abs() > 2.0_f64.max(0.01 * a.abs().max(b.abs()))
}

/// The STEP value is used over the catalogue value; `Some` carries both
/// numbers when they disagree beyond the tolerance, so the panel can show
/// them and say which was used (the first, the STEP).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reconciled {
    pub field: Field,
    pub conflict: Option<(f64, f64)>,
}

/// Picks between a STEP value and a catalogue value (either may be missing).
pub fn reconcile(step: Option<f64>, catalogue: Option<f64>) -> Option<Reconciled> {
    match (step, catalogue) {
        (Some(s), Some(c)) => Some(Reconciled {
            field: Field::new(s, Source::Step),
            conflict: disagree(s, c).then_some((s, c)),
        }),
        (Some(s), None) => Some(Reconciled {
            field: Field::new(s, Source::Step),
            conflict: None,
        }),
        (None, Some(c)) => Some(Reconciled {
            field: Field::new(c, Source::Catalogue),
            conflict: None,
        }),
        (None, None) => None,
    }
}

/// A field of the record that a feature needed and did not find.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Missing(pub &'static str);

impl std::fmt::Display for Missing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "The enclosure has no {}.", self.0)
    }
}

/// The chosen enclosure. `None` fields are unknown: readers report them
/// ([`Missing`]) and never fall back to a silent default.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnclosureSpec {
    /// Maker article number, for example `8455.600`.
    pub article: String,
    /// Series, for example `TS 8`.
    pub series: String,
    /// Outside size.
    pub width: Option<Field>,
    pub height: Option<Field>,
    pub depth: Option<Field>,
    /// Sheet thickness.
    pub wall: Option<Field>,
    /// Back wall to mounting plate surface.
    pub plate_setback: Option<Field>,
    /// How far things on the inside of the door stand out.
    pub door_inner_depth: Option<Field>,
    /// File name and SHA-256 of the user's STEP; the file is never stored.
    pub step_name: String,
    pub step_sha256: String,
    /// Rails and ducts fitted to the mounting plate at delivery, in plate
    /// millimetres (origin top left, as the plate drawing).
    pub parts: Vec<FactoryPart>,
    /// Openings for cables in the walls or a gland plate.
    pub entries: Vec<CableEntry>,
}

/// A DIN rail or duct fitted at delivery.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FactoryPart {
    pub kind: PartKind,
    /// Start of the run on the plate.
    pub x: f64,
    pub y: f64,
    pub length: f64,
    /// Runs down from the start instead of to the right.
    pub vertical: bool,
    /// Cross-section of a duct (0 for a rail or while unknown).
    pub width: f64,
    pub height: f64,
    pub source: Source,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartKind {
    Rail,
    Duct,
}

/// A cable entry: where cables pass through the enclosure. `x`, `y` is the
/// point on the plate edge nearest to it (plate millimetres), `wall` the
/// side of the enclosure it is in.
#[derive(Debug, Clone, PartialEq)]
pub struct CableEntry {
    pub name: String,
    pub wall: String,
    pub x: f64,
    pub y: f64,
    /// Diameter of the biggest round cable it takes; 0 while unknown.
    pub diameter: f64,
    pub source: Source,
}

impl FactoryPart {
    fn encode(&self) -> String {
        format!(
            "{} {} {} {} {} {} {} {}",
            if self.kind == PartKind::Duct {
                "duct"
            } else {
                "rail"
            },
            self.x,
            self.y,
            self.length,
            if self.vertical { "v" } else { "h" },
            self.width,
            self.height,
            self.source.key()
        )
    }

    fn decode(text: &str) -> Option<FactoryPart> {
        let w: Vec<&str> = text.split_whitespace().collect();
        if w.len() != 8 {
            return None;
        }
        let n = |i: usize| w[i].parse::<f64>().ok().filter(|v| v.is_finite());
        let length = n(3).filter(|v| *v > 0.0)?;
        Some(FactoryPart {
            kind: match w[0] {
                "duct" => PartKind::Duct,
                "rail" => PartKind::Rail,
                _ => return None,
            },
            x: n(1)?,
            y: n(2)?,
            length,
            vertical: match w[4] {
                "v" => true,
                "h" => false,
                _ => return None,
            },
            width: n(5)?.max(0.0),
            height: n(6)?.max(0.0),
            source: Source::from_key(w[7]).unwrap_or(Source::Typed),
        })
    }
}

impl CableEntry {
    fn encode(&self) -> String {
        format!(
            "{} {} {} {} {} {}",
            self.wall.replace(' ', "_"),
            self.x,
            self.y,
            self.diameter,
            self.source.key(),
            self.name
        )
    }

    fn decode(text: &str) -> Option<CableEntry> {
        let mut it = text.splitn(6, ' ');
        let wall = it.next()?.to_owned();
        let mut n = || it.next()?.parse::<f64>().ok().filter(|v| v.is_finite());
        let (x, y, diameter) = (n()?, n()?, n()?);
        let source = Source::from_key(it.next()?).unwrap_or(Source::Typed);
        Some(CableEntry {
            name: it.next().unwrap_or("").to_owned(),
            wall,
            x,
            y,
            diameter: diameter.max(0.0),
            source,
        })
    }
}

/// Inside size, from outside size and wall.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interior {
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    /// The weakest source of the numbers used.
    pub weakest: Source,
}

const PREFIX: &str = "encl.";
const NUMBERS: [&str; 6] = [
    "width_mm",
    "height_mm",
    "depth_mm",
    "wall_mm",
    "plate_setback_mm",
    "door_inner_depth_mm",
];

impl EnclosureSpec {
    fn numbers(&self) -> [&Option<Field>; 6] {
        [
            &self.width,
            &self.height,
            &self.depth,
            &self.wall,
            &self.plate_setback,
            &self.door_inner_depth,
        ]
    }

    /// Inside size; names the first missing field instead of guessing.
    pub fn interior(&self) -> Result<Interior, Missing> {
        let w = self.width.ok_or(Missing("outside width"))?;
        let h = self.height.ok_or(Missing("outside height"))?;
        let d = self.depth.ok_or(Missing("outside depth"))?;
        let t = self.wall.ok_or(Missing("wall thickness"))?;
        Ok(Interior {
            width: (w.value - 2.0 * t.value).max(0.0),
            height: (h.value - 2.0 * t.value).max(0.0),
            depth: (d.value - 2.0 * t.value).max(0.0),
            weakest: [w, h, d, t]
                .iter()
                .map(|f| f.source)
                .max()
                .unwrap_or(Source::Default),
        })
    }

    /// The first thing a chosen enclosure still lacks for the door and the
    /// 3D shell: outside size, wall and plate setback. Door inner depth is
    /// optional (nothing on the door). `None` when nothing was chosen.
    pub fn shell_missing(&self) -> Option<Missing> {
        if self.is_empty() {
            return None;
        }
        if let Err(m) = self.interior() {
            return Some(m);
        }
        self.plate_setback
            .is_none()
            .then_some(Missing("plate setback"))
    }

    /// The weakest source among the known numbers; `None` without numbers.
    pub fn weakest(&self) -> Option<Source> {
        self.numbers()
            .iter()
            .filter_map(|f| f.map(|f| f.source))
            .max()
    }

    /// The line to show while any value is a catalogue or default value.
    pub fn notice(&self) -> Option<&'static str> {
        self.numbers()
            .iter()
            .filter_map(|f| f.map(|f| f.source))
            .any(|s| matches!(s, Source::Catalogue | Source::Default))
            .then_some(ESTIMATE_NOTICE)
    }

    /// True when nothing was chosen.
    pub fn is_empty(&self) -> bool {
        self.article.is_empty()
            && self.numbers().iter().all(|f| f.is_none())
            && self.parts.is_empty()
            && self.entries.is_empty()
    }

    /// Writes the record into properties (replacing an earlier one).
    pub fn write(&self, props: &mut Properties) {
        props.retain(|k, _| !k.starts_with(PREFIX));
        let mut put = |k: &str, v: Value| {
            props.insert(format!("{PREFIX}{k}"), v);
        };
        if !self.article.is_empty() {
            put("article", Value::Text(self.article.clone()));
        }
        if !self.series.is_empty() {
            put("series", Value::Text(self.series.clone()));
        }
        if !self.step_name.is_empty() {
            put("step_name", Value::Text(self.step_name.clone()));
            put("step_sha256", Value::Text(self.step_sha256.clone()));
        }
        for (key, field) in NUMBERS.iter().zip(self.numbers()) {
            if let Some(f) = field {
                put(key, Value::Float(f.value));
                put(&format!("{key}.src"), Value::Text(f.source.key().into()));
            }
        }
        for (i, part) in self.parts.iter().enumerate() {
            put(&format!("part.{i}"), Value::Text(part.encode()));
        }
        for (i, entry) in self.entries.iter().enumerate() {
            put(&format!("entry.{i}"), Value::Text(entry.encode()));
        }
    }

    /// Reads the record; a number without a readable source is `Typed`.
    pub fn read(props: &Properties) -> EnclosureSpec {
        let text = |k: &str| match props.get(&format!("{PREFIX}{k}")) {
            Some(Value::Text(t)) => t.clone(),
            _ => String::new(),
        };
        let number = |k: &str| {
            let v = match props.get(&format!("{PREFIX}{k}"))? {
                Value::Float(v) => *v,
                Value::Int(v) => *v as f64,
                _ => return None,
            };
            let source = match props.get(&format!("{PREFIX}{k}.src")) {
                Some(Value::Text(s)) => Source::from_key(s).unwrap_or(Source::Typed),
                _ => Source::Typed,
            };
            Some(Field::new(v, source))
        };
        EnclosureSpec {
            article: text("article"),
            series: text("series"),
            width: number(NUMBERS[0]),
            height: number(NUMBERS[1]),
            depth: number(NUMBERS[2]),
            wall: number(NUMBERS[3]),
            plate_setback: number(NUMBERS[4]),
            door_inner_depth: number(NUMBERS[5]),
            step_name: text("step_name"),
            step_sha256: text("step_sha256"),
            parts: indexed(props, "part.", FactoryPart::decode),
            entries: indexed(props, "entry.", CableEntry::decode),
        }
    }
}

/// The entries `<PREFIX><stem>0`, `<stem>1` ... in order; an entry that does
/// not read is skipped (never guessed).
fn indexed<T>(props: &Properties, stem: &str, decode: fn(&str) -> Option<T>) -> Vec<T> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(Value::Text(t)) = props.get(&format!("{PREFIX}{stem}{i}")) {
        out.extend(decode(t));
        i += 1;
    }
    out
}

/// One catalogue row. Every row says where it came from and which terms
/// were checked; a row without both is refused by [`from_csv`].
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogueRow {
    pub article: String,
    pub series: String,
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    pub wall: f64,
    pub plate_article: String,
    pub source: String,
    pub terms: String,
}

impl CatalogueRow {
    /// The record this row gives, every number marked as catalogue.
    pub fn spec(&self) -> EnclosureSpec {
        let c = |v: f64| Some(Field::new(v, Source::Catalogue));
        EnclosureSpec {
            article: self.article.clone(),
            series: self.series.clone(),
            width: c(self.width),
            height: c(self.height),
            depth: c(self.depth),
            wall: c(self.wall),
            ..EnclosureSpec::default()
        }
    }
}

const HEADER: &str =
    "article;series;width_mm;height_mm;depth_mm;wall_mm;plate_article;source;terms";

/// Reads catalogue rows from CSV (separator `;`, decimal point or comma).
/// Returns good rows and one message per bad line (1-based). A row with an
/// empty source or terms column is bad.
pub fn from_csv(text: &str) -> (Vec<CatalogueRow>, Vec<(usize, String)>) {
    let mut rows = Vec::new();
    let mut errors = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let raw = raw.trim().trim_start_matches('\u{feff}');
        if raw.is_empty() || raw.eq_ignore_ascii_case(HEADER) {
            continue;
        }
        let cols: Vec<&str> = raw.split(';').map(str::trim).collect();
        if cols.len() != 9 {
            errors.push((i + 1, format!("expected 9 columns, found {}", cols.len())));
            continue;
        }
        let num = |s: &str| s.replace(',', ".").parse::<f64>().ok().filter(|v| *v > 0.0);
        let (Some(width), Some(height), Some(depth), Some(wall)) =
            (num(cols[2]), num(cols[3]), num(cols[4]), num(cols[5]))
        else {
            errors.push((i + 1, "size and wall must be positive numbers".into()));
            continue;
        };
        if cols[0].is_empty() || cols[7].is_empty() || cols[8].is_empty() {
            errors.push((i + 1, "article, source and terms must be filled".into()));
            continue;
        }
        rows.push(CatalogueRow {
            article: cols[0].into(),
            series: cols[1].into(),
            width,
            height,
            depth,
            wall,
            plate_article: cols[6].into(),
            source: cols[7].into(),
            terms: cols[8].into(),
        });
    }
    (rows, errors)
}

#[cfg(test)]
mod factory_tests {
    use super::*;

    fn spec() -> EnclosureSpec {
        EnclosureSpec {
            article: "8455.600".into(),
            parts: vec![
                FactoryPart {
                    kind: PartKind::Duct,
                    x: 100.0,
                    y: 50.0,
                    length: 400.0,
                    vertical: true,
                    width: 40.0,
                    height: 60.0,
                    source: Source::Catalogue,
                },
                FactoryPart {
                    kind: PartKind::Rail,
                    x: 20.0,
                    y: 200.0,
                    length: 300.0,
                    vertical: false,
                    width: 0.0,
                    height: 0.0,
                    source: Source::Step,
                },
            ],
            entries: vec![CableEntry {
                name: "gland plate, bottom".into(),
                wall: "bottom".into(),
                x: 150.0,
                y: 600.0,
                diameter: 32.0,
                source: Source::Catalogue,
            }],
            ..EnclosureSpec::default()
        }
    }

    #[test]
    fn parts_and_entries_round_trip() {
        let a = spec();
        let mut props = Properties::default();
        a.write(&mut props);
        assert_eq!(EnclosureSpec::read(&props), a);
    }

    #[test]
    fn writing_again_replaces_the_parts() {
        let mut props = Properties::default();
        spec().write(&mut props);
        let mut b = spec();
        b.parts.truncate(1);
        b.entries.clear();
        b.write(&mut props);
        let read = EnclosureSpec::read(&props);
        assert_eq!(read.parts.len(), 1);
        assert!(read.entries.is_empty());
    }

    #[test]
    fn a_part_that_does_not_read_is_skipped() {
        assert!(FactoryPart::decode("duct 1 2 0 v 40 60 catalogue").is_none());
        assert!(FactoryPart::decode("pipe 1 2 5 v 40 60 catalogue").is_none());
        assert!(CableEntry::decode("bottom x 1 2 step").is_none());
    }

    #[test]
    fn only_parts_still_counts_as_a_spec() {
        let mut s = spec();
        s.article.clear();
        assert!(!s.is_empty());
    }
}

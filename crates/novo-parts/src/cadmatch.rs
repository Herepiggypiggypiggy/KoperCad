//! Which part is this CAD file for?
//!
//! A maker's download is often not named after the article: Schneider calls
//! its files `MCADPP0003371_3D-simplified.stp`, and the model inside calls
//! itself `LC1D09-18-DC`, a family of contactors. Filing such a file under
//! whatever part happens to be selected (what the Parts window used to do)
//! puts the wrong 3D model on a part without a word. This module reads what
//! the file says about itself and decides, with its reasons:
//!
//! - **names**: the file name, the names inside a STEP file (`FILE_NAME`
//!   and `PRODUCT` entities), the comment lines of a DXF, the members of a
//!   ZIP. The order number in any of them is the strongest evidence; a
//!   family range such as `LC1D09-18` covers `LC1D09BD`, `LC1D12BD` and
//!   `LC1D18BD`; a type number counts too.
//! - **document id**: files of one download set share a prefix
//!   (`MCADPP0003371_3D...`, `MCADPP0003371_2D...`). Once one is matched
//!   to a part ([`CadLinks`]), the others follow without naming it.
//! - **size**: the size of the model against the part's dimensions, with
//!   the same sorted-axes comparison for a model and a data sheet. A size
//!   taken from all points of a STEP file is only a lower bound for the
//!   solid (it can be far too large), so it can speak against a part but
//!   never for it ([`SizeMeasure::exact`]).
//!
//! Nothing here reads a file from disk or touches the network: the caller
//! hands over bytes and gets a [`Candidate`] with a verdict and reasons in
//! plain words. A size or a family match never turns into a certainty:
//! [`Verdict::Verified`] needs the order number itself.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::catalog::{Catalog, CatalogPart, PartKey};
use crate::import::read_zip;
use crate::library::FileKind;
use crate::recipes::recipe_for;
use crate::search::normalize_code;

/// Where a name came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameFrom {
    FileName,
    ZipMember,
    StepHeader,
    StepProduct,
    Dxf,
}

impl NameFrom {
    fn words(self) -> &'static str {
        match self {
            NameFrom::FileName => "the file name",
            NameFrom::ZipMember => "a file inside the ZIP",
            NameFrom::StepHeader => "the STEP header",
            NameFrom::StepProduct => "the model inside the STEP file",
            NameFrom::Dxf => "a comment in the DXF file",
        }
    }
}

/// One name a file gives itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CadName {
    pub text: String,
    pub from: NameFrom,
}

/// A size read from the model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SizeMeasure {
    pub size_mm: [f64; 3],
    /// True when measured from the solid's geometry. False for the box
    /// around every point of a STEP file, which holds construction geometry
    /// and can be several times too large: a lower bound only.
    pub exact: bool,
}

/// What a CAD file says about itself.
#[derive(Debug, Clone, PartialEq)]
pub struct CadIdentity {
    pub file_name: String,
    pub kind: FileKind,
    pub names: Vec<CadName>,
    /// The maker named in a STEP header, if any.
    pub maker: Option<String>,
    /// The download-set id at the start of the file name (`MCADPP0003371`).
    pub doc_id: Option<String>,
    pub size: Option<SizeMeasure>,
    /// Whether the file is, or holds, a STEP or DXF model (a ZIP may hold
    /// only a catalog).
    pub holds_model: bool,
}

/// Most names kept from one file: a STEP assembly can hold thousands.
const MAX_NAMES: usize = 24;

impl CadIdentity {
    /// What the file name alone says.
    pub fn from_file_name(file_name: &str) -> Self {
        let mut id = CadIdentity {
            file_name: file_name.to_owned(),
            kind: FileKind::of_file_name(file_name),
            names: Vec::new(),
            maker: None,
            doc_id: doc_id_of(file_name),
            size: None,
            holds_model: false,
        };
        id.holds_model = matches!(id.kind, FileKind::Step | FileKind::Dxf | FileKind::Edz);
        id.push(stem_of(file_name), NameFrom::FileName);
        id
    }

    /// Reads the identity of a file from its bytes. Never fails: a file that
    /// cannot be read just says less about itself.
    pub fn read(file_name: &str, bytes: &[u8]) -> Self {
        let mut id = Self::from_file_name(file_name);
        match id.kind {
            FileKind::Step => id.add_step(&String::from_utf8_lossy(bytes)),
            FileKind::Dxf => id.add_dxf(&String::from_utf8_lossy(bytes)),
            FileKind::Zip | FileKind::Edz => {
                if let Ok(z) = read_zip(bytes) {
                    id.holds_model |= z.step.is_some() || !z.dxf.is_empty();
                    for member in &z.names {
                        let base = member.rsplit(['/', '\\']).next().unwrap_or(member);
                        id.push(stem_of(base), NameFrom::ZipMember);
                        if id.doc_id.is_none() {
                            id.doc_id = doc_id_of(base);
                        }
                    }
                    if let Some((_, text)) = &z.step {
                        id.add_step(text);
                    }
                    for (_, text) in &z.dxf {
                        id.add_dxf(text);
                    }
                }
            }
            _ => {}
        }
        id
    }

    #[must_use]
    pub fn with_size(mut self, size: SizeMeasure) -> Self {
        self.size = Some(size);
        self
    }

    fn push(&mut self, text: &str, from: NameFrom) {
        let text = text.trim();
        if text.is_empty()
            || is_generic(text)
            || self.names.len() >= MAX_NAMES
            || self.names.iter().any(|n| n.text.eq_ignore_ascii_case(text))
        {
            return;
        }
        self.names.push(CadName {
            text: text.to_owned(),
            from,
        });
    }

    fn add_step(&mut self, text: &str) {
        // Only the first FILE_NAME counts: FILE_NAME(name, time, (authors), (organisations), ...)
        if let Some(args) = step_entities(text, "FILE_NAME", false).into_iter().next() {
            if let Some(Arg::Str(name)) = args.first() {
                self.push(name, NameFrom::StepHeader);
            }
            if let Some(Arg::List(orgs)) = args.get(3) {
                for o in orgs {
                    if let Arg::Str(s) = o
                        && !s.trim().is_empty()
                        && self.maker.is_none()
                    {
                        self.maker = Some(s.trim().to_owned());
                    }
                }
            }
        }
        // PRODUCT(id, name, description, frames)
        for args in step_entities(text, "PRODUCT", true) {
            for a in args.iter().take(3) {
                if let Arg::Str(s) = a {
                    self.push(s, NameFrom::StepProduct);
                }
            }
        }
    }

    fn add_dxf(&mut self, text: &str) {
        // Group code 999 is a comment; CAD exports leave the source path there.
        let mut lines = text.lines().take(120);
        while let Some(code) = lines.next() {
            if code.trim() == "999"
                && let Some(value) = lines.next()
            {
                let value = value.trim();
                for part in value.split(['\\', '/']) {
                    if doc_id_of(part).is_some() {
                        if self.doc_id.is_none() {
                            self.doc_id = doc_id_of(part);
                        }
                        break;
                    }
                }
                self.push(
                    value.rsplit(['\\', '/']).next().unwrap_or(value),
                    NameFrom::Dxf,
                );
            }
        }
    }

    /// Whether any name could tie the file to one particular article.
    fn identifies(&self) -> bool {
        self.names.iter().any(|n| identifying(&n.text))
    }
}

fn stem_of(file_name: &str) -> &str {
    match file_name.rfind('.') {
        Some(i) if i > 0 => &file_name[..i],
        _ => file_name,
    }
}

/// Names that say nothing: CAD tools fill them in when the author did not.
fn is_generic(text: &str) -> bool {
    let c = normalize_code(text);
    c.len() < 4
        || matches!(
            c.as_str(),
            "notspecified" | "unspecified" | "none" | "unknown" | "part" | "assembly" | "model"
        )
        || (c.starts_with("part") && c[4..].chars().all(|ch| ch.is_ascii_digit()))
        || (c.starts_with("assembly") && c[8..].chars().all(|ch| ch.is_ascii_digit()))
        || (c.starts_with("untitled"))
}

/// A name with both letters and digits, or six digits: something an article
/// number can be found in, as opposed to `drawing` or `model v2`.
fn identifying(text: &str) -> bool {
    let c = normalize_code(text);
    let digits = c.chars().filter(char::is_ascii_digit).count();
    let letters = c.chars().filter(|ch| ch.is_alphabetic()).count();
    c.len() >= 5 && ((digits >= 1 && letters >= 1) || digits >= 6)
}

/// The id at the start of a file name that a whole download set shares:
/// the first `_` segment, eight characters or more with letters and five
/// digits or more (`MCADPP0003371`). A plain order number is not one.
pub fn doc_id_of(file_name: &str) -> Option<String> {
    let stem = stem_of(file_name);
    let seg = stem.split(['_', ' ']).next()?;
    let alnum = seg.chars().all(|c| c.is_ascii_alphanumeric());
    let letters = seg.chars().filter(char::is_ascii_alphabetic).count();
    let digits = seg.chars().filter(char::is_ascii_digit).count();
    (alnum && seg.len() >= 8 && letters >= 2 && digits >= 5).then(|| seg.to_ascii_uppercase())
}

// ---------------------------------------------------------------------
// A small reader for the parameter lists of STEP entities.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Arg {
    Str(String),
    List(Vec<Arg>),
    Other,
}

/// Parameter lists of every `NAME(...)` entity instance in STEP text, at
/// most [`MAX_ENTITIES`] of them. An entity instance (`#12=PRODUCT(`) must
/// follow `=`, so `PRODUCT(` is not found inside `PRODUCT_DEFINITION(`; the
/// header entities (`FILE_NAME(`) have no `#12=`.
fn step_entities(text: &str, name: &str, instance: bool) -> Vec<Vec<Arg>> {
    const MAX_ENTITIES: usize = 64;
    let needle = format!("{name}(");
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    // Entity names are upper case in practice; a lower-case file is read as is.
    for (at, _) in text.match_indices(&needle) {
        let mut b = at;
        while b > 0 && bytes[b - 1].is_ascii_whitespace() {
            b -= 1;
        }
        if instance && (b == 0 || bytes[b - 1] != b'=') {
            continue;
        }
        let mut i = at + needle.len();
        let end = (i + 4096).min(text.len());
        let slice = &text[..floor_char_boundary(text, end)];
        out.push(parse_list(slice, &mut i));
        if out.len() >= MAX_ENTITIES {
            break;
        }
    }
    out
}

fn floor_char_boundary(text: &str, mut i: usize) -> usize {
    while i > 0 && !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Parses until the `)` that closes the list; `i` starts just after `(`.
fn parse_list(text: &str, i: &mut usize) -> Vec<Arg> {
    let b = text.as_bytes();
    let mut args = Vec::new();
    while *i < b.len() {
        match b[*i] {
            b')' => {
                *i += 1;
                return args;
            }
            b'(' => {
                *i += 1;
                args.push(Arg::List(parse_list(text, i)));
            }
            b'\'' => {
                *i += 1;
                let mut s = Vec::new();
                while *i < b.len() {
                    if b[*i] == b'\'' {
                        if b.get(*i + 1) == Some(&b'\'') {
                            s.push(b'\'');
                            *i += 2;
                            continue;
                        }
                        *i += 1;
                        break;
                    }
                    s.push(b[*i]);
                    *i += 1;
                }
                args.push(Arg::Str(String::from_utf8_lossy(&s).into_owned()));
            }
            b',' | b' ' | b'\t' | b'\r' | b'\n' => *i += 1,
            _ => {
                // $, *, numbers, #refs, .ENUM.: one argument we do not use.
                while *i < b.len() && !matches!(b[*i], b',' | b')' | b'(' | b'\'') {
                    *i += 1;
                }
                args.push(Arg::Other);
            }
        }
    }
    args
}

// ---------------------------------------------------------------------
// Names against an article.
// ---------------------------------------------------------------------

/// `LC1D09-18-DC`: a model for the articles `LC1D09` to `LC1D18` with the
/// letters after them to be chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Family {
    stem: String,
    lo: u64,
    hi: u64,
    width: usize,
    /// What follows the range (`DC`, `3D_ASM`).
    rest: Vec<String>,
}

fn family_of(token: &str) -> Option<Family> {
    let segs: Vec<&str> = token.split('-').collect();
    if segs.len() < 2 {
        return None;
    }
    let first = segs[0];
    let digits_from = first
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_digit())
        .last()?
        .0;
    let (stem, lo_s) = first.split_at(digits_from);
    let stem = normalize_code(stem);
    if stem.len() < 3 {
        return None;
    }
    let second = segs[1];
    let d_end = second
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(second.len());
    let (hi_s, tail) = second.split_at(d_end);
    if hi_s.len() != lo_s.len() || tail.len() > 2 || !tail.chars().all(char::is_alphabetic) {
        return None;
    }
    let (lo, hi): (u64, u64) = (lo_s.parse().ok()?, hi_s.parse().ok()?);
    if hi <= lo || hi - lo > 200 {
        return None;
    }
    let mut rest: Vec<String> = Vec::new();
    if !tail.is_empty() {
        rest.push(tail.to_owned());
    }
    rest.extend(segs[2..].iter().map(|s| (*s).to_owned()));
    Some(Family {
        stem,
        lo,
        hi,
        width: lo_s.len(),
        rest,
    })
}

impl Family {
    fn covers(&self, code: &str) -> bool {
        let Some(after) = code.strip_prefix(&self.stem) else {
            return false;
        };
        let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
        digits.len() == self.width
            && digits
                .parse::<u64>()
                .is_ok_and(|n| (self.lo..=self.hi).contains(&n))
    }
}

/// Whether `hay` (normalized) holds `code` as a whole code, not inside a
/// longer run of the same kind (`120021201` does not hold `20021201`).
fn holds_code(hay: &str, code: &str) -> bool {
    if code.is_empty() {
        return false;
    }
    let first_digit = code.as_bytes()[0].is_ascii_digit();
    let last_digit = code.as_bytes()[code.len() - 1].is_ascii_digit();
    hay.match_indices(code).any(|(at, _)| {
        let before = hay[..at].chars().next_back();
        let after = hay[at + code.len()..].chars().next();
        before.is_none_or(|c| c.is_ascii_digit() != first_digit || !c.is_alphanumeric())
            && after.is_none_or(|c| c.is_ascii_digit() != last_digit || !c.is_alphanumeric())
    })
}

/// How a verdict reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verdict {
    /// Nothing ties the file to this part.
    Unmatched,
    /// Something ties it, but another fact speaks against it (the size).
    Check,
    /// A family, type number or download set ties it to the part.
    Likely,
    /// The file names the part's order number and nothing speaks against it.
    Verified,
}

impl Verdict {
    pub fn label(self) -> &'static str {
        match self {
            Verdict::Verified => "Checked",
            Verdict::Likely => "Probably this part",
            Verdict::Check => "Please check",
            Verdict::Unmatched => "No part found",
        }
    }
}

/// One part a file may belong to, with the reasons in plain words.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub key: PartKey,
    pub label: String,
    pub verdict: Verdict,
    pub score: i32,
    /// What speaks for it.
    pub evidence: Vec<String>,
    /// What speaks against it.
    pub warnings: Vec<String>,
    /// What could not be checked or is only worth knowing (the size could
    /// not be compared; the file was made by a CAD service). Not a reason
    /// to doubt the match.
    pub notes: Vec<String>,
}

const ORDER: i32 = 100;
const TYPE_NUMBER: i32 = 80;
const LINKED: i32 = 85;
const FAMILY: i32 = 70;

struct NameHit {
    score: i32,
    evidence: Vec<String>,
}

/// What does not depend on the part, worked out once per file: the
/// normalized names and the family ranges written in them. A catalog of tens
/// of thousands of parts is matched against one file without reading the
/// file's names again for each part.
struct Prepared {
    hays: Vec<String>,
    /// Per name, the words that are family ranges.
    families: Vec<Vec<(String, Family)>>,
}

fn prepare(id: &CadIdentity) -> Prepared {
    Prepared {
        hays: id.names.iter().map(|n| normalize_code(&n.text)).collect(),
        families: id
            .names
            .iter()
            .map(|n| {
                n.text
                    .split(|c: char| c.is_whitespace() || ",;:()/".contains(c))
                    .filter_map(|w| family_of(w).map(|f| (w.to_owned(), f)))
                    .collect()
            })
            .collect(),
    }
}

/// Names against a part; `score` 0 when nothing ties them.
fn name_hit(id: &CadIdentity, prep: &Prepared, part: &CatalogPart, links: &CadLinks) -> NameHit {
    let codes = recipe_for(&part.manufacturer).map_or_else(
        || vec![normalize_code(&part.order_number)],
        |r| r.file_codes(&part.order_number),
    );
    let type_code = normalize_code(&part.type_number);
    let part_code = normalize_code(&part.order_number);
    let mut best = NameHit {
        score: 0,
        evidence: Vec::new(),
    };
    let mut consider = |score: i32, text: String| {
        if score > best.score {
            best = NameHit {
                score,
                evidence: vec![text],
            };
        }
    };
    for (n, name) in id.names.iter().enumerate() {
        let hay = &prep.hays[n];
        if codes.iter().any(|c| c.len() >= 5 && holds_code(hay, c)) {
            consider(
                ORDER,
                format!(
                    "{} says \u{201c}{}\u{201d}, which holds the order number {}.",
                    cap(name.from.words()),
                    name.text,
                    part.order_number
                ),
            );
        } else if type_code.len() >= 4 && holds_code(hay, &type_code) {
            consider(
                TYPE_NUMBER,
                format!(
                    "{} says \u{201c}{}\u{201d}, which holds the type {}.",
                    cap(name.from.words()),
                    name.text,
                    part.type_number
                ),
            );
        }
        // Family ranges are written with dashes, so look at each word of the name.
        for (word, f) in &prep.families[n] {
            if f.covers(&part_code) {
                let mut text = format!(
                    "{} says \u{201c}{word}\u{201d}, a model for {}{:0w$} to {}{:0w$}; {} is in that range.",
                    cap(name.from.words()),
                    f.stem.to_uppercase(),
                    f.lo,
                    f.stem.to_uppercase(),
                    f.hi,
                    part.order_number,
                    w = f.width
                );
                let said = variant_words(f, part);
                if !said.is_empty() {
                    text.push_str(&format!(
                        " The part's description also says {}.",
                        said.join(", ")
                    ));
                }
                consider(FAMILY + if said.is_empty() { 0 } else { 5 }, text);
            }
        }
    }
    if let Some(doc) = &id.doc_id
        && links.keys_for(doc).contains(&part.key())
    {
        // Counts for more than a family, less than the order number.
        if LINKED > best.score {
            best = NameHit {
                score: LINKED,
                evidence: vec![format!(
                    "It belongs to the download set {doc}, which you already matched to {part}."
                )],
            };
        }
    }
    best
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| {
        f.to_uppercase().collect::<String>() + c.as_str()
    })
}

/// Words after a family range (`DC`) that the part's own text repeats.
fn variant_words(f: &Family, part: &CatalogPart) -> Vec<String> {
    let mut hay = format!(" {} {} ", part.description, part.keywords.join(" ")).to_lowercase();
    for v in part.data.values() {
        if let novo_core::Value::Text(t) = v {
            hay.push_str(&format!("{} ", t.to_lowercase()));
        }
    }
    let words: Vec<&str> = hay
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    f.rest
        .iter()
        .filter(|w| (2..=4).contains(&w.len()) && w.chars().all(char::is_alphabetic))
        .filter(|w| words.contains(&w.to_lowercase().as_str()))
        .cloned()
        .collect()
}

/// The size of a model against a part's dimensions, axes sorted so the
/// model's orientation does not matter.
/// Returns what speaks for the part, what speaks against it, and a note
/// when the size could not be compared.
fn size_view(
    size: &SizeMeasure,
    part: &CatalogPart,
) -> (Option<String>, Option<String>, Option<String>) {
    let Some(d) = part.dimensions else {
        return (
            None,
            None,
            Some("The part has no dimensions to compare the model's size with.".into()),
        );
    };
    let mut want = [d.width_mm, d.height_mm, d.depth_mm];
    let mut have = size.size_mm;
    want.sort_by(f64::total_cmp);
    have.sort_by(f64::total_cmp);
    let text = |s: [f64; 3]| format!("{:.0} \u{d7} {:.0} \u{d7} {:.0} mm", s[0], s[1], s[2]);
    if want.iter().any(|w| *w <= 0.0) {
        return (None, None, None);
    }
    if size.exact {
        let worst = (0..3)
            .map(|k| ((have[k] - want[k]) / want[k]).abs())
            .fold(0.0, f64::max);
        if worst <= 0.15 {
            (
                Some(format!(
                    "The model is {}; the part is {}: the size fits.",
                    text(have),
                    text(want)
                )),
                None,
                None,
            )
        } else if worst <= 0.30 {
            (
                None,
                Some(format!(
                    "The model is {} and the part {}: a bit different (simplified models often are).",
                    text(have),
                    text(want)
                )),
                None,
            )
        } else {
            (
                None,
                Some(format!(
                    "SIZE: the model is {} but the part is {}. This is probably another part.",
                    text(have),
                    text(want)
                )),
                None,
            )
        }
    } else {
        // The box around all points is at least the solid's box: a model
        // that is already too small along an axis cannot be this part.
        let too_small = (0..3).any(|k| have[k] < want[k] * 0.70);
        if too_small {
            (
                None,
                Some(format!(
                    "SIZE: all points of the model fit in {} but the part is {}. This is probably another part.",
                    text(have),
                    text(want)
                )),
                None,
            )
        } else {
            (
                None,
                None,
                Some(
                    "The size was not compared: the file's size could only be estimated from all its points."
                        .into(),
                ),
            )
        }
    }
}

/// Everything the file says about one part.
pub fn assess_part(
    id: &CadIdentity,
    part: &CatalogPart,
    catalog: Option<&Catalog>,
    links: &CadLinks,
) -> Candidate {
    let prep = prepare(id);
    let hit = name_hit(id, &prep, part, links);
    // The index of longer article numbers is worth building only for a part
    // the file is tied to, and only once.
    let index = (hit.score > 0)
        .then(|| catalog.map(LongerIndex::new))
        .flatten();
    assess_with(id, &prep, part, hit, index.as_ref())
}

fn assess_with(
    id: &CadIdentity,
    prep: &Prepared,
    part: &CatalogPart,
    hit: NameHit,
    index: Option<&LongerIndex>,
) -> Candidate {
    let mut c = Candidate {
        key: part.key(),
        label: part.to_string(),
        verdict: Verdict::Unmatched,
        score: hit.score,
        evidence: hit.evidence,
        warnings: Vec::new(),
        notes: Vec::new(),
    };
    if hit.score == 0 {
        return c;
    }
    if let Some(idx) = index
        && let Some(longer) = idx.named_by_longer(&prep.hays, part)
    {
        c.score = 0;
        c.evidence.clear();
        c.warnings.push(format!(
            "The file names {longer}, a longer article number that starts with {}; it is for that part.",
            part.order_number
        ));
        return c;
    }
    let mut against = false;
    if let Some(size) = &id.size {
        let (fits, warning, note) = size_view(size, part);
        if let Some(f) = fits {
            c.evidence.push(f);
        }
        if let Some(n) = warning {
            against |= n.starts_with("SIZE:");
            c.warnings.push(n.trim_start_matches("SIZE: ").to_owned());
        }
        c.notes.extend(note);
    }
    if let Some(m) = &id.maker {
        if maker_fits(m, &part.manufacturer) {
            c.evidence
                .push(format!("The file was made by {m}, the part's maker."));
        } else {
            c.notes.push(format!(
                "The file was made by {m}, not {} (this can be a CAD service, so it is only a note).",
                part.manufacturer
            ));
        }
    }
    c.verdict = if against {
        Verdict::Check
    } else if hit.score >= ORDER {
        Verdict::Verified
    } else {
        Verdict::Likely
    };
    c
}

fn maker_fits(file_maker: &str, maker: &str) -> bool {
    let (a, b) = (normalize_code(file_maker), normalize_code(maker));
    let aliases = recipe_for(maker).map_or(&[][..], |r| r.aliases);
    (a.len() >= 4 && b.len() >= 4 && (a.contains(&b) || b.contains(&a)))
        || aliases.iter().any(|al| {
            let n = normalize_code(al);
            n.len() >= 4 && a.contains(&n)
        })
}

/// The normalized order numbers of a catalog per maker, sorted, so the
/// numbers that start with a given one are found by bisection instead of by
/// reading the whole catalog for each part.
struct LongerIndex {
    by_maker: std::collections::HashMap<String, Vec<(String, String)>>,
}

impl LongerIndex {
    fn new(catalog: &Catalog) -> Self {
        let mut by_maker: std::collections::HashMap<String, Vec<(String, String)>> =
            std::collections::HashMap::new();
        for p in catalog.parts() {
            by_maker
                .entry(normalize_code(&p.manufacturer))
                .or_default()
                .push((normalize_code(&p.order_number), p.order_number.clone()));
        }
        for v in by_maker.values_mut() {
            v.sort();
        }
        LongerIndex { by_maker }
    }

    /// An order number of the same maker, longer than this part's and
    /// starting with it, that the file's names hold: `LC1D09BD` is not
    /// `LC1D09`.
    fn named_by_longer(&self, hays: &[String], part: &CatalogPart) -> Option<String> {
        let code = normalize_code(&part.order_number);
        let list = self.by_maker.get(&normalize_code(&part.manufacturer))?;
        let start = list.partition_point(|(n, _)| n.as_str() < code.as_str());
        list[start..]
            .iter()
            .take_while(|(n, _)| n.starts_with(&code))
            .filter(|(n, _)| n.len() > code.len())
            .find(|(n, _)| hays.iter().any(|h| holds_code(h, n)))
            .map(|(_, shown)| shown.clone())
    }
}

/// A catalog read once for a whole batch of files: which parts each code
/// belongs to, and the order numbers in sorted order. A file then finds the
/// parts it may be for by looking up the codes written in its names and the
/// range of a family, not by testing every part of the catalog; the full
/// judgement ([`name_hit`]) is then made on those parts only.
struct Index<'a> {
    parts: &'a [CatalogPart],
    /// Normalized order codes (as the maker's recipe writes them in file
    /// names) and type numbers, to the parts that have them.
    by_code: std::collections::HashMap<String, Vec<usize>>,
    longest_code: usize,
    /// Normalized order number and part, sorted: the family range is a
    /// stretch of this list.
    orders: Vec<(String, usize)>,
    by_key: std::collections::HashMap<PartKey, usize>,
    longer: LongerIndex,
}

/// Shortest code worth looking up (the judgement requires 5 for an order
/// code and 4 for a type number).
const MIN_CODE: usize = 4;
/// Names longer than this are looked at only up to here when searching for
/// codes.
const MAX_HAY: usize = 512;

impl<'a> Index<'a> {
    fn new(catalog: &'a Catalog) -> Self {
        let parts = catalog.parts();
        let mut by_code: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::new();
        let mut orders = Vec::with_capacity(parts.len());
        let mut by_key = std::collections::HashMap::with_capacity(parts.len());
        let mut longest_code = MIN_CODE;
        for (i, p) in parts.iter().enumerate() {
            let order = normalize_code(&p.order_number);
            let codes = recipe_for(&p.manufacturer)
                .map_or_else(|| vec![order.clone()], |r| r.file_codes(&p.order_number));
            let type_code = normalize_code(&p.type_number);
            for c in codes.into_iter().chain(Some(type_code)) {
                if c.chars().count() >= MIN_CODE {
                    longest_code = longest_code.max(c.chars().count());
                    let v = by_code.entry(c).or_default();
                    if v.last() != Some(&i) {
                        v.push(i);
                    }
                }
            }
            orders.push((order, i));
            by_key.insert(p.key(), i);
        }
        orders.sort();
        Index {
            parts,
            by_code,
            longest_code,
            orders,
            by_key,
            longer: LongerIndex::new(catalog),
        }
    }

    /// The parts a file may tie to: a superset of those [`name_hit`] scores
    /// above zero, in catalog order.
    fn candidates(
        &self,
        id: &CadIdentity,
        prep: &Prepared,
        links: &CadLinks,
    ) -> Vec<&'a CatalogPart> {
        let mut found = std::collections::BTreeSet::new();
        for hay in &prep.hays {
            let chars: Vec<(usize, char)> = hay.char_indices().take(MAX_HAY).collect();
            for start in 0..chars.len() {
                let from = chars[start].0;
                for end in (start + MIN_CODE)..=(start + self.longest_code).min(chars.len()) {
                    let to = chars.get(end).map_or(hay.len(), |c| c.0);
                    if let Some(v) = self.by_code.get(&hay[from..to]) {
                        found.extend(v.iter().copied());
                    }
                }
            }
        }
        for fam in prep.families.iter().flatten().map(|(_, f)| f) {
            let first = self
                .orders
                .partition_point(|(n, _)| n.as_str() < fam.stem.as_str());
            found.extend(
                self.orders[first..]
                    .iter()
                    .take_while(|(n, _)| n.starts_with(&fam.stem))
                    .filter(|(n, _)| fam.covers(n))
                    .map(|(_, i)| *i),
            );
        }
        if let Some(doc) = &id.doc_id {
            found.extend(
                links
                    .keys_for(doc)
                    .iter()
                    .filter_map(|k| self.by_key.get(k)),
            );
        }
        found.into_iter().map(|i| &self.parts[i]).collect()
    }
}

/// The parts a file may be for, best first. Parts with no tie to the file
/// are left out. At most `limit` are returned.
pub fn rank(id: &CadIdentity, catalog: &Catalog, links: &CadLinks, limit: usize) -> Vec<Candidate> {
    rank_in(id, &Index::new(catalog), links, limit)
}

fn rank_in(id: &CadIdentity, index: &Index, links: &CadLinks, limit: usize) -> Vec<Candidate> {
    let prep = prepare(id);
    let mut out: Vec<Candidate> = index
        .candidates(id, &prep, links)
        .into_iter()
        .map(|p| (p, name_hit(id, &prep, p, links)))
        .filter(|(_, h)| h.score > 0)
        .map(|(p, h)| assess_with(id, &prep, p, h, Some(&index.longer)))
        .filter(|c| c.score > 0)
        .collect();
    out.sort_by(|a, b| {
        b.verdict
            .cmp(&a.verdict)
            .then(b.score.cmp(&a.score))
            .then(a.label.cmp(&b.label))
    });
    out.truncate(limit);
    out
}

/// What to do with a file the user dropped on a chosen part.
#[derive(Debug, Clone, PartialEq)]
pub enum DropDecision {
    /// The file belongs to the part: import it, and say why.
    Import { notes: Vec<String> },
    /// The file says nothing that could be checked (no article number
    /// anywhere in it): import as before, and say that nothing was checked.
    ImportUnchecked { why: String },
    /// The file looks like it is for something else: ask first.
    Ask {
        reasons: Vec<String>,
        /// A better home for the file, when the catalog has one.
        better: Option<Candidate>,
    },
}

/// Decides for a file dropped on `part`.
pub fn decide_drop(
    id: &CadIdentity,
    part: &CatalogPart,
    catalog: &Catalog,
    links: &CadLinks,
) -> DropDecision {
    let c = assess_part(id, part, Some(catalog), links);
    match c.verdict {
        Verdict::Verified | Verdict::Likely => {
            let mut notes = c.evidence;
            notes.extend(c.warnings);
            notes.extend(c.notes);
            DropDecision::Import { notes }
        }
        Verdict::Check => {
            let mut reasons = c.warnings;
            reasons.extend(c.evidence);
            DropDecision::Ask {
                reasons,
                better: rank(id, catalog, links, 1)
                    .into_iter()
                    .find(|b| b.key != c.key && b.verdict >= Verdict::Likely),
            }
        }
        Verdict::Unmatched if !id.identifies() => DropDecision::ImportUnchecked {
            why: format!(
                "{} does not say which article it is for (no article number in its name or inside), so it was not checked.",
                id.file_name
            ),
        },
        Verdict::Unmatched => {
            let names: Vec<String> = id
                .names
                .iter()
                .filter(|n| identifying(&n.text))
                .take(3)
                .map(|n| format!("\u{201c}{}\u{201d}", n.text))
                .collect();
            DropDecision::Ask {
                reasons: vec![format!(
                    "The file is called {}; that does not name {part} or a range that includes it.",
                    names.join(" / ")
                )],
                better: rank(id, catalog, links, 1)
                    .into_iter()
                    .find(|b| b.key != c.key && b.verdict >= Verdict::Likely),
            }
        }
    }
}

/// One file of a batch with the parts it may belong to.
#[derive(Debug, Clone, PartialEq)]
pub struct FileMatch {
    pub identity: CadIdentity,
    pub candidates: Vec<Candidate>,
}

/// Matches a batch of files. A file that names nothing but arrived with one
/// that does (same document id) takes its candidates.
pub fn match_batch(
    ids: Vec<CadIdentity>,
    catalog: &Catalog,
    links: &CadLinks,
    limit: usize,
) -> Vec<FileMatch> {
    let index = Index::new(catalog);
    let mut out: Vec<FileMatch> = ids
        .into_iter()
        .map(|identity| FileMatch {
            candidates: rank_in(&identity, &index, links, limit),
            identity,
        })
        .collect();
    let mut by_doc: BTreeMap<String, (String, Vec<Candidate>)> = BTreeMap::new();
    for m in &out {
        if let Some(doc) = &m.identity.doc_id
            && !m.candidates.is_empty()
        {
            by_doc
                .entry(doc.clone())
                .or_insert_with(|| (m.identity.file_name.clone(), m.candidates.clone()));
        }
    }
    for m in &mut out {
        if !m.candidates.is_empty() {
            continue;
        }
        let Some(doc) = &m.identity.doc_id else {
            continue;
        };
        if let Some((from, cands)) = by_doc.get(doc) {
            m.candidates = cands
                .iter()
                .map(|c| Candidate {
                    verdict: Verdict::Likely,
                    score: LINKED,
                    evidence: vec![format!(
                        "It is in the same download set ({doc}) as {from}, which is for {}.",
                        c.label
                    )],
                    warnings: Vec::new(),
                    notes: Vec::new(),
                    ..c.clone()
                })
                .collect();
        }
    }
    out
}

// ---------------------------------------------------------------------
// Download sets learned from files the user confirmed.
// ---------------------------------------------------------------------

/// Which parts a download set (document id) was matched to. Kept in the
/// library folder beside the records, so the 2D drawing that comes next
/// finds the same parts without naming them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CadLinks {
    sets: BTreeMap<String, Vec<PartKey>>,
}

#[derive(Serialize, Deserialize, Default)]
struct LinksFile {
    format: u32,
    #[serde(default)]
    links: Vec<LinkRow>,
}

#[derive(Serialize, Deserialize)]
struct LinkRow {
    doc_id: String,
    manufacturer: String,
    order_number: String,
}

const LINKS_FILE: &str = "cad-links.json";

impl CadLinks {
    pub fn path_in(library_root: &Path) -> PathBuf {
        library_root.join(LINKS_FILE)
    }

    /// Reads the links of a library; a missing or unreadable file is no links.
    pub fn load(library_root: &Path) -> Self {
        let Ok(text) = fs::read_to_string(Self::path_in(library_root)) else {
            return CadLinks::default();
        };
        let Ok(file) = serde_json::from_str::<LinksFile>(&text) else {
            return CadLinks::default();
        };
        let mut links = CadLinks::default();
        for r in file.links {
            // A row that names no download set or no part links nothing.
            if [&r.doc_id, &r.manufacturer, &r.order_number]
                .iter()
                .any(|f| f.trim().is_empty())
            {
                continue;
            }
            links.add(&r.doc_id, &PartKey::new(&r.manufacturer, &r.order_number));
        }
        links
    }

    pub fn keys_for(&self, doc_id: &str) -> &[PartKey] {
        self.sets
            .get(&doc_id.to_ascii_uppercase())
            .map_or(&[], Vec::as_slice)
    }

    pub fn add(&mut self, doc_id: &str, key: &PartKey) {
        let keys = self.sets.entry(doc_id.to_ascii_uppercase()).or_default();
        if !keys.contains(key) {
            keys.push(key.clone());
        }
    }

    pub fn is_empty(&self) -> bool {
        self.sets.is_empty()
    }

    /// Writes beside the library records (temp file and rename).
    pub fn save(&self, library_root: &Path) -> io::Result<()> {
        let file = LinksFile {
            format: 1,
            links: self
                .sets
                .iter()
                .flat_map(|(doc, keys)| {
                    keys.iter().map(|k| LinkRow {
                        doc_id: doc.clone(),
                        manufacturer: k.manufacturer().to_owned(),
                        order_number: k.order_number().to_owned(),
                    })
                })
                .collect(),
        };
        fs::create_dir_all(library_root)?;
        let path = Self::path_in(library_root);
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(&file)?)?;
        fs::rename(&tmp, &path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Dimensions;

    fn part(maker: &str, order: &str, dims: Option<[f64; 3]>) -> CatalogPart {
        CatalogPart {
            manufacturer: maker.into(),
            order_number: order.into(),
            description: format!("Contactor {order} coil 24 V DC"),
            dimensions: dims.map(|d| Dimensions {
                width_mm: d[0],
                height_mm: d[1],
                depth_mm: d[2],
            }),
            ..Default::default()
        }
    }

    /// The header and product lines of the real Schneider LC1D09 model,
    /// reproduced by hand (the file itself stays out of the repository).
    const SCHNEIDER_STEP: &str = "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\nFILE_NAME('LC1D09-18-DC-3D_ASM','2000-01-01T00:00:00',('AUTHOR01'),(\n'Schneider-Electric'),'CREO PARAMETRIC BY PTC INC, 2019471',\n'CREO PARAMETRIC BY PTC INC, 2019471','');\nENDSEC;\nDATA;\n#26603=PRODUCT('LC1D09-18C-DC','LC1D09-18C-DC','NOT SPECIFIED',(#26602));\n#26605=PRODUCT_DEFINITION('design','',#26604,#26601);\n#54227=PRODUCT('LC1D09-18-DC-3D_ASM','LC1D09-18-DC-3D_ASM','NOT SPECIFIED',(\n#54226));\nENDSEC;\nEND-ISO-10303-21;\n";

    fn schneider() -> CadIdentity {
        CadIdentity::read("MCADPP0003371_3D-simplified.stp", SCHNEIDER_STEP.as_bytes())
    }

    fn catalog() -> Catalog {
        let mut c = Catalog::new();
        for (o, d) in [
            ("LC1D09BD", Some([45.0, 77.0, 86.0])),
            ("LC1D12BD", Some([45.0, 77.0, 86.0])),
            ("LC1D18BD", Some([45.0, 77.0, 86.0])),
            ("LC1D25BD", Some([45.0, 77.0, 86.0])),
            ("LC1F185BD", Some([120.0, 160.0, 140.0])),
        ] {
            c.upsert(part("Schneider Electric", o, d));
        }
        c.upsert(part("WAGO", "2002-1201", Some([5.2, 87.0, 36.0])));
        c
    }

    #[test]
    fn the_step_gives_its_header_and_product_names_and_maker() {
        let id = schneider();
        let names: Vec<&str> = id.names.iter().map(|n| n.text.as_str()).collect();
        assert!(names.contains(&"LC1D09-18-DC-3D_ASM"), "{names:?}");
        assert!(names.contains(&"LC1D09-18C-DC"), "{names:?}");
        assert!(
            !names.iter().any(|n| n.contains("NOT SPECIFIED")),
            "{names:?}"
        );
        assert_eq!(id.maker.as_deref(), Some("Schneider-Electric"));
        assert_eq!(id.doc_id.as_deref(), Some("MCADPP0003371"));
    }

    #[test]
    fn product_definition_is_not_mistaken_for_a_product() {
        let id = schneider();
        assert!(!id.names.iter().any(|n| n.text == "design"));
    }

    #[test]
    fn a_family_range_covers_the_articles_inside_it_and_no_others() {
        let c = catalog();
        let ranked = rank(&schneider(), &c, &CadLinks::default(), 10);
        let labels: Vec<&str> = ranked.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Schneider Electric LC1D09BD",
                "Schneider Electric LC1D12BD",
                "Schneider Electric LC1D18BD"
            ]
        );
        assert!(ranked.iter().all(|c| c.verdict == Verdict::Likely));
        assert!(ranked[0].evidence[0].contains("LC1D09 to LC1D18"));
        // The description says DC, which the range's tail says too.
        assert!(ranked[0].evidence[0].contains("also says DC"));
    }

    #[test]
    fn the_order_number_in_the_file_name_is_enough_and_a_longer_article_wins() {
        let mut c = catalog();
        c.upsert(part("Schneider Electric", "LC1D09", None));
        let id = CadIdentity::from_file_name("LC1D09BD.stp");
        let ranked = rank(&id, &c, &CadLinks::default(), 5);
        assert_eq!(ranked.len(), 1, "{ranked:?}");
        assert_eq!(ranked[0].label, "Schneider Electric LC1D09BD");
        assert_eq!(ranked[0].verdict, Verdict::Verified);
        // LC1D09 is a prefix of the named article, so it is not matched.
        let short = assess_part(
            &id,
            c.find("Schneider Electric", "LC1D09").unwrap(),
            Some(&c),
            &CadLinks::default(),
        );
        assert_eq!(short.verdict, Verdict::Unmatched);
        assert!(short.warnings[0].contains("LC1D09BD"));
    }

    #[test]
    fn the_wago_zip_name_is_matched_by_the_digits_of_the_order_number() {
        let c = catalog();
        let id = CadIdentity::from_file_name("WAGO_20021201HI.zip");
        let ranked = rank(&id, &c, &CadLinks::default(), 5);
        assert_eq!(ranked[0].label, "WAGO 2002-1201");
        assert_eq!(ranked[0].verdict, Verdict::Verified);
        // Not inside a longer digit run.
        let id = CadIdentity::from_file_name("x120021201.zip");
        assert!(rank(&id, &c, &CadLinks::default(), 5).is_empty());
    }

    #[test]
    fn a_download_set_follows_the_file_that_was_matched() {
        let c = catalog();
        let mut links = CadLinks::default();
        links.add(
            "MCADPP0003371",
            &PartKey::new("Schneider Electric", "LC1D09BD"),
        );
        let dxf = CadIdentity::from_file_name("MCADPP0003371_2D-front.dxf");
        let ranked = rank(&dxf, &c, &links, 5);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].label, "Schneider Electric LC1D09BD");
        assert_eq!(ranked[0].verdict, Verdict::Likely);
        assert!(ranked[0].evidence[0].contains("MCADPP0003371"));
    }

    #[test]
    fn a_batch_gives_the_nameless_drawing_the_candidates_of_its_step_file() {
        let c = catalog();
        let dxf = CadIdentity::from_file_name("MCADPP0003371_2D-front.dxf");
        let out = match_batch(vec![schneider(), dxf], &c, &CadLinks::default(), 5);
        assert_eq!(out[0].candidates.len(), 3);
        assert_eq!(out[1].candidates.len(), 3);
        assert!(out[1].candidates[0].evidence[0].contains("same download set"));
        assert!(out[1].candidates[0].evidence[0].contains("MCADPP0003371_3D-simplified.stp"));
    }

    #[test]
    fn a_size_that_fits_is_evidence_and_a_size_that_does_not_asks_for_a_check() {
        let c = catalog();
        let p = c.find("Schneider Electric", "LC1D09BD").unwrap();
        let fits = schneider().with_size(SizeMeasure {
            size_mm: [44.8, 76.6, 94.6],
            exact: true,
        });
        // 94.6 against 86 is 10 %: simplified models carry terminals.
        let a = assess_part(&fits, p, Some(&c), &CadLinks::default());
        assert_eq!(a.verdict, Verdict::Likely);
        assert!(a.evidence.iter().any(|e| e.contains("size fits")), "{a:?}");
        let big = schneider().with_size(SizeMeasure {
            size_mm: [120.0, 160.0, 140.0],
            exact: true,
        });
        let a = assess_part(&big, p, Some(&c), &CadLinks::default());
        assert_eq!(a.verdict, Verdict::Check);
        assert!(a.warnings[0].contains("probably another part"));
    }

    #[test]
    fn a_size_from_all_points_can_speak_against_a_part_but_never_for_it() {
        let c = catalog();
        let p = c.find("Schneider Electric", "LC1D09BD").unwrap();
        // 543 x 823 x 490 is what the all-points box gave for the real file:
        // far too large, so nothing can be concluded from it.
        let inflated = schneider().with_size(SizeMeasure {
            size_mm: [543.6, 823.4, 489.9],
            exact: false,
        });
        let a = assess_part(&inflated, p, Some(&c), &CadLinks::default());
        assert_eq!(a.verdict, Verdict::Likely);
        assert!(a.warnings.is_empty(), "{a:?}");
        assert!(a.notes[0].contains("not compared"), "{a:?}");
        // A box smaller than the part cannot hold a solid of that part.
        let small = schneider().with_size(SizeMeasure {
            size_mm: [10.0, 12.0, 15.0],
            exact: false,
        });
        assert_eq!(
            assess_part(&small, p, Some(&c), &CadLinks::default()).verdict,
            Verdict::Check
        );
    }

    #[test]
    fn a_file_dropped_on_the_wrong_part_is_asked_about_and_the_right_part_is_suggested() {
        let c = catalog();
        let wrong = c.find("Schneider Electric", "LC1F185BD").unwrap();
        match decide_drop(&schneider(), wrong, &c, &CadLinks::default()) {
            DropDecision::Ask { reasons, better } => {
                assert!(reasons[0].contains("LC1D09-18-DC"), "{reasons:?}");
                assert_eq!(better.unwrap().label, "Schneider Electric LC1D09BD");
            }
            other => panic!("{other:?}"),
        }
        let right = c.find("Schneider Electric", "LC1D12BD").unwrap();
        assert!(matches!(
            decide_drop(&schneider(), right, &c, &CadLinks::default()),
            DropDecision::Import { .. }
        ));
    }

    #[test]
    fn a_file_that_names_nothing_is_imported_but_said_to_be_unchecked() {
        let c = catalog();
        let p = c.find("WAGO", "2002-1201").unwrap();
        let id = CadIdentity::read("model.stp", b"ISO-10303-21;\nHEADER;\nFILE_NAME('Part1','',(''),(''),'','','');\nENDSEC;\nDATA;\n#1=PRODUCT('','NOT SPECIFIED','',(#2));\nENDSEC;\n");
        match decide_drop(&id, p, &c, &CadLinks::default()) {
            DropDecision::ImportUnchecked { why } => assert!(why.contains("not checked"), "{why}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_zip_is_read_for_its_member_names_and_the_step_inside() {
        use std::io::Write;
        let mut buf = Vec::new();
        {
            let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let o = zip::write::SimpleFileOptions::default();
            z.start_file("data/LC1D12BD.stp", o).unwrap();
            z.write_all(SCHNEIDER_STEP.as_bytes()).unwrap();
            z.start_file("readme.txt", o).unwrap();
            z.write_all(b"hello").unwrap();
            z.finish().unwrap();
        }
        let id = CadIdentity::read("download.zip", &buf);
        assert!(
            id.names
                .iter()
                .any(|n| n.text == "LC1D12BD" && n.from == NameFrom::ZipMember)
        );
        assert!(id.names.iter().any(|n| n.text == "LC1D09-18-DC-3D_ASM"));
        let ranked = rank(&id, &catalog(), &CadLinks::default(), 5);
        assert_eq!(ranked[0].label, "Schneider Electric LC1D12BD");
        assert_eq!(ranked[0].verdict, Verdict::Verified);
    }

    #[test]
    fn a_dxf_comment_gives_the_download_set_id() {
        let dxf = "999\nDXF file: C:\\Users\\x\\Dev\\MCADPP0003371_3D-CAD\\drw0001_4.dxf\n999\nPro/ENGINEER 2019471 by Parametric Technology Corporation\n  0\nSECTION\n";
        let id = CadIdentity::read("drawing.dxf", dxf.as_bytes());
        assert_eq!(id.doc_id.as_deref(), Some("MCADPP0003371"));
    }

    #[test]
    fn links_are_saved_beside_the_library_and_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let mut l = CadLinks::default();
        l.add(
            "mcadpp0003371",
            &PartKey::new("Schneider Electric", "LC1D09BD"),
        );
        l.save(dir.path()).unwrap();
        let back = CadLinks::load(dir.path());
        assert_eq!(back, l);
        assert_eq!(back.keys_for("MCADPP0003371").len(), 1);
        // A damaged file is no links, not an error.
        std::fs::write(CadLinks::path_in(dir.path()), "{ nope").unwrap();
        assert!(CadLinks::load(dir.path()).is_empty());
    }

    #[test]
    fn family_parsing_rejects_things_that_are_not_ranges() {
        for t in [
            "RJ45-8",
            "M12-12",
            "LC1D09-DC",
            "3RT2016-1BB41",
            "A-B",
            "X1-5",
        ] {
            assert_eq!(family_of(t), None, "{t}");
        }
        let f = family_of("LC1D09-18C-DC").unwrap();
        assert_eq!((f.lo, f.hi, f.width), (9, 18, 2));
        assert_eq!(f.rest, ["C", "DC"]);
        assert!(f.covers("lc1d12bd") && !f.covers("lc1d25bd") && !f.covers("lc1d9bd"));
    }

    /// A maker catalog of tens of thousands of parts, and a family file that
    /// ties the file to hundreds of them: matching must not read the whole
    /// catalog again for each candidate.
    #[test]
    fn a_family_file_against_a_large_catalog_is_matched_in_reasonable_time() {
        let mut c = Catalog::new();
        let mut n = 0;
        for nn in 9..100 {
            for k in 0..220 {
                c.upsert(part(
                    "Schneider Electric",
                    &format!("LC1D{nn:02}BD{k}"),
                    Some([45.0, 77.0, 86.0]),
                ));
                n += 1;
            }
        }
        assert!(n >= 20_000);
        // The file names the family and, for one of its members, the number.
        let id = schneider();
        let t = std::time::Instant::now();
        let out = rank(&id, &c, &CadLinks::default(), 10_000);
        let took = t.elapsed();
        eprintln!("{} parts, {} candidates in {took:?}", c.len(), out.len());
        assert!(out.len() >= 2000, "{}", out.len());
        assert!(took.as_secs_f64() < 3.0, "{took:?}");

        // The exact order number is still told apart from a shorter one.
        let exact = CadIdentity::from_file_name("LC1D09BD5.stp");
        let t = std::time::Instant::now();
        let top = rank(&exact, &c, &CadLinks::default(), 3);
        assert!(t.elapsed().as_secs_f64() < 3.0, "{:?}", t.elapsed());
        assert_eq!(top[0].label, "Schneider Electric LC1D09BD5");
    }

    /// Many files dropped together against a large catalog: the catalog is
    /// read once for the batch, not once per file.
    #[test]
    fn many_files_against_a_large_catalog_are_matched_in_reasonable_time() {
        // 50 product lines of 91 sizes with 5 variants each: a family file
        // (`LC1D09-18`) ties to the 50 parts of its own line only.
        let mut c = Catalog::new();
        let lines: Vec<String> = std::iter::once("LC1D".to_owned())
            .chain((0..49).map(|s| format!("LK{s:02}D")))
            .collect();
        for line in &lines {
            for nn in 9..100 {
                for k in 0..5 {
                    c.upsert(part(
                        "Schneider Electric",
                        &format!("{line}{nn:02}BD{k}"),
                        Some([45.0, 77.0, 86.0]),
                    ));
                }
            }
        }
        let mut ids = Vec::new();
        for i in 0..300 {
            ids.push(match i % 3 {
                0 => CadIdentity::from_file_name(&format!("LC1D{:02}BD{}.stp", 9 + i % 90, i % 5)),
                1 => schneider(),
                _ => CadIdentity::from_file_name(&format!("MCADPP{i:07}_3D-simplified.stp")),
            });
        }
        let t = std::time::Instant::now();
        let out = match_batch(ids, &c, &CadLinks::default(), 20);
        let took = t.elapsed();
        eprintln!("300 files against {} parts in {took:?}", c.len());
        assert!(c.len() > 20_000);
        assert_eq!(out.len(), 300);
        assert!(took.as_secs_f64() < 6.0, "{took:?}");
        assert_eq!(out[0].candidates[0].verdict, Verdict::Verified);
        assert_eq!(out[1].candidates.len(), 20, "the family's first twenty");
        assert!(
            out[2].candidates.is_empty(),
            "a bare document id names nothing"
        );
    }

    /// The lookup by codes finds every part the full judgement would tie to
    /// the file: same answer as testing every part.
    #[test]
    fn the_indexed_match_equals_testing_every_part() {
        let mut c = catalog();
        let mut typed = part("Schneider Electric", "LC1D32BD", None);
        typed.type_number = "TYPE-D32".into();
        c.upsert(typed);
        c.upsert(part("Schneider Electric", "LC1D09", None));
        c.upsert(part("Schneider Electric", "LC1D09BD7", None));
        let mut links = CadLinks::default();
        links.add("MCADQQ0000001", &c.find("WAGO", "2002-1201").unwrap().key());
        let ids = [
            schneider(),
            CadIdentity::from_file_name("LC1D18BD_3D.stp"),
            CadIdentity::from_file_name("LC1D09BD7.stp"),
            CadIdentity::from_file_name("TYPE-D32.stp"),
            CadIdentity::from_file_name("2002-1201.stp"),
            CadIdentity::from_file_name("MCADQQ0000001_2D-front.dxf"),
            CadIdentity::from_file_name("holiday.stp"),
        ];
        for id in &ids {
            let mut brute: Vec<Candidate> = c
                .parts()
                .iter()
                .map(|p| assess_part(id, p, Some(&c), &links))
                .filter(|x| x.score > 0)
                .collect();
            brute.sort_by(|a, b| {
                b.verdict
                    .cmp(&a.verdict)
                    .then(b.score.cmp(&a.score))
                    .then(a.label.cmp(&b.label))
            });
            assert_eq!(rank(id, &c, &links, 100), brute, "{}", id.file_name);
        }
    }

    #[test]
    fn a_links_file_with_garbage_or_empty_rows_links_nothing_wrong() {
        let dir = std::env::temp_dir().join(format!("kopercad-links-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = CadLinks::path_in(&dir);
        std::fs::write(&path, "{ not json").unwrap();
        assert!(CadLinks::load(&dir).is_empty());
        std::fs::write(
            &path,
            r#"{"format":1,"links":[
              {"doc_id":"","manufacturer":"X","order_number":"A-1"},
              {"doc_id":"MCAD1","manufacturer":" ","order_number":"A-1"},
              {"doc_id":"MCAD1","manufacturer":"X","order_number":""},
              {"doc_id":"MCAD2","manufacturer":"X","order_number":"A-2"}]}"#,
        )
        .unwrap();
        let links = CadLinks::load(&dir);
        assert!(links.keys_for("MCAD1").is_empty());
        assert_eq!(links.keys_for("mcad2").len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

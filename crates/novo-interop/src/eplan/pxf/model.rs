//! What a PXF file holds, as plain values. Nothing here refers to the XML.

use std::collections::BTreeMap;

use super::text::MlText;

/// `4/123`: object type and number. Numbers are unique in the project;
/// devices and cable cores share one number space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjId {
    pub kind: u32,
    pub number: u64,
}

impl ObjId {
    pub fn parse(s: &str) -> Option<Self> {
        let (k, n) = s.trim().split_once('/')?;
        Some(Self {
            kind: k.parse().ok()?,
            number: n.parse().ok()?,
        })
    }
}

/// What kind of file the root says it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PxfKind {
    /// A project export (`.epj`).
    Project,
    /// A page macro (`.emp`).
    PageMacro,
    /// Some other `Type` value.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PxfHeader {
    pub kind: PxfKind,
    /// The raw `Type` attribute.
    pub type_name: String,
    pub version: String,
    pub pxf_version: String,
    pub schema_version: String,
    /// `NumMainObjects` as the file states it.
    pub declared_objects: Option<u64>,
}

/// A device tag in its parts: function `=`, location `+`, then the device.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PxfTag {
    pub function: String,
    pub location: String,
    pub prefix: String,
    pub letters: String,
    pub number: String,
    pub sub: String,
    /// The tag as EPLAN shows it (`P144`), when the file has it.
    pub display: String,
}

impl PxfTag {
    pub fn is_empty(&self) -> bool {
        self.letters.is_empty() && self.number.is_empty() && self.display.is_empty()
    }

    /// `-K1.2`: the device part. A prefix that is not a sign (`+`, `-`, `=`)
    /// is read as part of the name and the sign is `-`. A sub index is
    /// written after a dot.
    pub fn device_name(&self) -> String {
        let signs = ['-', '+', '=', '&', '#'];
        let mut s = String::new();
        if self
            .prefix
            .chars()
            .next()
            .is_some_and(|c| signs.contains(&c))
        {
            s.push_str(&self.prefix);
        } else {
            s.push('-');
            s.push_str(&self.prefix);
        }
        s.push_str(&self.letters);
        s.push_str(&self.number);
        if !self.sub.is_empty() {
            if !self.sub.starts_with('.') {
                s.push('.');
            }
            s.push_str(&self.sub);
        }
        s
    }

    /// `=A1+CAB1-K1`, KoperCAD's order (IEC 81346). Empty parts are left out.
    pub fn full(&self) -> String {
        let mut s = String::new();
        let f = self.function.trim_start_matches('=');
        if !f.is_empty() {
            s.push('=');
            s.push_str(f);
        }
        let l = self.location.trim_start_matches('+');
        if !l.is_empty() {
            s.push('+');
            s.push_str(l);
        }
        s.push_str(&self.device_name());
        s
    }
}

/// `SCHNEI.GV2P10`: EPLAN's maker and type, split at the first dot.
pub fn split_part_number(p: &str) -> (&str, &str) {
    p.split_once('.').unwrap_or(("", p))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PxfPage {
    pub id: Option<ObjId>,
    pub function: String,
    pub location: String,
    pub number: String,
    pub suffix: String,
    pub title: MlText,
    /// Objects found on the page (all kinds).
    pub objects: usize,
}

impl PxfPage {
    /// `=A1+CAB1/12`-style label.
    pub fn label(&self) -> String {
        let mut s = String::new();
        if !self.function.is_empty() {
            s.push('=');
            s.push_str(self.function.trim_start_matches('='));
        }
        if !self.location.is_empty() {
            s.push('+');
            s.push_str(self.location.trim_start_matches('+'));
        }
        s.push('/');
        s.push_str(&self.number);
        s.push_str(&self.suffix);
        s
    }
}

/// What a tagged object is, decided from its texts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceRole {
    Device,
    /// Carries a terminal text.
    Terminal,
    /// Carries a cable text and no terminal text.
    Cable,
}

/// The symbol a device uses, as EPLAN numbers it (`A1381` to `A1383`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolRef {
    pub number: i64,
    pub variant: i64,
    /// Library set id; resolve through the file's `ESymLib` rows.
    pub library: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PxfDevice {
    pub id: Option<ObjId>,
    /// Index into [`PxfFile::pages`].
    pub page: Option<usize>,
    pub role: DeviceRole,
    pub tag: PxfTag,
    pub type_text: MlText,
    /// EPLAN part number, `MAKER.TYPE`.
    pub part_number: String,
    pub terminal_text: String,
    pub cable_text: String,
    pub symbol: Option<SymbolRef>,
    /// Millimetres, y up, origin bottom left of the sheet.
    pub position: Option<(f64, f64)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PxfCore {
    pub id: Option<ObjId>,
    pub page: Option<usize>,
    /// The cable it belongs to, when the core names it.
    pub cable: PxfTag,
    pub number: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PxfInterruption {
    pub id: Option<ObjId>,
    pub page: Option<usize>,
    pub tag: PxfTag,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PxfText {
    pub id: Option<ObjId>,
    pub page: Option<usize>,
    pub text: MlText,
    pub position: Option<(f64, f64)>,
}

/// Everything read from one PXF file.
#[derive(Debug, Clone, PartialEq)]
pub struct PxfFile {
    pub header: PxfHeader,
    pub pages: Vec<PxfPage>,
    pub devices: Vec<PxfDevice>,
    pub cores: Vec<PxfCore>,
    pub interruptions: Vec<PxfInterruption>,
    pub texts: Vec<PxfText>,
    /// Objects the reader does not carry, by object type.
    pub skipped: BTreeMap<u32, usize>,
    /// Ids that occur twice (EPLAN would refuse such a file on import).
    pub duplicate_ids: Vec<ObjId>,
    /// Things worth telling the person.
    pub notes: Vec<String>,
}

impl PxfFile {
    pub fn devices_of(&self, role: DeviceRole) -> impl Iterator<Item = &PxfDevice> {
        self.devices.iter().filter(move |d| d.role == role)
    }
}

/// Interruption points with the same full name pair up two by two in file
/// (page) order; an odd one stays alone. Returns the pairs and the lonely
/// ones, as indexes into `points`.
pub fn pair_interruptions(points: &[PxfInterruption]) -> (Vec<(usize, usize)>, Vec<usize>) {
    let mut by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, p) in points.iter().enumerate() {
        by_name.entry(p.tag.full()).or_default().push(i);
    }
    let mut pairs = Vec::new();
    let mut alone = Vec::new();
    for idx in by_name.values() {
        for c in idx.chunks(2) {
            match c {
                [a, b] => pairs.push((*a, *b)),
                [a] => alone.push(*a),
                _ => {}
            }
        }
    }
    pairs.sort_unstable();
    alone.sort_unstable();
    (pairs, alone)
}

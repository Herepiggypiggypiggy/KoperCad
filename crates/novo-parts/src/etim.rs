//! The ETIM release the user supplies, to read ETIM codes in maker files.
//!
//! Real BMEcat files from makers write a feature as a code, not a name:
//! `EF000008` for the feature, `EU570448` for its unit, `EV000207` for a
//! choice. What a code means is in the ETIM model, which ETIM International
//! publishes as a release the user downloads (free of charge; the download
//! page may ask for a sign-in, which happens in the user's own browser).
//! KoperCAD carries no ETIM codes of its own and does not bundle the
//! release: [`EtimModel::install`] reads the user's copy once, keeps a
//! compact copy in the user's profile and every later import reads it.
//!
//! The reader takes the CSV tables of a release (as a ZIP, a folder or a
//! single file) and finds what each holds by the codes in its columns, not by
//! file or column names, because those differ between releases: a table
//! whose single code column holds `EF` codes is the feature table, `EV` the
//! values, `EU` the units, `EC` the classes. The English text comes from the
//! column named like `DESC` with an `EN` in it, or from the rows whose
//! language column says English. A table with codes of several kinds (the
//! class-to-feature link) is skipped.
//!
//! Written from the public description of the data model and tested on
//! synthetic tables in the layouts above; it has not yet read an actual
//! release file. A release that has no readable table gives an error that
//! names the files it looked at.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::csv::parse_delimited;
use crate::import::decode_text;

const MAX_MEMBER_BYTES: u64 = 120 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 400 * 1024 * 1024;
const MAX_ENTRIES: usize = 400;
/// Rows looked at to decide what a column holds.
const SAMPLE_ROWS: usize = 200;
const FORMAT: u32 = 1;

#[derive(Debug, Error)]
pub enum EtimError {
    #[error("cannot read {}: {}", path.display(), crate::plain::plain_io_error(source))]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{0}")]
    Unreadable(String),
    #[error(
        "no table with ETIM feature codes (EF000000) was found in {looked}; \
         pick the CSV release from ETIM International (a ZIP, or the folder it was unpacked to)"
    )]
    NoTables { looked: String },
}

/// Feature, value, unit and class texts of one ETIM release.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EtimModel {
    #[serde(default)]
    format: u32,
    /// What the user chose, for display (`ETIM-9.0-CSV.zip`).
    pub source: String,
    features: BTreeMap<String, String>,
    values: BTreeMap<String, String>,
    /// Unit code to symbol (`mm`, `A`), or the unit's English name when the
    /// release has no symbol.
    units: BTreeMap<String, String>,
    classes: BTreeMap<String, String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Feature,
    Value,
    Unit,
    Class,
}

impl Kind {
    fn prefix(self) -> &'static str {
        match self {
            Kind::Feature => "EF",
            Kind::Value => "EV",
            Kind::Unit => "EU",
            Kind::Class => "EC",
        }
    }
}

fn is_code(s: &str, prefix: &str) -> bool {
    let s = s.trim();
    s.len() == 8 && s.starts_with(prefix) && s[2..].bytes().all(|b| b.is_ascii_digit())
}

impl EtimModel {
    pub fn is_feature_code(s: &str) -> bool {
        is_code(s, "EF")
    }
    pub fn is_value_code(s: &str) -> bool {
        is_code(s, "EV")
    }
    pub fn is_unit_code(s: &str) -> bool {
        is_code(s, "EU")
    }
    pub fn is_class_code(s: &str) -> bool {
        is_code(s, "EC")
    }

    /// English name of a feature code.
    pub fn feature(&self, code: &str) -> Option<&str> {
        self.features.get(code.trim()).map(String::as_str)
    }
    /// English text of a value code.
    pub fn value(&self, code: &str) -> Option<&str> {
        self.values.get(code.trim()).map(String::as_str)
    }
    /// Symbol (or name) of a unit code.
    pub fn unit(&self, code: &str) -> Option<&str> {
        self.units.get(code.trim()).map(String::as_str)
    }
    /// English name of a class code.
    pub fn class(&self, code: &str) -> Option<&str> {
        self.classes.get(code.trim()).map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.features.is_empty()
    }

    /// One line for the screen: what the loaded release holds.
    pub fn summary(&self) -> String {
        format!(
            "{}: {} features, {} values, {} units, {} classes",
            if self.source.is_empty() {
                "ETIM release"
            } else {
                &self.source
            },
            self.features.len(),
            self.values.len(),
            self.units.len(),
            self.classes.len()
        )
    }

    /// Reads the tables out of named files (name, content).
    pub fn from_files(source: &str, files: &[(String, Vec<u8>)]) -> Result<Self, EtimError> {
        let mut m = EtimModel {
            format: FORMAT,
            source: source.to_owned(),
            ..Default::default()
        };
        for (_, bytes) in files {
            m.take_table(&decode_text(bytes));
        }
        if m.features.is_empty() {
            let looked = if files.is_empty() {
                "no CSV files".to_owned()
            } else {
                files
                    .iter()
                    .take(6)
                    .map(|(n, _)| n.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            return Err(EtimError::NoTables { looked });
        }
        Ok(m)
    }

    /// Reads a release from a ZIP, a folder of CSV files or one CSV file.
    pub fn read(path: &Path) -> Result<Self, EtimError> {
        let source = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let io = |source: std::io::Error| EtimError::Io {
            path: path.to_owned(),
            source,
        };
        let mut files: Vec<(String, Vec<u8>)> = Vec::new();
        if path.is_dir() {
            let mut total = 0u64;
            collect(path, &mut files, &mut total, 0)?;
        } else {
            let size = fs::metadata(path).map_err(io)?.len();
            if size > MAX_TOTAL_BYTES {
                return Err(EtimError::Unreadable(format!(
                    "{source} is {} MB, more than the {} MB read from one release",
                    size / (1024 * 1024),
                    MAX_TOTAL_BYTES / (1024 * 1024)
                )));
            }
            let bytes = fs::read(path).map_err(io)?;
            if bytes.starts_with(b"PK") {
                files = read_zip(&bytes)?;
            } else {
                files.push((source.clone(), bytes));
            }
        }
        Self::from_files(&source, &files)
    }

    /// Reads the release at `path` and keeps a compact copy in `dir`, which
    /// later imports load with [`EtimModel::load_installed`].
    pub fn install(path: &Path, dir: &Path) -> Result<Self, EtimError> {
        let model = Self::read(path)?;
        fs::create_dir_all(dir).map_err(|source| EtimError::Io {
            path: dir.to_owned(),
            source,
        })?;
        let file = Self::installed_path(dir);
        let json = serde_json::to_vec(&model).map_err(|e| EtimError::Unreadable(e.to_string()))?;
        let tmp = file.with_extension("json.tmp");
        fs::write(&tmp, json)
            .and_then(|()| fs::rename(&tmp, &file))
            .map_err(|source| EtimError::Io { path: file, source })?;
        Ok(model)
    }

    /// The release kept by [`EtimModel::install`], if one is there and
    /// readable.
    pub fn load_installed(dir: &Path) -> Option<Self> {
        let bytes = fs::read(Self::installed_path(dir)).ok()?;
        let m: EtimModel = serde_json::from_slice(&bytes).ok()?;
        (m.format == FORMAT && !m.is_empty()).then_some(m)
    }

    pub fn installed_path(dir: &Path) -> PathBuf {
        dir.join("etim-model.json")
    }

    /// The folder the installed release lives in: `etim` beside the catalog
    /// (the given one, else the user's), so it follows the profile folder.
    pub fn dir_for(catalog: Option<&Path>) -> Option<PathBuf> {
        let catalog = match catalog {
            Some(c) => c.to_owned(),
            None => crate::catalog::user_catalog_path()?,
        };
        catalog.parent().map(|d| d.join("etim"))
    }

    fn take_table(&mut self, text: &str) {
        let Ok(rows) = parse_delimited(text) else {
            return;
        };
        let Some((head, body)) = rows.split_first() else {
            return;
        };
        let head: Vec<&str> = head.fields.iter().map(String::as_str).collect();
        let sample: Vec<&Vec<String>> = body.iter().take(SAMPLE_ROWS).map(|r| &r.fields).collect();

        // The code columns: more than four in five filled cells of one kind.
        let mut code_cols: Vec<(usize, Kind)> = Vec::new();
        for col in 0..head.len() {
            for kind in [Kind::Feature, Kind::Value, Kind::Unit, Kind::Class] {
                let cells: Vec<&str> = sample
                    .iter()
                    .filter_map(|r| r.get(col).map(|c| c.trim()))
                    .filter(|c| !c.is_empty())
                    .collect();
                if cells.len() >= 2
                    && cells.iter().filter(|c| is_code(c, kind.prefix())).count() * 5
                        > cells.len() * 4
                {
                    code_cols.push((col, kind));
                }
            }
        }
        // One kind of code only: a link table (class, feature, unit) says
        // nothing about what a code means.
        let [(code_col, kind)] = code_cols[..] else {
            return;
        };
        let lang_col = head.iter().position(|h| {
            let h = h.to_ascii_uppercase();
            h.contains("LANG")
        });
        let desc_col = describe_col(&head, code_col, lang_col, &sample);
        let symbol_col = (kind == Kind::Unit)
            .then(|| {
                head.iter().position(|h| {
                    let h = h.to_ascii_uppercase();
                    h.contains("ABBREV") || h.contains("SYMBOL") || h.contains("SIGN")
                })
            })
            .flatten();

        let target = match kind {
            Kind::Feature => &mut self.features,
            Kind::Value => &mut self.values,
            Kind::Unit => &mut self.units,
            Kind::Class => &mut self.classes,
        };
        for row in body {
            let f = &row.fields;
            let Some(code) = f.get(code_col).map(|c| c.trim()) else {
                continue;
            };
            if !is_code(code, kind.prefix()) {
                continue;
            }
            if let Some(l) = lang_col
                && !f.get(l).is_some_and(|v| is_english(v))
            {
                continue;
            }
            let text = desc_col
                .and_then(|c| f.get(c))
                .map(|t| t.trim())
                .unwrap_or("");
            let symbol = symbol_col
                .and_then(|c| f.get(c))
                .map(|t| t.trim())
                .unwrap_or("");
            let shown = match kind {
                Kind::Unit if !symbol.is_empty() => symbol.to_owned(),
                Kind::Unit => unit_symbol(text).unwrap_or(text).to_owned(),
                _ => text.to_owned(),
            };
            if !shown.is_empty() {
                target.entry(code.to_owned()).or_insert(shown);
            }
        }
    }
}

/// The sentence for codes a file used that could not be translated: what
/// they were and what to do.
pub(crate) fn unread_note(
    codes: &std::collections::BTreeSet<String>,
    model: Option<&EtimModel>,
) -> String {
    let some: Vec<&str> = codes.iter().take(4).map(String::as_str).collect();
    let n = codes.len();
    let list = format!(
        "{}{}",
        some.join(", "),
        if n > some.len() { ", ..." } else { "" }
    );
    let codes_word = if n == 1 {
        "ETIM code was"
    } else {
        "ETIM codes were"
    };
    match model {
        None => format!(
            "{n} {codes_word} not translated ({list}) because no ETIM release is installed. \
             Download the ETIM release from ETIM International, install it (Parts window, \
             ETIM release, or `kopercad-cli parts etim <file>`) and import the file again."
        ),
        Some(m) => format!(
            "{n} {codes_word} not translated ({list}): not in the installed release ({}). \
             A newer release may know them.",
            if m.source.is_empty() {
                "ETIM"
            } else {
                &m.source
            }
        ),
    }
}

fn is_english(v: &str) -> bool {
    let v = v.trim().to_ascii_lowercase();
    v == "en" || v == "eng" || v == "english" || v.starts_with("en-") || v.starts_with("en_")
}

/// The column holding the English text of the code.
fn describe_col(
    head: &[&str],
    code_col: usize,
    lang_col: Option<usize>,
    sample: &[&Vec<String>],
) -> Option<usize> {
    let upper: Vec<String> = head.iter().map(|h| h.to_ascii_uppercase()).collect();
    let texty = |i: usize| {
        i != code_col
            && Some(i) != lang_col
            && (upper[i].contains("DESC") || upper[i].contains("NAME") || upper[i].contains("TEXT"))
    };
    let english = |i: usize| {
        upper[i]
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|w| matches!(w, "EN" | "ENG" | "ENGB" | "ENGLISH" | "ENUS"))
    };
    (0..head.len())
        .find(|&i| texty(i) && english(i))
        .or_else(|| (0..head.len()).find(|&i| texty(i)))
        // No header says so: the first column after the code that holds text.
        .or_else(|| {
            (0..head.len()).find(|&i| {
                i != code_col
                    && Some(i) != lang_col
                    && sample
                        .iter()
                        .filter_map(|r| r.get(i))
                        .any(|c| c.chars().any(char::is_alphabetic))
            })
        })
}

/// The symbol of a unit given by its English name; only units that have one
/// everybody writes the same way.
fn unit_symbol(name: &str) -> Option<&'static str> {
    let n = name.trim().to_ascii_lowercase().replace('-', " ");
    Some(match n.as_str() {
        "millimetre" | "millimeter" => "mm",
        "centimetre" | "centimeter" => "cm",
        "metre" | "meter" => "m",
        "kilometre" | "kilometer" => "km",
        "square millimetre" | "square millimeter" => "mm2",
        "ampere" => "A",
        "milliampere" => "mA",
        "kiloampere" => "kA",
        "volt" => "V",
        "millivolt" => "mV",
        "kilovolt" => "kV",
        "watt" => "W",
        "kilowatt" => "kW",
        "volt ampere" => "VA",
        "hertz" => "Hz",
        "kilohertz" => "kHz",
        "ohm" => "ohm",
        "kilogram" => "kg",
        "gram" => "g",
        "second" => "s",
        "millisecond" => "ms",
        "minute" => "min",
        "hour" => "h",
        "degree celsius" => "\u{b0}C",
        "percent" => "%",
        "newton metre" | "newton meter" => "Nm",
        "newton" => "N",
        "litre" | "liter" => "l",
        _ => return None,
    })
}

fn collect(
    dir: &Path,
    out: &mut Vec<(String, Vec<u8>)>,
    total: &mut u64,
    depth: usize,
) -> Result<(), EtimError> {
    let io = |source: std::io::Error| EtimError::Io {
        path: dir.to_owned(),
        source,
    };
    let mut entries: Vec<_> = fs::read_dir(dir).map_err(io)?.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let Ok(t) = e.file_type() else { continue };
        if t.is_dir() && depth < 3 {
            collect(&p, out, total, depth + 1)?;
        } else if t.is_file() && is_table_name(&p.to_string_lossy()) {
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            if size > MAX_MEMBER_BYTES
                || *total + size > MAX_TOTAL_BYTES
                || out.len() >= MAX_ENTRIES
            {
                return Err(EtimError::Unreadable(format!(
                    "{} is too large to read as an ETIM table",
                    p.display()
                )));
            }
            *total += size;
            let bytes = fs::read(&p).map_err(|source| EtimError::Io {
                path: p.clone(),
                source,
            })?;
            out.push((e.file_name().to_string_lossy().into_owned(), bytes));
        }
    }
    Ok(())
}

fn is_table_name(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    l.ends_with(".csv") || l.ends_with(".txt") || l.ends_with(".tsv")
}

fn read_zip(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, EtimError> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| EtimError::Unreadable(format!("not a readable ZIP ({e})")))?;
    if archive.len() > MAX_ENTRIES {
        return Err(EtimError::Unreadable(format!(
            "the ZIP has {} files, more than {MAX_ENTRIES}",
            archive.len()
        )));
    }
    let mut out = Vec::new();
    let mut total = 0u64;
    for i in 0..archive.len() {
        let Ok(file) = archive.by_index(i) else {
            continue;
        };
        if !file.is_file() || !is_table_name(file.name()) {
            continue;
        }
        if file.size() > MAX_MEMBER_BYTES || total + file.size() > MAX_TOTAL_BYTES {
            return Err(EtimError::Unreadable(format!(
                "{} is too large to read as an ETIM table",
                file.name()
            )));
        }
        let name = file
            .name()
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or("")
            .to_owned();
        let mut buf = Vec::new();
        file.take(MAX_MEMBER_BYTES)
            .read_to_end(&mut buf)
            .map_err(|e| EtimError::Unreadable(format!("{name}: {e}")))?;
        total += buf.len() as u64;
        out.push((name, buf));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(list: &[(&str, &str)]) -> Vec<(String, Vec<u8>)> {
        list.iter()
            .map(|(n, t)| (n.to_string(), t.as_bytes().to_vec()))
            .collect()
    }

    // Codes in these tables are made up for the test (the layout is what is
    // tested); they are not ETIM's.
    const FEATURES: &str = "FEATURECODE;FEATUREDESC_EN;FEATUREDESC_DE\n\
        EF900001;Test width;Test Breite\n\
        EF900002;Test rated current;Test Bemessungsstrom\n\
        EF900003;Test number of poles;Test Polzahl\n";
    const UNITS: &str = "UNITCODE;UNITDESC_EN;ABBREVIATION\n\
        EU900001;Test millimetre;mm\n\
        EU900002;Test ampere;A\n";
    const VALUES: &str = "VALUECODE;VALUEDESC_EN\nEV900001;Test push-in\nEV900002;Test screw\n";
    const LINKS: &str =
        "CLASSCODE;FEATURECODE;UNITCODE\nEC900001;EF900001;EU900001\nEC900001;EF900002;EU900002\n";

    #[test]
    fn tables_are_found_by_their_codes_not_their_names() {
        let m = EtimModel::from_files(
            "x.zip",
            &files(&[
                ("a.csv", FEATURES),
                ("b.csv", UNITS),
                ("c.csv", VALUES),
                ("d.csv", LINKS),
            ]),
        )
        .unwrap();
        assert_eq!(m.feature("EF900001"), Some("Test width"));
        assert_eq!(m.unit("EU900002"), Some("A"));
        assert_eq!(m.value("EV900002"), Some("Test screw"));
        assert_eq!(m.feature("EF000000"), None, "unknown stays unknown");
        assert!(m.summary().contains("3 features"), "{}", m.summary());
    }

    #[test]
    fn a_link_table_with_several_kinds_of_codes_is_not_taken_for_a_feature_table() {
        let err = EtimModel::from_files("x", &files(&[("links.csv", LINKS)])).unwrap_err();
        assert!(matches!(err, EtimError::NoTables { .. }), "{err}");
        assert!(err.to_string().contains("links.csv"), "{err}");
    }

    #[test]
    fn rows_of_a_language_column_are_filtered_to_english() {
        let long = "Featurecode,Language,Description\n\
            EF900001,de,Test Breite\nEF900001,en-GB,Test width\n\
            EF900002,EN,Test current\nEF900002,nl,Test stroom\n";
        let m = EtimModel::from_files("x", &files(&[("f.csv", long)])).unwrap();
        assert_eq!(m.feature("EF900001"), Some("Test width"));
        assert_eq!(m.feature("EF900002"), Some("Test current"));
    }

    #[test]
    fn a_unit_without_a_symbol_column_gets_the_symbol_of_its_name_or_stays_as_named() {
        let units = "Code;Description\nEU900001;Millimetre\nEU900002;Ampere\nEU900003;Furlong\n";
        let m =
            EtimModel::from_files("x", &files(&[("f.csv", FEATURES), ("u.csv", units)])).unwrap();
        assert_eq!(m.unit("EU900001"), Some("mm"));
        assert_eq!(m.unit("EU900002"), Some("A"));
        assert_eq!(m.unit("EU900003"), Some("Furlong"));
    }

    #[test]
    fn windows_1252_and_comma_tables_read_like_utf8_semicolon_ones() {
        let mut bytes = b"Code,Desc EN\nEF900001,Breite \xb0 test\nEF900002,x\n".to_vec();
        bytes.extend_from_slice(b"EF900003,y\n");
        let m = EtimModel::from_files("x", &[("f.csv".into(), bytes)]).unwrap();
        assert!(m.feature("EF900001").unwrap().contains('\u{b0}'));
    }

    #[test]
    fn a_folder_a_zip_and_the_installed_copy_give_the_same_model() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("kopercad-etim-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let rel = dir.join("release");
        fs::create_dir_all(rel.join("nested")).unwrap();
        fs::write(rel.join("features.csv"), FEATURES).unwrap();
        fs::write(rel.join("nested").join("units.csv"), UNITS).unwrap();
        fs::write(rel.join("readme.pdf"), b"not a table").unwrap();

        let from_dir = EtimModel::read(&rel).unwrap();

        let zip_path = dir.join("release.zip");
        {
            let f = fs::File::create(&zip_path).unwrap();
            let mut z = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default();
            z.start_file("ETIM/features.csv", opts).unwrap();
            z.write_all(FEATURES.as_bytes()).unwrap();
            z.start_file("ETIM/units.csv", opts).unwrap();
            z.write_all(UNITS.as_bytes()).unwrap();
            z.finish().unwrap();
        }
        let from_zip = EtimModel::read(&zip_path).unwrap();
        assert_eq!(from_dir.features, from_zip.features);
        assert_eq!(from_dir.units, from_zip.units);

        let store = dir.join("etim");
        let installed = EtimModel::install(&zip_path, &store).unwrap();
        let loaded = EtimModel::load_installed(&store).unwrap();
        assert_eq!(installed, loaded);
        assert_eq!(loaded.source, "release.zip");
        assert!(EtimModel::load_installed(&dir.join("nothing")).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn something_that_is_not_a_release_says_what_it_looked_at() {
        let err = EtimModel::from_files("x", &files(&[("price.csv", "a;b\n1;2\n")])).unwrap_err();
        assert!(err.to_string().contains("price.csv"), "{err}");
        let err = EtimModel::from_files("x", &[]).unwrap_err();
        assert!(err.to_string().contains("no CSV files"), "{err}");
    }
}

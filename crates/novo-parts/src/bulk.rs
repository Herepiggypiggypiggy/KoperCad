//! Bulk import of everything in one folder (the "inbox"), for the
//! user-run `kopercad-cli parts import`.
//!
//! The user drops the catalog files they obtained from the makers or from a
//! wholesaler (BMEcat XML, CSV tables, or ZIPs holding BMEcat) into
//! `inbox/<maker>/`. One run reads every file through the same readers and
//! maker recipes as the one-click import, checks each part, merges the
//! parts into the user's catalog and returns a report per maker.
//!
//! Nothing here touches the network: the files are the user's own downloads,
//! so the limits in `docs/PARTS-LIBRARY-DESIGN.md` (no crawl, no bulk fetch)
//! hold. Re-running is safe: a part is identified by maker and order number,
//! so the same file again changes nothing, and a newer file updates.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::bmecat::import_bmecat;
use crate::catalog::{Catalog, CatalogError, CatalogPart, user_catalog_path};
use crate::csv::import_catalog;
use crate::import::{decode_text, looks_like_bmecat, read_zip};
use crate::library::FileKind;
use crate::quality::{Grade, assess};
use crate::recipes::{ApplyContext, GENERIC, Login, MakerRecipe, RECIPES};
use crate::search::normalize_code;

/// Largest plausible outer size of a part, in mm (a cabinet is ~2200).
const MAX_DIMENSION_MM: f64 = 3000.0;

#[derive(Debug, Error)]
pub enum InboxError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error("no profile folder is known, so there is no default place for the catalog")]
    NoProfile,
}

/// What a run does.
#[derive(Debug, Clone, Default)]
pub struct InboxOptions {
    /// Date stored in the parts' provenance, `2026-10-16`.
    pub today: String,
    /// Read and report, change nothing.
    pub dry_run: bool,
}

/// Counts for one maker.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MakerSummary {
    pub files: usize,
    /// Parts read (after dropping duplicates inside the run).
    pub read: usize,
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub no_dimensions: usize,
    pub no_etim: usize,
    pub no_terminals: usize,
    pub good: usize,
    pub fair: usize,
    pub poor: usize,
    /// Parts with at least one warning.
    pub flagged: usize,
}

/// The result of a run.
#[derive(Debug, Clone, Default)]
pub struct InboxReport {
    pub makers: BTreeMap<String, MakerSummary>,
    /// Files that held no parts, with why.
    pub rejected: Vec<String>,
    /// Files left alone on purpose (geometry, readmes, spreadsheets).
    pub ignored: Vec<String>,
    /// Per-part warnings, `maker order: text`, capped for display.
    pub warnings: Vec<String>,
    /// Recipe lines (unmapped features and so on).
    pub notes: Vec<String>,
    pub dry_run: bool,
    pub catalog_size: usize,
}

impl InboxReport {
    pub fn total_read(&self) -> usize {
        self.makers.values().map(|m| m.read).sum()
    }

    pub fn total_added(&self) -> usize {
        self.makers.values().map(|m| m.added).sum()
    }

    /// Plain-text report for the console and `import-report.txt`.
    pub fn render(&self) -> String {
        let mut t = String::new();
        if self.makers.is_empty() {
            t.push_str("No parts were read.\n");
        } else {
            t.push_str(&format!(
                "{:<22}{:>6}{:>8}{:>8}{:>8}{:>9}{:>8}{:>8}{:>7}\n",
                "Maker", "files", "parts", "new", "updated", "same", "no size", "no ETIM", "flags"
            ));
            for (maker, m) in &self.makers {
                t.push_str(&format!(
                    "{:<22}{:>6}{:>8}{:>8}{:>8}{:>9}{:>8}{:>8}{:>7}\n",
                    truncate(maker, 21),
                    m.files,
                    m.read,
                    m.added,
                    m.updated,
                    m.unchanged,
                    m.no_dimensions,
                    m.no_etim,
                    m.flagged
                ));
            }
            t.push_str(&format!(
                "\nTotal: {} parts read, {} new. Catalog now holds {} parts{}.\n",
                self.total_read(),
                self.total_added(),
                self.catalog_size,
                if self.dry_run {
                    " (dry run: nothing was written)"
                } else {
                    ""
                }
            ));
            t.push_str("\nCompleteness (Good 80%+, Fair 40-79%, Poor under 40%):\n");
            for (maker, m) in &self.makers {
                t.push_str(&format!(
                    "  {maker}: {} good, {} fair, {} poor; {} without terminals\n",
                    m.good, m.fair, m.poor, m.no_terminals
                ));
            }
        }
        section(&mut t, "Files with no parts", &self.rejected, usize::MAX);
        section(&mut t, "Files left alone", &self.ignored, usize::MAX);
        section(&mut t, "Features the recipes did not map", &self.notes, 40);
        section(&mut t, "Parts to check", &self.warnings, 60);
        t
    }
}

fn section(out: &mut String, title: &str, lines: &[String], cap: usize) {
    if lines.is_empty() {
        return;
    }
    out.push_str(&format!("\n{title} ({}):\n", lines.len()));
    for l in lines.iter().take(cap) {
        out.push_str(&format!("  {l}\n"));
    }
    if lines.len() > cap {
        out.push_str(&format!("  ... and {} more\n", lines.len() - cap));
    }
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// The default inbox: `inbox` next to the user's catalog
/// (`%APPDATA%\KoperCAD\inbox` on Windows).
pub fn default_inbox() -> Option<PathBuf> {
    user_catalog_path().and_then(|p| p.parent().map(|d| d.join("inbox")))
}

/// Folder name for a maker: `phoenix-contact`.
fn slug(name: &str) -> String {
    let mut s = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'ü' => 'u',
            'ö' => 'o',
            'ä' => 'a',
            c if c.is_ascii_alphanumeric() => c,
            _ => '-',
        };
        if c != '-' || !s.ends_with('-') {
            s.push(c);
        }
    }
    s.trim_matches('-').to_owned()
}

/// Creates the inbox with one folder per known maker and a `_wholesaler`
/// folder, and writes a short README. Returns the folders created. Safe to
/// run again.
pub fn prepare_inbox(inbox: &Path) -> Result<Vec<PathBuf>, InboxError> {
    let io = |path: &Path| {
        let path = path.to_owned();
        move |source| InboxError::Io { path, source }
    };
    let mut made = Vec::new();
    for name in RECIPES
        .iter()
        .map(|r| slug(r.manufacturer))
        .chain(std::iter::once("_wholesaler".to_owned()))
    {
        let dir = inbox.join(&name);
        if !dir.exists() {
            fs::create_dir_all(&dir).map_err(io(&dir))?;
            made.push(dir);
        }
    }
    let readme = inbox.join("README.txt");
    if !readme.exists() {
        fs::write(
            &readme,
            "KoperCAD parts inbox\r\n\r\n\
Drop the catalog files you downloaded (BMEcat .xml, .csv, or .zip holding BMEcat)\r\n\
into the folder of the maker, then run:  import-parts.cmd (in the KoperCAD-latest folder)\r\n\
\r\n\
A file from a wholesaler or a maker without a folder: make a new folder named after\r\n\
the maker (its name is used as the manufacturer) or use _wholesaler when the file\r\n\
names the maker in every row.\r\n\
\r\n\
Files stay here; running again only adds or updates parts.\r\n",
        )
        .map_err(io(&readme))?;
    }
    Ok(made)
}

/// What to download and where to put it, per maker, as lines for the console.
/// `have` lists makers whose folder already holds files.
pub fn maker_guide(have: &[String]) -> Vec<String> {
    let mut lines = Vec::new();
    for r in RECIPES {
        let state = if have.iter().any(|h| h == r.manufacturer) {
            "files found"
        } else {
            "nothing yet"
        };
        let login = match r.route.login {
            Login::None => "no login",
            Login::Free => "free login",
            Login::Required => "LOGIN NEEDED",
            Login::Unknown => "login unknown",
        };
        lines.push(format!(
            "{} ({state}; {login}): {}  ->  save into inbox\\{}",
            r.manufacturer,
            r.route.home,
            slug(r.manufacturer)
        ));
        for s in r.steps {
            if !s.contains("{order}") && !s.contains("imported by itself") {
                lines.push(format!("    {s}"));
            }
        }
    }
    lines
}

#[derive(Default)]
struct Run {
    report: InboxReport,
    parts: Vec<CatalogPart>,
}

/// Reads every file under `inbox`, checks the parts and merges them into
/// the catalog at `catalog_path` (default: [`user_catalog_path`]).
pub fn import_inbox(
    inbox: &Path,
    catalog_path: Option<&Path>,
    opts: &InboxOptions,
) -> Result<InboxReport, InboxError> {
    let catalog_path = match catalog_path {
        Some(p) => p.to_owned(),
        None => user_catalog_path().ok_or(InboxError::NoProfile)?,
    };
    let mut files = Vec::new();
    collect(inbox, &mut files)?;
    files.sort();

    let mut run = Run::default();
    for file in &files {
        read_file(inbox, file, opts, &mut run);
    }

    // Duplicates inside the run: the later file wins.
    let mut unique: BTreeMap<_, CatalogPart> = BTreeMap::new();
    for p in std::mem::take(&mut run.parts) {
        unique.insert(p.key(), p);
    }
    let parts: Vec<CatalogPart> = unique.into_values().collect();

    let mut catalog = if catalog_path.exists() {
        Catalog::load(&catalog_path)?
    } else {
        Catalog::new()
    };
    let preview = catalog.preview_merge(&parts);
    let mut report = run.report;
    report.dry_run = opts.dry_run;

    for p in &parts {
        let m = report.makers.entry(p.manufacturer.clone()).or_default();
        m.read += 1;
        let key = p.key();
        if preview.added.contains(&key) {
            m.added += 1;
        } else if preview.changed.contains(&key) {
            m.updated += 1;
        } else {
            m.unchanged += 1;
        }
        let flags = check(p);
        if p.dimensions.is_none() {
            m.no_dimensions += 1;
        }
        if p.class.etim.is_none() {
            m.no_etim += 1;
        }
        if p.terminals.is_empty() {
            m.no_terminals += 1;
        }
        match assess(p).grade() {
            Grade::Good => m.good += 1,
            Grade::Fair => m.fair += 1,
            Grade::Poor => m.poor += 1,
        }
        if !flags.is_empty() {
            m.flagged += 1;
            for f in flags {
                report
                    .warnings
                    .push(format!("{} {}: {f}", p.manufacturer, p.order_number));
            }
        }
    }

    if !opts.dry_run && !parts.is_empty() {
        if catalog_path.exists() {
            // One backup of the catalog as it was before this run.
            let _ = fs::copy(&catalog_path, catalog_path.with_extension("json.bak"));
        }
        for p in parts {
            catalog.upsert(p);
        }
        catalog.save(&catalog_path)?;
    } else {
        for p in parts {
            catalog.upsert(p);
        }
    }
    report.catalog_size = catalog.len();
    Ok(report)
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), InboxError> {
    let entries = fs::read_dir(dir).map_err(|source| InboxError::Io {
        path: dir.to_owned(),
        source,
    })?;
    for e in entries.flatten() {
        let path = e.path();
        if e.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        match e.file_type() {
            Ok(t) if t.is_dir() => collect(&path, out)?,
            Ok(t) if t.is_file() => out.push(path),
            _ => {}
        }
    }
    Ok(())
}

/// The recipe whose name or alias starts the given maker text
/// ("Phoenix Contact GmbH & Co. KG" -> Phoenix Contact).
fn recipe_by_name(text: &str) -> Option<&'static MakerRecipe> {
    let m = normalize_code(text);
    if m.len() < 3 {
        return None;
    }
    RECIPES.iter().find(|r| {
        std::iter::once(r.manufacturer)
            .chain(r.aliases.iter().copied())
            .map(normalize_code)
            .any(|n| n.len() >= 3 && m.starts_with(&n))
    })
}

fn read_file(inbox: &Path, file: &Path, opts: &InboxOptions, run: &mut Run) {
    let rel = file.strip_prefix(inbox).unwrap_or(file);
    let label = rel.display().to_string();
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    // Maker from the first folder (not `_wholesaler`), else from the file name.
    let folder = rel
        .components()
        .next()
        .filter(|_| rel.components().count() > 1)
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .filter(|f| !f.starts_with('_'));
    let hint_recipe = folder
        .as_deref()
        .map(|f| f.replace('-', " "))
        .and_then(|f| recipe_by_name(&f))
        .or_else(|| recipe_by_name_in(&name));
    let hint_name = match (&hint_recipe, &folder) {
        (Some(r), _) => r.manufacturer.to_owned(),
        (None, Some(f)) => f.replace('-', " "),
        (None, None) => String::new(),
    };

    let kind = FileKind::of_file_name(&name);
    let lower = name.to_ascii_lowercase();
    let parts: Result<Vec<(CatalogPart, &'static str)>, String> = match kind {
        FileKind::Bmecat => fs::read(file)
            .map_err(|e| e.to_string())
            .and_then(|b| bmecat(&String::from_utf8_lossy(&b), &hint_name)),
        FileKind::Csv => fs::read(file)
            .map_err(|e| e.to_string())
            .and_then(|b| csv(&decode_text(&b), &hint_name)),
        FileKind::Zip | FileKind::Edz => fs::read(file)
            .map_err(|e| e.to_string())
            .and_then(|b| read_zip(&b))
            .and_then(|z| {
                let mut all = Vec::new();
                for (member, text) in z.xml {
                    if looks_like_bmecat(&text) {
                        all.extend(bmecat(&text, &hint_name).map_err(|e| format!("{member}: {e}"))?);
                    }
                }
                if all.is_empty() {
                    Err("the ZIP holds no BMEcat catalog (a STEP or DXF ZIP belongs to the one-click Import in the app)".into())
                } else {
                    Ok(all)
                }
            }),
        FileKind::Step | FileKind::Dxf => {
            run.report.ignored.push(format!(
                "{label}: geometry file, import it with the part in the app"
            ));
            return;
        }
        FileKind::Other => {
            if lower.ends_with(".xlsx") || lower.ends_with(".xls") {
                run.report.ignored.push(format!(
                    "{label}: a spreadsheet; save it as CSV (File > Save As > CSV) and drop that"
                ));
            } else if !lower.starts_with("readme") && !lower.ends_with(".txt") {
                run.report.ignored.push(format!("{label}: not a catalog file"));
            }
            return;
        }
    };
    let parts = match parts {
        Ok(p) => p,
        Err(e) => {
            run.report.rejected.push(format!("{label}: {e}"));
            return;
        }
    };
    if parts.is_empty() {
        run.report.rejected.push(format!("{label}: no parts"));
        return;
    }

    // Group per recipe so each maker's rules read its own parts.
    let mut groups: BTreeMap<&'static str, (&'static MakerRecipe, Vec<CatalogPart>, &'static str)> =
        BTreeMap::new();
    for (p, source) in parts {
        let recipe = recipe_by_name(&p.manufacturer)
            .or(hint_recipe)
            .unwrap_or(&GENERIC);
        groups
            .entry(recipe.manufacturer)
            .or_insert_with(|| (recipe, Vec::new(), source))
            .1
            .push(p);
    }
    let mut counted: Vec<String> = Vec::new();
    for (_, (recipe, mut group, source)) in groups {
        let ctx = ApplyContext {
            source,
            file: &name,
            imported: &opts.today,
        };
        let rep = recipe.apply(&mut group, &ctx);
        for (feature, n) in rep.unmapped {
            run.report
                .notes
                .push(format!("{} \"{feature}\": {n} parts", maker_label(recipe)));
        }
        for w in rep.warnings.iter().take(5) {
            run.report.warnings.push(format!("{label}: {w}"));
        }
        for p in group {
            if !counted.contains(&p.manufacturer) {
                counted.push(p.manufacturer.clone());
                run.report
                    .makers
                    .entry(p.manufacturer.clone())
                    .or_default()
                    .files += 1;
            }
            run.parts.push(p);
        }
    }
}

fn maker_label(r: &MakerRecipe) -> &str {
    if r.manufacturer.is_empty() {
        "(other)"
    } else {
        r.manufacturer
    }
}

/// A recipe named in a file name (`Weidmueller_BMEcat_2026.xml`).
fn recipe_by_name_in(file_name: &str) -> Option<&'static MakerRecipe> {
    let f = normalize_code(file_name);
    RECIPES.iter().find(|r| {
        std::iter::once(r.manufacturer)
            .chain(r.aliases.iter().copied())
            .map(normalize_code)
            .any(|n| n.len() >= 4 && f.contains(&n))
    })
}

fn bmecat(text: &str, default_maker: &str) -> Result<Vec<(CatalogPart, &'static str)>, String> {
    import_bmecat(text, default_maker)
        .map(|v| v.into_iter().map(|p| (p, "import:bmecat")).collect())
        .map_err(|e| e.to_string())
}

fn csv(text: &str, default_maker: &str) -> Result<Vec<(CatalogPart, &'static str)>, String> {
    import_catalog(text, default_maker)
        .map(|r| r.parts.into_iter().map(|p| (p, "import:csv")).collect())
        .map_err(|e| e.to_string())
}

/// Problems with one part. Returned as text; none stops the import.
fn check(p: &CatalogPart) -> Vec<String> {
    let mut f = Vec::new();
    if p.manufacturer.trim().is_empty() {
        f.push("no manufacturer (put the file in a folder named after the maker)".into());
    }
    if p.description.trim().is_empty() {
        f.push("no description".into());
    }
    if let Some(d) = p.dimensions {
        let v = [d.width_mm, d.height_mm, d.depth_mm];
        if v.iter().any(|x| !x.is_finite() || *x < 0.0) || v.iter().all(|x| *x == 0.0) {
            f.push("dimensions are zero or negative".into());
        } else if v.iter().any(|x| *x > MAX_DIMENSION_MM) {
            f.push(format!(
                "a dimension is over {MAX_DIMENSION_MM} mm (unit mix-up?)"
            ));
        }
    }
    if let Some(e) = &p.class.etim
        && !is_etim_class(e)
    {
        f.push(format!("ETIM class \"{e}\" is not of the form EC000000"));
    }
    if let Some(poles) = pole_count(p)
        && !p.terminals.is_empty()
        && p.terminals.len() < poles
    {
        f.push(format!(
            "{} terminals listed for {poles} poles",
            p.terminals.len()
        ));
    }
    f
}

fn pole_count(p: &CatalogPart) -> Option<usize> {
    match p.data.get("poles")? {
        novo_core::Value::Int(n) if *n > 0 => Some(*n as usize),
        _ => None,
    }
}

fn is_etim_class(s: &str) -> bool {
    s.len() == 8 && s.starts_with("EC") && s[2..].bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bmecat_file(supplier: &str, pid: &str, poles: i32) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<BMECAT version="2005"><HEADER><SUPPLIER><SUPPLIER_NAME>{supplier}</SUPPLIER_NAME></SUPPLIER></HEADER>
<T_NEW_CATALOG><PRODUCT><SUPPLIER_PID>{pid}</SUPPLIER_PID>
<PRODUCT_DETAILS><DESCRIPTION_SHORT>Terminal block</DESCRIPTION_SHORT></PRODUCT_DETAILS>
<PRODUCT_FEATURES><REFERENCE_FEATURE_SYSTEM_NAME>ETIM-8.0</REFERENCE_FEATURE_SYSTEM_NAME>
<REFERENCE_FEATURE_GROUP_ID>EC000897</REFERENCE_FEATURE_GROUP_ID>
<FEATURE><FNAME>Poles</FNAME><FVALUE>{poles}</FVALUE></FEATURE></PRODUCT_FEATURES>
</PRODUCT></T_NEW_CATALOG></BMECAT>"#
        )
    }

    fn opts() -> InboxOptions {
        InboxOptions {
            today: "2026-10-16".into(),
            dry_run: false,
        }
    }

    #[test]
    fn slugs_match_recipe_names() {
        assert_eq!(slug("Phoenix Contact"), "phoenix-contact");
        assert_eq!(slug("Weidmüller"), "weidmuller");
        assert!(recipe_by_name("weidmuller").is_some());
        assert_eq!(
            recipe_by_name("Phoenix Contact GmbH & Co. KG")
                .unwrap()
                .manufacturer,
            "Phoenix Contact"
        );
        assert!(recipe_by_name("Acme").is_none());
    }

    #[test]
    fn imports_a_folder_and_a_rerun_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = dir.path().join("inbox");
        prepare_inbox(&inbox).unwrap();
        fs::write(
            inbox.join("phoenix-contact/cat.xml"),
            bmecat_file("Phoenix Contact GmbH &amp; Co. KG", "3044076", 2),
        )
        .unwrap();
        fs::write(
            inbox.join("wago/prices.csv"),
            "Order number;Description\n2002-1201;Terminal\n",
        )
        .unwrap();
        fs::write(inbox.join("wago/sheet.xlsx"), b"x").unwrap();
        fs::write(inbox.join("lapp/broken.xml"), "<a/>").unwrap();
        let cat = dir.path().join("parts.json");

        let r = import_inbox(&inbox, Some(&cat), &opts()).unwrap();
        assert_eq!(r.total_read(), 2, "{}", r.render());
        assert_eq!(r.total_added(), 2);
        assert_eq!(r.makers["Phoenix Contact"].added, 1);
        assert_eq!(r.makers["WAGO"].added, 1);
        assert_eq!(r.makers["Phoenix Contact"].no_etim, 0);
        assert_eq!(r.rejected.len(), 1);
        assert_eq!(r.ignored.len(), 1);
        assert_eq!(Catalog::load(&cat).unwrap().len(), 2);

        let again = import_inbox(&inbox, Some(&cat), &opts()).unwrap();
        assert_eq!(again.total_added(), 0);
        assert_eq!(again.makers["WAGO"].unchanged, 1);
        assert!(again.render().contains("Total: 2 parts read, 0 new"));
    }

    #[test]
    fn dry_run_writes_nothing_and_unknown_folder_names_the_maker() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = dir.path().join("inbox");
        fs::create_dir_all(inbox.join("Hager")).unwrap();
        fs::write(
            inbox.join("Hager/h.csv"),
            "Order number;Description\nMBN110;MCB 1P 10A\n",
        )
        .unwrap();
        let cat = dir.path().join("parts.json");
        let mut o = opts();
        o.dry_run = true;
        let r = import_inbox(&inbox, Some(&cat), &o).unwrap();
        assert_eq!(r.makers["Hager"].added, 1);
        assert!(!cat.exists());
    }

    #[test]
    fn checks_flag_bad_parts_without_dropping_them() {
        let mut p = CatalogPart::new("X", "1");
        p.class.etim = Some("EC12".into());
        p.dimensions = Some(crate::catalog::Dimensions {
            width_mm: 12000.0,
            height_mm: 1.0,
            depth_mm: 1.0,
        });
        let f = check(&p);
        assert!(f.iter().any(|s| s.contains("ETIM")));
        assert!(f.iter().any(|s| s.contains("unit mix-up")));
        assert!(f.iter().any(|s| s.contains("description")));
    }

    #[test]
    fn zip_with_bmecat_is_read_and_second_run_updates_a_changed_part() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let inbox = dir.path().join("inbox");
        fs::create_dir_all(inbox.join("siemens")).unwrap();
        let zip_path = inbox.join("siemens/export.zip");
        let mut w = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
        w.start_file("c.xml", zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(bmecat_file("Siemens AG", "3RT2016", 3).as_bytes())
            .unwrap();
        w.finish().unwrap();
        let cat = dir.path().join("parts.json");
        let r = import_inbox(&inbox, Some(&cat), &opts()).unwrap();
        assert_eq!(r.makers["Siemens"].added, 1, "{}", r.render());

        fs::remove_file(&zip_path).unwrap();
        fs::write(
            inbox.join("siemens/c.xml"),
            bmecat_file("Siemens AG", "3RT2016", 4),
        )
        .unwrap();
        let r = import_inbox(&inbox, Some(&cat), &opts()).unwrap();
        assert_eq!(r.makers["Siemens"].updated, 1);
        assert!(cat.with_extension("json.bak").exists());
    }

    #[test]
    fn guide_lists_every_maker_and_flags_logins() {
        let g = maker_guide(&["WAGO".into()]);
        assert!(
            g.iter()
                .any(|l| l.starts_with("WAGO (files found; LOGIN NEEDED)"))
        );
        assert!(g.iter().any(|l| l.starts_with("Rittal")));
    }
}

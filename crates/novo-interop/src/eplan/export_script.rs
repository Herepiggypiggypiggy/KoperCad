//! The user-run "Export from EPLAN" helper. See `docs/design/interop.md`.
//!
//! KoperCAD cannot read an EPLAN project file directly, but EPLAN itself can
//! write its project exchange format (PXF, plain XML) through its own
//! `export` action. This module hands the person a small EPLAN script that
//! runs that action inside their own EPLAN, with their own licence, no
//! administrator rights and no API licence, plus step-by-step instructions
//! in English and Dutch and a synthetic self-test file.
//!
//! Everything here is written from scratch. The script and the self-test file
//! contain no EPLAN content and no customer data. The functions return text
//! and values; the caller owns the disk.

use thiserror::Error;

use super::xml;
use crate::report::Report;

/// Version of the shipped script. Raise it whenever the script or the
/// manifest it writes changes, and the matching `ScriptVersion` constant in
/// the `.cs` file with it (a test keeps the two equal).
pub const SCRIPT_VERSION: u32 = 1;

/// File name of the script.
pub const SCRIPT_FILE: &str = "KoperCAD_ExportFromEPLAN.cs";
/// Manifest the script writes next to `project.epj`.
pub const MANIFEST_FILE: &str = "kopercad-export.xml";
/// Name of the exchange file the script asks EPLAN for.
pub const EXCHANGE_FILE: &str = "project.epj";
/// Name of the synthetic self-test file.
pub const SELF_TEST_FILE: &str = "kopercad-selftest.epj";

const SCRIPT: &str = include_str!("../../assets/eplan/KoperCAD_ExportFromEPLAN.cs");

/// Language of the instructions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    English,
    Dutch,
}

impl Lang {
    /// `en`, `nl` (also `dutch`, `nederlands`), case insensitive.
    pub fn parse(s: &str) -> Option<Lang> {
        match s.trim().to_ascii_lowercase().as_str() {
            "en" | "english" => Some(Lang::English),
            "nl" | "dutch" | "nederlands" => Some(Lang::Dutch),
            _ => None,
        }
    }
}

/// One file of the bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleFile {
    pub name: &'static str,
    pub text: String,
}

/// The script source.
pub fn script_source() -> &'static str {
    SCRIPT
}

/// Everything the person gets: the script, the instructions in both
/// languages and the synthetic self-test file.
pub fn bundle() -> Vec<BundleFile> {
    vec![
        BundleFile {
            name: SCRIPT_FILE,
            text: SCRIPT.to_string(),
        },
        BundleFile {
            name: "README-EN.txt",
            text: instructions(Lang::English),
        },
        BundleFile {
            name: "README-NL.txt",
            text: instructions(Lang::Dutch),
        },
        BundleFile {
            name: SELF_TEST_FILE,
            text: self_test_pxf(),
        },
    ]
}

/// Step-by-step instructions as plain text.
pub fn instructions(lang: Lang) -> String {
    let v = SCRIPT_VERSION;
    match lang {
        Lang::English => format!(
            "Export from EPLAN (KoperCAD script version {v})\n\
=================================================\n\
\n\
What it does: runs EPLAN's own project export (PXF, plain XML) from inside\n\
your EPLAN and saves it where KoperCAD can import it. It does not change\n\
your project and sends nothing anywhere. It needs no administrator rights and\n\
no EPLAN API licence; everything is written under your Documents folder.\n\
\n\
Before you start\n\
  - EPLAN Electric P8 2024 writes the format KoperCAD was built against\n\
    (PXF 1.23). Older projects: open and save them once in EPLAN 2024 first.\n\
  - Close nothing: the project you want to export stays open in EPLAN.\n\
\n\
Steps\n\
  1. In KoperCAD choose File > Import > Get the EPLAN export script and pick\n\
     a folder. KoperCAD writes {SCRIPT_FILE}, these instructions and a\n\
     self-test file there.\n\
  2. In EPLAN open the Utilities menu > Scripts > Load, and choose\n\
     {SCRIPT_FILE}. (Menu names differ a little per EPLAN version and language.)\n\
  3. Optional self-test: Utilities > Scripts > Run action, type\n\
     KoperCadSelfTest. It writes a small test folder and a message; it touches\n\
     no project.\n\
  4. Open the project you want to export. Utilities > Scripts, choose\n\
     KoperCadExportFromEplan (the script name) to run it.\n\
  5. Wait. Big projects take minutes; EPLAN may look busy. A message shows the\n\
     folder when it is done: Documents\\KoperCAD\\EPLAN-export\\<project>-<date>.\n\
  6. In KoperCAD choose File > Import > EPLAN project and pick that folder\n\
     (that import arrives with the EPLAN project import itself). KoperCAD\n\
     checks the script version first.\n\
\n\
Check the pipeline without EPLAN: import {SELF_TEST_FILE} (a synthetic file\n\
made by KoperCAD, not from EPLAN). Command line: kopercad-cli eplan check <folder>.\n\
\n\
If something fails\n\
  - No project.epj: make sure a project is open, or set the Windows\n\
    environment variable KOPERCAD_EPLAN_PROJECT to the project's .elk path.\n\
  - EPLAN cannot load the script: your EPLAN version may use another script\n\
    interface; tell KoperVonk the version and the message EPLAN shows.\n\
  - KoperCAD says the script is older than the program: run step 1 again and\n\
    load the new script.\n\
\n\
The script contains no EPLAN content and no customer data. It is yours to read:\n\
open {SCRIPT_FILE} in any text editor.\n"
        ),
        Lang::Dutch => format!(
            "Exporteren uit EPLAN (KoperCAD-script versie {v})\n\
=================================================\n\
\n\
Wat het doet: voert de eigen projectexport van EPLAN (PXF, gewone XML) uit\n\
binnen jouw EPLAN en bewaart het resultaat waar KoperCAD het kan importeren.\n\
Het wijzigt je project niet en verstuurt niets. Je hebt geen beheerdersrechten\n\
en geen EPLAN API-licentie nodig; alles komt in je map Documenten.\n\
\n\
Voor je begint\n\
  - EPLAN Electric P8 2024 schrijft het formaat waarop KoperCAD is gebouwd\n\
    (PXF 1.23). Oudere projecten: open en bewaar ze eerst een keer in\n\
    EPLAN 2024.\n\
  - Laat het project dat je wilt exporteren gewoon open in EPLAN staan.\n\
\n\
Stappen\n\
  1. Kies in KoperCAD File > Import > Get the EPLAN export script en\n\
     kies een map. KoperCAD schrijft daar {SCRIPT_FILE}, deze handleiding en\n\
     een zelftestbestand.\n\
  2. Open in EPLAN het menu Extra (Utilities) > Scripts > Laden en kies\n\
     {SCRIPT_FILE}. (Menunamen verschillen iets per EPLAN-versie en taal.)\n\
  3. Optionele zelftest: Scripts > Actie uitvoeren, typ KoperCadSelfTest. Het\n\
     schrijft een kleine testmap en toont een melding; er wordt geen project\n\
     aangeraakt.\n\
  4. Open het project dat je wilt exporteren. Kies bij Scripts\n\
     KoperCadExportFromEplan (de scriptnaam) om het uit te voeren.\n\
  5. Wacht. Grote projecten duren minuten; EPLAN kan druk lijken. Een melding\n\
     toont de map als het klaar is: Documenten\\KoperCAD\\EPLAN-export\\<project>-<datum>.\n\
  6. Kies in KoperCAD File > Import > EPLAN project en kies die map (die\n\
     import komt met de EPLAN-import zelf). KoperCAD controleert eerst de\n\
     scriptversie.\n\
\n\
De keten testen zonder EPLAN: importeer {SELF_TEST_FILE} (een synthetisch\n\
bestand van KoperCAD, niet uit EPLAN). Opdrachtregel: kopercad-cli eplan check <map>.\n\
\n\
Als het misgaat\n\
  - Geen project.epj: zorg dat een project open staat, of zet de Windows-\n\
    omgevingsvariabele KOPERCAD_EPLAN_PROJECT op het .elk-pad van het project.\n\
  - EPLAN kan het script niet laden: je EPLAN-versie gebruikt misschien een\n\
    andere scriptinterface; geef de versie en de melding van EPLAN door aan\n\
    KoperVonk.\n\
  - KoperCAD meldt dat het script ouder is dan het programma: herhaal stap 1\n\
    en laad het nieuwe script.\n\
\n\
Het script bevat geen EPLAN-inhoud en geen klantgegevens. Je mag het lezen:\n\
open {SCRIPT_FILE} in een tekstverwerker.\n"
        ),
    }
}

/// A tiny synthetic exchange file in the PXF layout (two pages, two devices,
/// one text). Made by KoperCAD; it is not an EPLAN file and says so.
pub fn self_test_pxf() -> String {
    "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
<!-- KoperCAD synthetic self-test file. Not exported by EPLAN. -->\n\
<EplanPxfRoot Type=\"Project\" Version=\"2024\" PxfVersion=\"1.23\" SelfTest=\"1\" NumMainObjects=\"5\">\n\
  <O4 A1=\"4/1\" A1101=\"1\"><P49 P1100=\"PLANT\" P1200=\"CAB1\" P11012=\"1\"/><P11 P11011=\"en_US@Main circuit;\"/></O4>\n\
  <O4 A1=\"4/2\" A1101=\"2\"><P49 P1100=\"PLANT\" P1200=\"CAB1\" P11012=\"2\"/><P11 P11011=\"en_US@Control;\"/></O4>\n\
  <O17 A1=\"17/3\"><P150 P1100=\"PLANT\" P1200=\"CAB1\" P20013=\"Q\" P20014=\"1\"/><P11 P20027=\"Circuit breaker\" P20100_1=\"MAKER.TYPE1\"/></O17>\n\
  <O17 A1=\"17/4\"><P150 P1100=\"PLANT\" P1200=\"CAB1\" P20013=\"K\" P20014=\"1\"/><P11 P20027=\"Contactor\" P20100_1=\"MAKER.TYPE2\"/></O17>\n\
  <O30 A1=\"30/5\"><P11 P11011=\"en_US@Self-test text;\"/></O30>\n\
</EplanPxfRoot>\n"
        .to_string()
}

/// What the head of a PXF file says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PxfHead {
    pub pxf_version: String,
    pub kind: String,
    pub main_objects: Option<usize>,
    pub self_test: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExchangeError {
    #[error("this is not an EPLAN exchange file (no EplanPxfRoot element)")]
    NotPxf,
    #[error("the manifest is not a KoperCAD export manifest: {0}")]
    BadManifest(String),
}

/// Reads the `EplanPxfRoot` start tag from the first bytes of a file. Pass
/// the first 64 KB of a large project; nothing else is parsed.
pub fn pxf_head(head: &str) -> Result<PxfHead, ExchangeError> {
    let start = head.find("<EplanPxfRoot").ok_or(ExchangeError::NotPxf)?;
    let tag = &head[start..];
    let tag = &tag[..tag.find('>').unwrap_or(tag.len())];
    let attr = |name: &str| -> Option<String> {
        let key = format!("{name}=\"");
        let at = tag.find(&format!(" {key}"))? + 1 + key.len();
        let end = tag[at..].find('"')?;
        Some(tag[at..at + end].to_string())
    };
    Ok(PxfHead {
        pxf_version: attr("PxfVersion").unwrap_or_default(),
        kind: attr("Type").unwrap_or_default(),
        main_objects: attr("NumMainObjects").and_then(|n| n.parse().ok()),
        self_test: attr("SelfTest").is_some_and(|v| v == "1"),
    })
}

/// What the script wrote into `kopercad-export.xml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub script_version: u32,
    pub eplan_version: String,
    pub project: String,
    pub created: String,
    pub pxf_file: String,
    pub self_test: bool,
}

pub fn read_manifest(text: &str) -> Result<Manifest, ExchangeError> {
    let root = xml::parse(text).map_err(|e| ExchangeError::BadManifest(e.to_string()))?;
    if root.name != "KoperCadEplanExport" {
        return Err(ExchangeError::BadManifest(format!(
            "the root element is <{}>",
            root.name
        )));
    }
    let script_version = root
        .attr("scriptVersion")
        .and_then(|v| v.trim().parse().ok())
        .ok_or_else(|| ExchangeError::BadManifest("scriptVersion is missing".into()))?;
    let text_of = |k: &str| root.attr(k).unwrap_or_default().to_string();
    Ok(Manifest {
        script_version,
        eplan_version: text_of("eplanVersion"),
        project: text_of("project"),
        created: text_of("created"),
        pxf_file: text_of("pxfFile"),
        self_test: root.attr("selfTest") == Some("true"),
    })
}

/// How the script that made an export compares with this KoperCAD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionCheck {
    Current,
    /// The person's script is older: they should get the script again.
    ScriptOlder {
        found: u32,
        current: u32,
    },
    /// The export came from a newer script than this KoperCAD knows.
    ScriptNewer {
        found: u32,
        current: u32,
    },
}

impl VersionCheck {
    pub fn of(script_version: u32) -> VersionCheck {
        use std::cmp::Ordering::*;
        match script_version.cmp(&SCRIPT_VERSION) {
            Equal => VersionCheck::Current,
            Less => VersionCheck::ScriptOlder {
                found: script_version,
                current: SCRIPT_VERSION,
            },
            Greater => VersionCheck::ScriptNewer {
                found: script_version,
                current: SCRIPT_VERSION,
            },
        }
    }

    pub fn is_current(self) -> bool {
        self == VersionCheck::Current
    }

    /// One sentence for the person.
    pub fn message(self) -> String {
        match self {
            VersionCheck::Current => "The export script is up to date.".into(),
            VersionCheck::ScriptOlder { found, current } => format!(
                "The export was made by script version {found}; this KoperCAD ships version {current}. Get the script again (File > Import > Get the EPLAN export script) and load it in EPLAN."
            ),
            VersionCheck::ScriptNewer { found, current } => format!(
                "The export was made by script version {found}, newer than the {current} this KoperCAD knows. Update KoperCAD."
            ),
        }
    }
}

/// Checks an exchange folder from its two files: the manifest (if present)
/// and the first bytes of the exchange file. Returns the version verdict and
/// a report of what was found; the caller decides whether to import.
pub fn check_exchange(
    manifest: Option<&str>,
    pxf_start: &str,
) -> Result<(VersionCheck, Report), ExchangeError> {
    let mut report = Report::default();
    let head = pxf_head(pxf_start)?;
    report.moved("exchange file", 1);
    if head.pxf_version.is_empty() {
        report.note("the file names no PxfVersion; it may come from another EPLAN version");
    } else if head.pxf_version != "1.23" {
        report.note(format!(
            "PXF version {} (KoperCAD was built against 1.23)",
            head.pxf_version
        ));
    }
    if head.self_test {
        report.note("this is the KoperCAD self-test file, not a real project");
    }
    let check = match manifest {
        Some(m) => {
            let m = read_manifest(m)?;
            if m.self_test {
                report.note("the manifest is from the script self-test");
            }
            VersionCheck::of(m.script_version)
        }
        None => {
            report.note("no kopercad-export.xml next to the file; the script version is unknown");
            VersionCheck::Current
        }
    };
    Ok((check, report))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_version_matches_the_cs_file() {
        let want = format!("const int ScriptVersion = {SCRIPT_VERSION};");
        assert!(
            SCRIPT.contains(&want),
            "update ScriptVersion in the .cs file"
        );
    }

    #[test]
    fn script_is_plain_ascii_and_has_no_secrets() {
        assert!(SCRIPT.is_ascii());
        let low = SCRIPT.to_ascii_lowercase();
        for bad in ["password", "http://", "https://", "webclient", "socket"] {
            assert!(!low.contains(bad), "{bad}");
        }
        assert!(SCRIPT.contains("[Start]"));
        assert!(SCRIPT.contains("PXFPROJECT"));
    }

    #[test]
    fn instructions_name_the_steps_in_both_languages() {
        for lang in [Lang::English, Lang::Dutch] {
            let t = instructions(lang);
            assert!(t.contains(SCRIPT_FILE));
            assert!(t.contains("KoperCadSelfTest"));
            assert!(t.contains("KOPERCAD_EPLAN_PROJECT"));
            assert!(t.contains("1.23"));
        }
        assert!(instructions(Lang::Dutch).contains("Stappen"));
        assert_eq!(Lang::parse("NL"), Some(Lang::Dutch));
        assert_eq!(Lang::parse("fr"), None);
    }

    #[test]
    fn self_test_file_is_well_formed_and_consistent() {
        let text = self_test_pxf();
        let root = xml::parse(&text).unwrap();
        assert_eq!(root.name, "EplanPxfRoot");
        let head = pxf_head(&text).unwrap();
        assert_eq!(head.pxf_version, "1.23");
        assert!(head.self_test);
        assert_eq!(head.main_objects, Some(root.children.len()));
        let mut ids: Vec<_> = root.children.iter().filter_map(|c| c.attr("A1")).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n, "ids are unique");
        assert!(!text.contains("@;"), "no empty multi-language text");
    }

    #[test]
    fn bundle_has_script_readmes_and_self_test() {
        let names: Vec<_> = bundle().iter().map(|f| f.name).collect();
        assert_eq!(
            names,
            [
                SCRIPT_FILE,
                "README-EN.txt",
                "README-NL.txt",
                SELF_TEST_FILE
            ]
        );
    }

    #[test]
    fn manifest_round_trip_and_version_check() {
        let m = format!(
            "<?xml version=\"1.0\"?><KoperCadEplanExport scriptVersion=\"{SCRIPT_VERSION}\" eplanVersion=\"2024.0.3\" project=\"A&amp;B\" created=\"2026-10-16T08:00:00Z\" pxfFile=\"project.epj\" selfTest=\"false\"/>"
        );
        let got = read_manifest(&m).unwrap();
        assert_eq!(got.project, "A&B");
        assert!(!got.self_test);
        let (check, report) = check_exchange(Some(&m), &self_test_pxf()).unwrap();
        assert!(check.is_current());
        assert_eq!(report.count_of("exchange file"), 1);
    }

    #[test]
    fn older_and_newer_scripts_are_told_apart() {
        assert_eq!(VersionCheck::of(SCRIPT_VERSION), VersionCheck::Current);
        let old = VersionCheck::of(0);
        assert!(matches!(old, VersionCheck::ScriptOlder { found: 0, .. }));
        assert!(old.message().contains("Get the script again"));
        let new = VersionCheck::of(SCRIPT_VERSION + 1);
        assert!(new.message().contains("Update KoperCAD"));
    }

    #[test]
    fn rejects_other_files() {
        assert_eq!(pxf_head("<root/>"), Err(ExchangeError::NotPxf));
        assert!(matches!(
            read_manifest("<other/>"),
            Err(ExchangeError::BadManifest(_))
        ));
        assert!(matches!(
            read_manifest("<KoperCadEplanExport/>"),
            Err(ExchangeError::BadManifest(_))
        ));
    }
}

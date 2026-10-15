//! Dutch aliases for command words (DESIGN.md section 3, "Language").
//!
//! Every command has its English label, keywords and verbs in
//! [`crate::commands`]; this table adds the Dutch words. Both languages
//! always match, whatever the UI language, so a colleague's habit still
//! works: `hernummer pagina` is `renumber page`, `alles accepteren` is
//! `accept all`. Device tags, IEC letter codes, units and potential names
//! are never translated, so nothing here touches them.

/// The Dutch words for one command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aliases {
    /// A command id from [`crate::commands::COMMANDS`].
    pub command: &'static str,
    /// Verb phrases, like the command's English `verbs`.
    pub verbs: &'static [&'static str],
    /// Search words, like the command's English label and keywords.
    pub words: &'static [&'static str],
}

const fn nl(
    command: &'static str,
    verbs: &'static [&'static str],
    words: &'static [&'static str],
) -> Aliases {
    Aliases {
        command,
        verbs,
        words,
    }
}

/// Dutch words per command.
pub const DUTCH: &[Aliases] = &[
    // Project
    nl("file.new", &[], &["nieuw project", "nieuw"]),
    nl("file.open", &[], &["project openen", "openen"]),
    nl("file.example", &[], &["voorbeeld openen", "voorbeeld"]),
    nl("file.practice", &[], &["snelle toetsen", "oefenen"]),
    nl(
        "help.walkthrough",
        &[],
        &["rondleiding", "snelle toetsen uitleg"],
    ),
    nl(
        "help.learn",
        &["leren", "les"],
        &["leren", "training", "cursus", "uitleg"],
    ),
    nl("learn.next", &[], &["les volgende stap", "les verder"]),
    nl("learn.back", &[], &["les vorige stap", "les terug"]),
    nl("learn.restart", &[], &["les opnieuw", "les herstarten"]),
    nl("learn.close", &[], &["les sluiten"]),
    nl("file.save", &[], &["opslaan", "bewaren"]),
    nl("file.save_as", &[], &["opslaan als"]),
    nl("file.close", &[], &["project sluiten", "sluiten"]),
    // Not a bare "pagina": `pagina 2` goes to page 2.
    nl(
        "page.new",
        &["nieuwe pagina", "pagina toevoegen"],
        &["pagina", "blad"],
    ),
    nl("project.rules", &["regels"], &["nummering", "instellingen"]),
    nl(
        "app.preferences",
        &["voorkeuren"],
        &["instellingen", "opties"],
    ),
    nl(
        "parts.browser",
        &["onderdelen"],
        &["artikelen", "onderdeel"],
    ),
    nl("project.standard", &["tekennorm"], &["norm", "standaard"]),
    nl(
        "project.options",
        &["projectopties", "varianten"],
        &["opties van het project", "uitvoeringen", "machineopties"],
    ),
    nl(
        "project.variant",
        &["variant"],
        &["uitvoering", "configuratie"],
    ),
    nl(
        "project.generate",
        &["genereer"],
        &["genereren", "specificatielijst", "uit excel"],
    ),
    nl("terminal.toggle_pe", &["pe-klem"], &["aardklem", "aarde"]),
    nl(
        "revision.issue",
        &["revisie"],
        &["uitgeven", "versie", "vrijgeven"],
    ),
    nl(
        "revision.compare",
        &["vergelijk"],
        &["verschil", "revisies vergelijken"],
    ),
    nl("file.quit", &[], &["afsluiten", "stoppen"]),
    nl("help.about", &[], &["over", "versie"]),
    // Draw
    nl(
        "tool.repeat",
        &[],
        &["herhaal laatste", "herhalen", "opnieuw"],
    ),
    nl("tool.place", &[], &["plaatsen", "invoegen", "component"]),
    nl("tool.connect", &[], &["verbinden", "draad"]),
    nl(
        "wire.mode",
        &["bedradingsmodus"],
        &["modus", "hoeken", "lijnen", "tekenen"],
    ),
    nl("tool.cable", &["kabel"], &["kabel tekenen", "aders"]),
    nl(
        "wire.end_on_part",
        &[],
        &[
            "draad op component laten eindigen",
            "afsluiten op component",
        ],
    ),
    nl(
        "wire.next_pin",
        &[],
        &["volgend aansluitpunt", "volgende pin"],
    ),
    nl("edit.rotate", &[], &["draaien", "roteren"]),
    nl("edit.mirror", &[], &["spiegelen"]),
    nl(
        "proposal.accept",
        &["accepteren", "accepteer"],
        &["voorstel accepteren"],
    ),
    nl(
        "proposal.accept_all",
        &["alles accepteren", "accepteer alles"],
        &["alle voorstellen accepteren"],
    ),
    nl("proposal.next", &[], &["volgend voorstel"]),
    nl("draw.tidy", &["opruimen", "netjes"], &["routes opruimen"]),
    // Edit
    nl("edit.undo", &[], &["ongedaan maken", "terugdraaien"]),
    nl("edit.redo", &[], &["opnieuw uitvoeren"]),
    nl("edit.copy", &[], &["kopiëren", "kopieren"]),
    nl("edit.cut", &[], &["knippen"]),
    nl("edit.paste", &[], &["plakken"]),
    nl("edit.paste_keep_tags", &[], &["plakken met originele tags"]),
    nl("edit.duplicate", &[], &["dupliceren", "kopie"]),
    nl("macro.save", &["macro opslaan"], &["opslaan als macro"]),
    nl(
        "macro.place",
        &["macro plaatsen"],
        &["plaats macro", "macro"],
    ),
    nl("edit.select_all", &[], &["alles selecteren"]),
    nl("edit.delete", &[], &["verwijderen", "wissen"]),
    nl("edit.rename", &[], &["hernoemen", "tag wijzigen"]),
    nl(
        "edit.renumber",
        &["hernummeren", "hernummer"],
        &["nummering", "apparaten hernummeren"],
    ),
    nl(
        "plc.add_card",
        &["kaart toevoegen"],
        &["plc kaart", "plc module"],
    ),
    nl(
        "plc.to_plc",
        &["naar plc"],
        &["signaal naar plc", "ingang uitgang"],
    ),
    nl(
        "plc.renumber_io",
        &["hernummer io"],
        &["adressen hernummeren"],
    ),
    nl(
        "xref.link",
        &["koppel", "koppelen"],
        &["contact aan spoel koppelen", "kruisverwijzing koppelen"],
    ),
    nl("edit.escape", &[], &["annuleren"]),
    // Navigate
    nl("nav.partner", &[], &["naar partner", "kruisverwijzing"]),
    nl(
        "nav.all_references",
        &[],
        &["alle verwijzingen", "waar gebruikt"],
    ),
    nl("nav.back", &[], &["terug"]),
    nl("nav.forward", &[], &["vooruit"]),
    nl("nav.previous_page", &[], &["vorige pagina"]),
    nl("nav.next_page", &[], &["volgende pagina"]),
    nl("nav.changes", &["wijzigingen"], &["gewijzigd sinds export"]),
    // View
    nl("view.command_palette", &[], &["opdrachtveld", "zoeken"]),
    nl(
        "view.command_field_here",
        &[],
        &["opdrachtveld bij de muis"],
    ),
    nl("view.fit", &[], &["passend zoomen", "alles tonen"]),
    nl("view.schematic", &[], &["schema"]),
    nl("view.cabinet", &[], &["kast", "kastindeling"]),
    nl(
        "interruption.renumber",
        &[],
        &["onderbrekingspunten nummeren"],
    ),
    nl("wires.number", &[], &["draden nummeren", "draadnummers"]),
    nl(
        "wires.numbering",
        &["draadnummering"],
        &["draadnummers instellen", "nummerschema"],
    ),
    nl(
        "wires.renumber",
        &["hernummer draden"],
        &["draden opnieuw nummeren"],
    ),
    nl(
        "wires.check_numbers",
        &[],
        &["draadnummers controleren", "dubbele draadnummers"],
    ),
    nl(
        "wires.set_number",
        &["nummer draad"],
        &["draadnummer typen", "draadnummer vastzetten"],
    ),
    nl("wires.unlock_number", &[], &["draadnummer ontgrendelen"]),
    nl("terminal.reset_pe", &["herstel"], &["aarde", "klemmen"]),
    nl("terminal.connect", &["verbind"], &["klemmen", "brug"]),
    nl("terminal.disconnect", &["ontkoppel"], &["klemmen", "brug"]),
    nl(
        "terminal.strip_up",
        &["hoger"],
        &["klemmenstrook", "nummer"],
    ),
    nl(
        "terminal.strip_down",
        &["lager"],
        &["klemmenstrook", "nummer"],
    ),
    nl(
        "terminal.strip_lowest",
        &["laagste"],
        &["klemmenstrook", "nummer"],
    ),
    nl("view.terminals_drawer", &[], &["klemmen", "klemmenstroken"]),
    nl("view.cables_drawer", &[], &["kabels", "aders"]),
    nl("view.devices_drawer", &[], &["apparaten", "onderdelen"]),
    nl("view.focus", &[], &["focusmodus"]),
    nl(
        "view.pin_outline",
        &[],
        &["overzicht vastzetten", "paginaoverzicht"],
    ),
    nl("view.full_screen", &[], &["volledig scherm"]),
    nl("view.cabinet_table", &[], &["kast als tabel", "tabel"]),
    nl(
        "heat.toggle",
        &[],
        &["warmte tonen", "temperatuur", "warmtebeeld"],
    ),
    nl("heat.airflow", &[], &["luchtstroom", "luchtstroming"]),
    nl("heat.contours", &[], &["isothermen", "warmtelijnen"]),
    nl(
        "cabinet.size-enclosure",
        &[],
        &["kast dimensioneren", "kastmaat", "koeling", "warmte"],
    ),
    nl(
        "cabinet.enclosures",
        &[],
        &["kasten", "kastenlijst", "behuizingen"],
    ),
    nl(
        "simulate.motor-start",
        &[],
        &[
            "motor starten",
            "ster-driehoek",
            "aanloopstroom",
            "zachtstarter",
        ],
    ),
    nl("view.changes", &["wijzigingen"], &["vergelijk", "verschil"]),
    nl(
        "view.commissioning",
        &["inbedrijfstelling"],
        &["uitvoeren", "afvinken", "restpunten"],
    ),
    nl(
        "sim.live",
        &[],
        &["live potentialen tonen", "contacten sluiten", "spanning"],
    ),
    nl(
        "sim.faults",
        &["storingen"],
        &[
            "kortsluiting",
            "beveiliging",
            "selectiviteit",
            "uitschakeltijd",
        ],
    ),
    nl(
        "view.toggle_marks",
        &[],
        &["markeringen tonen of verbergen"],
    ),
    nl(
        "view.override_marks",
        &[],
        &["handmatige wijzigingen tonen"],
    ),
    nl("view.shortcuts", &[], &["sneltoetsen", "toetsen"]),
    // Check
    nl("check.problems", &["problemen", "fouten"], &["controles"]),
    nl("check.next_problem", &[], &["volgend probleem"]),
    nl("check.previous_problem", &[], &["vorig probleem"]),
    nl(
        "file.import_dxf_background",
        &["importeer dxf"],
        &["achtergrond", "onderlegger", "tekening"],
    ),
    nl(
        "view.background",
        &[],
        &["achtergrond tonen", "achtergrond verbergen", "dxf"],
    ),
    nl(
        "view.background_place",
        &[],
        &["achtergrond verplaatsen", "achtergrond schalen", "dxf"],
    ),
    nl(
        "file.remove_background",
        &[],
        &["achtergrond verwijderen", "dxf"],
    ),
    nl(
        "export.dxf_page",
        &["exporteer dxf"],
        &["cad", "autocad", "pagina"],
    ),
    nl("export.dxf_all", &[], &["alle paginas", "cad", "autocad"]),
    // Export
    nl(
        "export.pdf",
        &["exporteer pdf", "pdf exporteren"],
        &["afdrukken"],
    ),
    nl("export.bom", &["exporteer stuklijst"], &["onderdelenlijst"]),
    nl(
        "export.commissioning",
        &["exporteer inbedrijfstelling", "inbedrijfstellingsrapport"],
        &["restpuntenlijst", "oplevering"],
    ),
    nl(
        "export.plate",
        &["exporteer montageplaat", "montageplaat exporteren"],
        &["boortabel", "boren"],
    ),
    nl(
        "export.reports",
        &[],
        &["alle rapporten", "excel exporteren"],
    ),
    nl("view.reports", &[], &["rapporten", "lijsten"]),
    nl("report.toc", &["inhoud", "inhoudsopgave"], &["paginalijst"]),
    nl(
        "report.bom",
        &["stuklijst", "materiaallijst"],
        &["onderdelenlijst"],
    ),
    nl("report.devices", &["apparatenlijst"], &["apparaten"]),
    nl("report.wires", &["draadlijst"], &["draden", "aders"]),
    nl("report.terminals", &["klemmenlijst"], &["klemmenstrook"]),
    nl("report.cables", &["kabellijst"], &["kabels"]),
    nl(
        "export.sheets",
        &[
            "exporteer spreadsheet",
            "exporteer excel",
            "excel exporteren",
        ],
        &["excel"],
    ),
    nl(
        "export.assembly",
        &["exporteer montagelijst", "montagelijst"],
        &["werkplaats", "boren"],
    ),
    nl("export.sheets_csv", &["exporteer csv"], &["csv"]),
    nl(
        "export.variant_pdf",
        &["exporteer variant"],
        &["pdf van een variant", "uitvoering exporteren"],
    ),
    nl(
        "export.spec",
        &["exporteer specificatie"],
        &["specificatielijst exporteren"],
    ),
    nl(
        "plc.import_tags",
        &["importeer plc", "importeer tags"],
        &["plc", "tags", "inlezen"],
    ),
    nl(
        "export.plc_csv",
        &["exporteer plc", "exporteer io"],
        &["plc", "io lijst"],
    ),
    nl(
        "export.plc_plcopen",
        &["exporteer plcopen", "exporteer codesys"],
        &["plcopen", "codesys"],
    ),
    nl(
        "export.plc_tia",
        &["exporteer tia", "exporteer siemens"],
        &["tia", "siemens"],
    ),
    nl(
        "export.plc_studio5000",
        &["exporteer studio 5000", "exporteer rockwell"],
        &["rockwell", "studio 5000"],
    ),
    nl(
        "export.plc_address_plan",
        &["exporteer adresplan"],
        &["adresplan"],
    ),
];

/// Dutch command arguments and the English chip each one stands for.
pub const DUTCH_ARGUMENTS: &[(&str, &str)] = &[
    ("pagina", "page"),
    ("hoeken", "corners"),
    ("lijnen", "lines"),
    ("selectie", "selection"),
    ("project", "project"),
];

/// The Dutch words for a command, if it has any.
pub fn dutch(command: &str) -> Option<&'static Aliases> {
    DUTCH.iter().find(|a| a.command == command)
}

/// The English argument chip for a typed argument in either language.
pub fn english_argument(word: &str) -> &str {
    DUTCH_ARGUMENTS
        .iter()
        .find(|(nl, _)| nl.eq_ignore_ascii_case(word))
        .map_or(word, |(_, en)| en)
}

/// The Dutch name of an English argument chip, if it has one.
pub fn dutch_argument(english: &str) -> Option<&'static str> {
    DUTCH_ARGUMENTS
        .iter()
        .find(|(_, en)| *en == english)
        .map(|(nl, _)| *nl)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{COMMANDS, is_known};
    use std::collections::BTreeSet;

    #[test]
    fn every_command_has_dutch_words_once() {
        let mut seen = BTreeSet::new();
        for aliases in DUTCH {
            assert!(is_known(aliases.command), "{}", aliases.command);
            assert!(seen.insert(aliases.command), "{} twice", aliases.command);
            assert!(!aliases.verbs.is_empty() || !aliases.words.is_empty());
        }
        for info in COMMANDS {
            assert!(dutch(info.id).is_some(), "{} has no Dutch words", info.id);
            // A command with English verbs has Dutch verbs too.
            if !info.verbs.is_empty() {
                assert!(!dutch(info.id).unwrap().verbs.is_empty(), "{}", info.id);
            }
        }
    }

    #[test]
    fn verbs_are_lower_case_ascii() {
        // The command field matches verbs case-insensitively by ASCII
        // folding, which keeps byte offsets in the typed text valid.
        for verb in DUTCH.iter().flat_map(|a| a.verbs) {
            assert!(
                verb.is_ascii() && *verb == verb.to_ascii_lowercase(),
                "{verb}"
            );
        }
        for info in COMMANDS {
            for verb in info.verbs {
                assert!(
                    verb.is_ascii() && *verb == verb.to_ascii_lowercase(),
                    "{verb}"
                );
            }
        }
    }

    #[test]
    fn arguments_translate_both_ways() {
        assert_eq!(english_argument("Pagina"), "page");
        assert_eq!(english_argument("page"), "page");
        assert_eq!(dutch_argument("selection"), Some("selectie"));
    }
}

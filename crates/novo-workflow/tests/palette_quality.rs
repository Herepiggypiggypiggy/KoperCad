//! What the command field ranks and shows (DESIGN §3): the key next to every
//! command, labels before keywords, a typo that still finds the command,
//! and recent picks first for a short query.

use novo_symbols::Library;
use novo_workflow::commands::Context;
use novo_workflow::keymap::Keymap;
use novo_workflow::palette::{LessonEntry, MacroEntry, Mode, Palette, Row, Sources, Target};

fn context() -> Context {
    Context {
        has_project: true,
        has_page: true,
        // Something selected and copied, so the editing commands can run
        // and these tests judge the match, not the greying.
        selection: 1,
        has_device: true,
        clipboard: true,
        can_undo: true,
        can_redo: true,
        ..Context::default()
    }
}

fn search(palette: &Palette, keymap: &Keymap, query: &str) -> Vec<Row> {
    let library = Library::builtin().unwrap();
    let sources = Sources {
        project: None,
        library: Some(&library),
        keymap,
        context: context(),
        lessons: &[],
        macros: &[],
    };
    palette.search(query, &sources)
}

fn position(rows: &[Row], command: &str) -> Option<usize> {
    rows.iter()
        .position(|r| matches!(&r.target, Target::Command(id) if *id == command))
}

#[test]
fn a_command_row_shows_its_key() {
    let (palette, keymap) = (Palette::default(), Keymap::workflow());
    let rows = search(&palette, &keymap, "dup");
    assert_eq!(rows[0].target, Target::Command("edit.duplicate"));
    assert_eq!(rows[0].shortcut.as_deref(), Some("Ctrl+D"));
}

#[test]
fn a_typo_and_a_dutch_word_still_find_the_command() {
    let (palette, keymap) = (Palette::default(), Keymap::workflow());
    assert_eq!(
        search(&palette, &keymap, "duplcate")[0].target,
        Target::Command("edit.duplicate")
    );
    assert_eq!(
        search(&palette, &keymap, "dupliceren")[0].target,
        Target::Command("edit.duplicate")
    );
}

#[test]
fn the_start_of_a_label_beats_a_keyword() {
    let (palette, keymap) = (Palette::default(), Keymap::workflow());
    let rows = search(&palette, &keymap, "ro");
    let rotate = position(&rows, "edit.rotate").expect("Rotate is listed");
    let wiring = position(&rows, "tool.wiring_mode");
    assert!(
        wiring.is_none_or(|w| rotate < w),
        "Rotate before commands that only have a keyword starting with ro"
    );
}

#[test]
fn recent_picks_lead_a_short_query_but_only_when_they_match() {
    let keymap = Keymap::workflow();
    let mut palette = Palette::default();
    let before = search(&palette, &keymap, "m");
    palette.picked(Target::Command("edit.mirror"));
    let after = search(&palette, &keymap, "m");
    let (was, is) = (
        position(&before, "edit.mirror").unwrap(),
        position(&after, "edit.mirror").unwrap(),
    );
    assert!(is <= was, "a recent pick does not drop: {was} -> {is}");
    assert_eq!(is, 0, "and leads a query it matches");

    // `ro` only scatters across Mirror's letters: no lift over Rotate.
    let rows = search(&palette, &keymap, "ro");
    let rotate = position(&rows, "edit.rotate").unwrap();
    assert!(position(&rows, "edit.mirror").is_none_or(|m| rotate < m));
}

#[test]
fn a_part_word_finds_the_part_before_a_command_that_only_has_it_as_a_keyword() {
    let (palette, keymap) = (Palette::default(), Keymap::workflow());
    for word in [
        "overload",
        "contactor",
        "relay",
        "breaker",
        "terminal",
        "motor",
        "fuse",
        "transformer",
        "coil",
    ] {
        let rows = search(&palette, &keymap, word);
        assert_eq!(
            rows[0].kind,
            novo_workflow::palette::Kind::Symbol,
            "`{word}` starts with a part, not {:?}",
            rows[0].label
        );
    }
}

#[test]
fn overload_is_the_overload_relay_and_fault_scenarios_stay_reachable() {
    let (palette, keymap) = (Palette::default(), Keymap::workflow());
    let rows = search(&palette, &keymap, "overload");
    assert!(rows[0].label.contains("overload"), "{:?}", rows[0].label);
    assert!(position(&rows, "sim.faults").is_some(), "still listed");
    // The command's own words still lead to it.
    let rows = search(&palette, &keymap, "fault");
    assert_eq!(rows[0].target, Target::Command("sim.faults"));
    let rows = search(&palette, &keymap, "short circuit");
    assert_eq!(rows[0].target, Target::Command("sim.faults"));
}

fn lessons() -> Vec<LessonEntry> {
    let entry = |id: &str, title: &str, words: &str| LessonEntry {
        id: id.to_owned(),
        title: title.to_owned(),
        detail: String::new(),
        words: words.to_owned(),
    };
    vec![
        entry(
            "terminal-strips",
            "Terminal strips",
            "klemmenstrook rij klemmen",
        ),
        entry(
            "wire-numbers",
            "Wire numbers",
            "wire numbering potentials draadnummers",
        ),
    ]
}

fn search_lessons(query: &str) -> Vec<Row> {
    let library = Library::builtin().unwrap();
    let keymap = Keymap::workflow();
    let lessons = lessons();
    let sources = Sources {
        project: None,
        library: Some(&library),
        keymap: &keymap,
        context: context(),
        lessons: &lessons,
        macros: &[],
    };
    Palette::default().search(query, &sources)
}

#[test]
fn a_lesson_is_found_by_title_topic_or_the_word_lesson() {
    let lesson = |rows: &[Row]| {
        rows.iter()
            .position(|r| matches!(&r.target, Target::Lesson(_)))
    };
    // By title. The `lesson` verb (help.learn) may run first; the lesson row
    // is among the first few.
    let rows = search_lessons("lesson terminal");
    assert!(lesson(&rows).is_some_and(|i| i < 3), "{rows:?}");
    assert_eq!(
        rows[lesson(&rows).unwrap()].target,
        Target::Lesson("terminal-strips".into())
    );
    // By a topic word in either language.
    let rows = search_lessons("klemmenstrook");
    assert_eq!(
        rows[lesson(&rows).unwrap()].target,
        Target::Lesson("terminal-strips".into())
    );
    let rows = search_lessons("les draadnummers");
    assert_eq!(
        rows[lesson(&rows).unwrap()].target,
        Target::Lesson("wire-numbers".into())
    );
    // A word that matches no lesson shows none.
    assert!(lesson(&search_lessons("lesson zzzz")).is_none());
}

#[test]
fn a_command_with_the_word_in_its_label_beats_a_part_that_only_starts_with_it() {
    let (palette, keymap) = (Palette::default(), Keymap::workflow());
    let rows = search(&palette, &keymap, "heat");
    assert_eq!(
        rows[0].kind,
        novo_workflow::palette::Kind::Command,
        "`heat` starts with a command, not {:?}",
        rows[0].label
    );
    let simulate = position(&rows, "heat.simulate").expect("listed");
    let element = rows
        .iter()
        .position(|r| r.label == "Heating element")
        .expect("the part is still listed");
    assert!(simulate < element);
}

#[test]
fn a_command_that_cannot_run_now_scores_below_the_same_command_that_can() {
    let library = Library::builtin().unwrap();
    let keymap = Keymap::workflow();
    let palette = Palette::default();
    let score = |can_undo: bool| {
        let sources = Sources {
            project: None,
            library: Some(&library),
            keymap: &keymap,
            context: Context {
                can_undo,
                ..context()
            },
            lessons: &[],
            macros: &[],
        };
        let rows = palette.search("undo", &sources);
        let at = position(&rows, "edit.undo").expect("Undo is found");
        (rows[at].score, rows[at].disabled.is_some())
    };
    let (ready, ready_greyed) = score(true);
    let (idle, idle_greyed) = score(false);
    assert!(!ready_greyed && idle_greyed);
    assert!(idle <= ready - 300, "{idle} vs {ready}");
}

fn macros() -> Vec<MacroEntry> {
    [
        (
            "Motor starter",
            "Direct on line starter",
            "motor starter dol contactor",
        ),
        (
            "Reversing starter",
            "Forward and reverse",
            "reversing motor contactor",
        ),
        (
            "Emergency stop",
            "E-stop with a safety relay",
            "safety emergency stop e-stop relay",
        ),
    ]
    .into_iter()
    .map(|(name, detail, words)| MacroEntry {
        name: name.into(),
        detail: detail.into(),
        words: format!("{words} {}", detail.to_lowercase()),
    })
    .collect()
}

fn search_macros(query: &str) -> Vec<Row> {
    let library = Library::builtin().unwrap();
    let keymap = Keymap::workflow();
    let macros = macros();
    let sources = Sources {
        project: None,
        library: Some(&library),
        keymap: &keymap,
        context: context(),
        lessons: &[],
        macros: &macros,
    };
    Palette::default().search(query, &sources)
}

#[test]
fn a_circuit_is_found_by_name_by_tag_and_leads_what_only_contains_the_words() {
    let rows = search_macros("motor starter");
    assert_eq!(rows[0].target, Target::Macro("Motor starter".into()));
    let rows = search_macros("reversing");
    assert_eq!(rows[0].target, Target::Macro("Reversing starter".into()));
    let rows = search_macros("e-stop");
    assert!(
        rows.iter()
            .take(3)
            .any(|r| r.target == Target::Macro("Emergency stop".into())),
        "{:?}",
        rows.iter().take(3).map(|r| &r.label).collect::<Vec<_>>()
    );
}

#[test]
fn q_finds_a_circuit_too() {
    let library = Library::builtin().unwrap();
    let keymap = Keymap::workflow();
    let macros = macros();
    let sources = Sources {
        project: None,
        library: Some(&library),
        keymap: &keymap,
        context: context(),
        lessons: &[],
        macros: &macros,
    };
    let rows = Palette::default().search_in(Mode::Place, "motor starter", &sources);
    assert!(
        rows.iter()
            .any(|r| r.target == Target::Macro("Motor starter".into()))
    );
}

#[test]
fn two_letters_do_not_list_every_circuit() {
    assert!(
        !search_macros("mo")
            .iter()
            .any(|r| matches!(r.target, Target::Macro(_)))
    );
}

#[test]
fn print_opens_the_preview_and_does_not_export_at_once() {
    let (palette, keymap) = (Palette::default(), Keymap::workflow());
    let rows = search(&palette, &keymap, "print");
    assert_eq!(rows[0].target, Target::Command("export.print_preview"));
}

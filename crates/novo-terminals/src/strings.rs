//! Every user-facing sentence and label of this crate, in one place so they
//! can be translated later. Nothing else in the crate builds text for people
//! to read; identifiers and property keys are not here.

use novo_core::{JoinLink, SameName, SymbolKey, TerminalKind, TerminalRules, WireId};

use crate::occurrence::State;

/// Plain name of a terminal kind for screens.
pub fn kind_label(kind: TerminalKind) -> &'static str {
    match kind {
        TerminalKind::Feedthrough => "feed-through",
        TerminalKind::ProtectiveEarth => "PE",
        TerminalKind::Neutral => "N",
        TerminalKind::Fuse => "fuse",
        TerminalKind::Disconnect => "disconnect",
    }
}

/// Short text for a wiring state, for the schematic and the strip editor.
pub fn state_label(state: State) -> &'static str {
    match state {
        State::Through => "through",
        State::InternalEnd => "end (internal side)",
        State::ExternalEnd => "end (external side)",
        State::Open => "not wired",
    }
}

/// Plain name of the piece that links the clamps of a joined terminal.
pub fn link_label(link: JoinLink) -> &'static str {
    match link {
        JoinLink::Connector => "connector",
        JoinLink::Jumper => "jumper",
    }
}

fn plural(word: &str, n: usize) -> String {
    if n == 1 {
        word.to_owned()
    } else {
        format!("{word}s")
    }
}

/// The terminal rule as one plain sentence, for the Terminals drawer and
/// the Rules document.
pub fn rule_sentence(rules: TerminalRules) -> String {
    match rules.same_name {
        SameName::Join => format!(
            "Terminals with the same name are one terminal, joined with {}.",
            plural(link_label(rules.join_link), 2)
        ),
        SameName::Refuse => "Each terminal name may be used once per strip.".to_owned(),
        SameName::Off => {
            "Terminals with the same name are separate terminals; the checker lists them."
                .to_owned()
        }
    }
}

// ----- checks ----------------------------------------------------------------

pub fn kind_differs_from_name(
    designation: &str,
    expected: TerminalKind,
    actual: TerminalKind,
) -> String {
    format!(
        "{designation} is named like a {} terminal but is a {} terminal.",
        kind_label(expected),
        kind_label(actual)
    )
}

pub fn unwired(designation: &str) -> String {
    format!("{designation} is drawn but not wired.")
}

pub fn single_sided(designation: &str, kind: TerminalKind) -> String {
    format!(
        "{designation} is a {} terminal but is wired on one side only.",
        kind_label(kind)
    )
}

pub fn drawn_again_refused(designation: &str) -> String {
    format!("{designation} is drawn again; the project refuses duplicate terminal names.")
}

pub fn duplicate_record(designation: &str, rule: SameName) -> String {
    match rule {
        SameName::Refuse => format!(
            "{designation} exists more than once; the project refuses duplicate terminal names."
        ),
        _ => format!(
            "{designation} exists more than once as separate terminals; no project rule joins them."
        ),
    }
}

pub fn potential_differs_from_kind(
    designation: &str,
    kind: TerminalKind,
    potential: &str,
) -> String {
    format!(
        "{designation} is a {} terminal on potential {potential}.",
        kind_label(kind)
    )
}

pub fn style_override(designation: &str) -> String {
    format!("{designation} has its own symbol or part instead of its strip's.")
}

// ----- explain -------------------------------------------------------------

pub fn explain_kind_and_state(designation: &str, kind: TerminalKind, state: State) -> String {
    format!(
        "{designation} is a {} terminal, {}.",
        kind_label(kind),
        state_label(state)
    )
}

pub fn explain_spare(designation: &str) -> String {
    format!("{designation} is a spare: it is not drawn on any page.")
}

pub fn explain_drawn_once(designation: &str, page: &str) -> String {
    format!("{designation} is drawn once, on page {page}.")
}

pub fn explain_drawn_joined(
    designation: &str,
    pages: &[String],
    clamps: usize,
    links: usize,
    link: JoinLink,
) -> String {
    format!(
        "{designation} is drawn {} times (pages {}). The project joins terminals with the same name, so these are one terminal: {clamps} clamps linked by {links} {}.",
        pages.len(),
        pages.join(", "),
        plural(link_label(link), links),
    )
}

pub fn explain_drawn_refused(designation: &str, pages: &[String]) -> String {
    format!(
        "{designation} is drawn {} times (pages {}). All drawings are this one terminal and are connected, but the project refuses duplicate terminal names, so the extra drawings are flagged.",
        pages.len(),
        pages.join(", ")
    )
}

pub fn explain_drawn_one_record(designation: &str, pages: &[String]) -> String {
    format!(
        "{designation} is drawn {} times (pages {}). All drawings are this one terminal and are connected.",
        pages.len(),
        pages.join(", ")
    )
}

pub fn explain_records_joined(records: usize) -> String {
    format!(
        "{records} separate records carry this name; the project joins them, so they are connected."
    )
}

pub fn explain_records_separate(records: usize) -> String {
    format!(
        "{records} separate records carry this name and are not connected; no project rule joins them."
    )
}

pub fn explain_jumpered(others: &[String], group: u32) -> String {
    format!("Jumpered to {} (jumper {group}).", others.join(", "))
}

// ----- screen ----------------------------------------------------------------

pub fn screen_title(tag: &str) -> String {
    format!("Terminal strip {tag}")
}

pub const SCREEN_HEADER: [&str; 6] = [
    "Internal", "Terminal", "External", "Cable", "Jumper", "Pages",
];

pub fn screen_spare(name: &str) -> String {
    format!("{name} (spare)")
}

/// How a terminal is built, e.g. `2 clamps + 1 connector`.
pub fn build(clamps: usize, links: usize, link: JoinLink) -> String {
    if clamps <= 1 {
        "1 clamp".to_owned()
    } else {
        format!(
            "{clamps} clamps + {links} {}",
            plural(link_label(link), links)
        )
    }
}

// ----- suggestions -----------------------------------------------------------

pub fn terminals_on(count: usize, strip: &str) -> String {
    format!("{count} {} on {strip}", plural("terminal", count))
}

// ----- refusals ----------------------------------------------------------------

pub fn no_room(wire: WireId, length: f64) -> String {
    format!("no room for a terminal on {wire}: it needs a straight stretch longer than {length} mm")
}

pub fn not_a_terminal_symbol(symbol: &SymbolKey) -> String {
    format!(
        "{} is not a known single-level terminal symbol (two connection points, 1 at the origin and 2 straight below it)",
        symbol.0
    )
}

pub fn no_room_to_refit(symbol: &SymbolKey) -> String {
    format!(
        "the wires at the terminal leave no room to redraw it as {}; move them first",
        symbol.0
    )
}

pub fn not_on_one_strip() -> String {
    "a jumper can only bridge terminals of one strip".to_owned()
}

/// Why a rename cannot be applied, for the find-and-replace preview.
/// `taken_by` is the designation of the terminal that has the name.
pub fn rename_problem(new: &str, problem: crate::RenameProblem, taken_by: &str) -> String {
    match problem {
        crate::RenameProblem::Empty => "The new name would be empty.".to_owned(),
        crate::RenameProblem::Taken(_) => format!("{new} is already used by {taken_by}."),
    }
}

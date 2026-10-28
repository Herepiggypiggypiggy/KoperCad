//! Generic controllers that ship with the program. Each is written as the I/O table a person
//! would paste and goes through the same wizard, so the library and the import cannot drift.
//! Names are neutral on purpose: no vendor, no product.

use super::{Options, Proposal, Table, WordList, propose};

struct Entry {
    id: &'static str,
    name: &'static str,
    kind: &'static str,
    table: &'static str,
}

const ENTRIES: &[Entry] = &[
    Entry {
        id: "generic-radio-3x2",
        kind: "radio",
        name: "Radio, 3 motions, 2-step",
        table: include_str!("../../data/library/radio-3x2.txt"),
    },
    Entry {
        id: "generic-radio-5x2",
        kind: "radio",
        name: "Radio, 5 motions, 2-step",
        table: include_str!("../../data/library/radio-5x2.txt"),
    },
    Entry {
        id: "generic-pendant-3x2",
        kind: "pendant",
        name: "Wired pendant, 3 motions, 2-step",
        table: include_str!("../../data/library/pendant-3x2.txt"),
    },
    Entry {
        id: "generic-pendant-estop-horn",
        kind: "pendant",
        name: "Pendant with E-stop and horn, hoist and travel",
        table: include_str!("../../data/library/pendant-estop-horn.txt"),
    },
    Entry {
        id: "generic-joystick-2axis",
        kind: "joystick",
        name: "Joystick, 2 axes, 3 positions each",
        table: include_str!("../../data/library/joystick-2axis.txt"),
    },
];

/// Ids and names of the generic profiles.
pub fn list() -> Vec<(&'static str, &'static str)> {
    ENTRIES.iter().map(|e| (e.id, e.name)).collect()
}

/// The generic profile with this id, built by the wizard from its table.
pub fn build(id: &str) -> Option<Proposal> {
    let e = ENTRIES.iter().find(|e| e.id == id)?;
    let table = Table::from_text(e.table);
    let options = Options {
        id: e.id.to_owned(),
        name: e.name.to_owned(),
        ..Options::default()
    };
    let mut proposal = propose(&table, &WordList::builtin(), &options);
    finish(id, &mut proposal);
    Some(proposal)
}

/// The I/O table a generic profile is built from, as text the wizard can show and edit.
pub fn table(id: &str) -> Option<&'static str> {
    ENTRIES.iter().find(|e| e.id == id).map(|e| e.table)
}

/// What a table cannot say about a generic profile: its kind, that it is a generic stand-in,
/// and that the receiver latch of a radio needs the link. Applied by [`build`], and by the
/// wizard when the person edits a library table.
pub fn finish(id: &str, proposal: &mut Proposal) {
    let Some(e) = ENTRIES.iter().find(|e| e.id == id) else {
        return;
    };
    proposal.profile.kind = e.kind.to_owned();
    if e.kind == "radio" {
        for c in proposal
            .profile
            .controls
            .iter_mut()
            .filter(|c| c.id == "start")
        {
            c.needs_link = true;
        }
    }
    proposal.profile.generic = true;
}

//! The cross-reference actions a selection offers, for the context menu
//! and the command bar alike (one list, so both always agree).

use novo_core::{Project, SymbolId};

use crate::index::XrefIndex;
use crate::link::can_link;

/// A cross-reference action on the selected symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XrefAction {
    /// G: jump to the coil, the first contact or the partner arrow.
    GoToPartner,
    /// Shift+G: list every place the device occurs.
    AllReferences,
    /// Link to a partner picked with the next click.
    Link,
}

impl XrefAction {
    /// Stable command id for the command bar and key bindings.
    pub fn id(self) -> &'static str {
        match self {
            XrefAction::GoToPartner => "xref.go_to_partner",
            XrefAction::AllReferences => "xref.all_references",
            XrefAction::Link => "xref.link",
        }
    }

    /// Text shown in the context menu and the command bar.
    pub fn label(self) -> &'static str {
        match self {
            XrefAction::GoToPartner => "Go to cross reference",
            XrefAction::AllReferences => "All references",
            XrefAction::Link => "Link cross reference",
        }
    }

    /// Default key, from the shortcut map in docs/WORKFLOW.md.
    pub fn shortcut(self) -> Option<&'static str> {
        match self {
            XrefAction::GoToPartner => Some("G"),
            XrefAction::AllReferences => Some("Shift+G"),
            XrefAction::Link => None,
        }
    }

    /// The actions that apply to a selected symbol, most useful first.
    pub fn for_symbol(project: &Project, index: &XrefIndex, symbol: SymbolId) -> Vec<XrefAction> {
        let mut actions = Vec::new();
        if !index.partners(symbol).is_empty() {
            actions.push(XrefAction::GoToPartner);
        }
        if index.reference(symbol).is_some() {
            actions.push(XrefAction::AllReferences);
        }
        if can_link(project, symbol) {
            actions.push(XrefAction::Link);
        }
        actions
    }
}

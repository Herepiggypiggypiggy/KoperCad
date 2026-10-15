//! Switch the drawing standard of a project (IEC to NFPA and back).
//!
//! [`plan`] maps every placed symbol to its counterpart in the target pack
//! (`Pack::counterpart`, checked against the pack's built-in library) and
//! lists what has none, so the preview can show it before anything changes.
//! [`SwitchPlan::command`] is one `Command::SwitchStandard`: the standard and
//! the new keys change in a single undo step, and pins, placement and roles
//! stay as they are because counterparts agree on connections.
//!
//! A symbol without a counterpart keeps its old key (the engines recognise
//! symbols of any pack) and is reported, with its device. Symbols from a
//! company library belong to no pack and are left alone silently; they are
//! only counted.

use std::collections::BTreeSet;

use novo_core::{Command, DeviceTag, PageId, Project, SymbolId, SymbolKey, SymbolRole};
use novo_symbols::{Library, Pack};

/// A symbol that stays as it is because the target pack has no drawing for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unmapped {
    pub symbol: SymbolId,
    pub page: PageId,
    pub key: String,
    /// The device the symbol belongs to, when it has one.
    pub device: Option<DeviceTag>,
}

/// What switching to `pack` would do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchPlan {
    pub pack: Pack,
    /// Symbols to re-key, with their new key.
    pub rekey: Vec<(SymbolId, SymbolKey)>,
    /// Symbols the target pack cannot draw; they keep their key.
    pub without_counterpart: Vec<Unmapped>,
    /// Symbols of no known pack (a company library), left alone.
    pub foreign: usize,
}

impl SwitchPlan {
    /// The command that applies the plan as one undo step.
    pub fn command(&self) -> Command {
        Command::SwitchStandard {
            standard: self.pack.id().to_owned(),
            symbols: self.rekey.clone(),
        }
    }

    /// The devices that keep a symbol of the old pack, each once, in tag
    /// order. Symbols without a device (terminals, interruptions) are not
    /// listed here; see [`SwitchPlan::without_counterpart`].
    pub fn devices_without_counterpart(&self) -> Vec<DeviceTag> {
        let tags: BTreeSet<_> = self
            .without_counterpart
            .iter()
            .filter_map(|u| u.device.clone())
            .collect();
        tags.into_iter().collect()
    }

    /// Whether everything switches.
    pub fn is_complete(&self) -> bool {
        self.without_counterpart.is_empty()
    }
}

/// The pack a project is set to: its `standard` rule, IEC when empty or
/// unknown.
pub fn current(project: &Project) -> Pack {
    Pack::from_id(&project.info().rules.standard).unwrap_or(Pack::Iec)
}

/// Plans the switch of every symbol in `project` to `pack`.
pub fn plan(project: &Project, pack: Pack) -> SwitchPlan {
    let library = Library::shared(pack);
    let mut plan = SwitchPlan {
        pack,
        rekey: Vec::new(),
        without_counterpart: Vec::new(),
        foreign: 0,
    };
    for symbol in project.symbols() {
        let key = symbol.symbol.0.as_str();
        match Pack::of(key) {
            None => plan.foreign += 1,
            Some(of) if of == pack => {}
            Some(_) => match Pack::counterpart(key, pack).filter(|id| library.get(id).is_some()) {
                Some(id) => plan.rekey.push((symbol.id, SymbolKey::from(id.as_str()))),
                None => plan.without_counterpart.push(Unmapped {
                    symbol: symbol.id,
                    page: symbol.page,
                    key: key.to_owned(),
                    device: match &symbol.role {
                        SymbolRole::Device { device, .. } => {
                            project.device(*device).map(|d| d.tag.clone())
                        }
                        _ => None,
                    },
                }),
            },
        }
    }
    plan
}

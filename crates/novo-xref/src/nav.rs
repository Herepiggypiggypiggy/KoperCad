//! G, Shift+G and Alt+Left/Right: jumping between references.

use novo_core::{PageId, SymbolId};

use crate::index::{Reference, XrefIndex};

/// A place in the project the view can jump to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    pub page: PageId,
    /// The symbol selected there, if any.
    pub symbol: Option<SymbolId>,
}

impl Place {
    pub fn of(reference: &Reference) -> Self {
        Place {
            page: reference.location.page,
            symbol: Some(reference.symbol),
        }
    }
}

/// Back/forward history of jumps, like a browser.
#[derive(Debug, Clone)]
pub struct JumpHistory {
    back: Vec<Place>,
    forward: Vec<Place>,
    limit: usize,
}

impl Default for JumpHistory {
    fn default() -> Self {
        JumpHistory {
            back: Vec::new(),
            forward: Vec::new(),
            limit: 100,
        }
    }
}

impl JumpHistory {
    /// Records a jump away from `from`; clears the forward list.
    pub fn record(&mut self, from: Place) {
        if self.back.last() != Some(&from) {
            self.back.push(from);
            if self.back.len() > self.limit {
                self.back.remove(0);
            }
        }
        self.forward.clear();
    }

    /// Alt+Left: where to go back to from `current`.
    pub fn back(&mut self, current: Place) -> Option<Place> {
        let to = self.back.pop()?;
        self.forward.push(current);
        Some(to)
    }

    /// Alt+Right: where to go forward to from `current`.
    pub fn forward(&mut self, current: Place) -> Option<Place> {
        let to = self.forward.pop()?;
        self.back.push(current);
        Some(to)
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }
}

/// The result of pressing G.
#[derive(Debug, Clone, PartialEq)]
pub struct Go {
    /// Where to jump.
    pub to: Place,
    /// Every target of this G cycle, for the small list shown while
    /// cycling; `position` is the index of `to` in it.
    pub targets: Vec<Reference>,
    pub position: usize,
}

/// G and Shift+G with the jump history.
///
/// G on a coil jumps to its first contact; G again (while that contact is
/// still the selection) cycles to the next. G on a contact jumps to its
/// coil; on an interruption point, to its partner. Only the first jump of a
/// cycle enters the history, so Alt+Left always returns to where G was
/// first pressed.
#[derive(Debug, Clone, Default)]
pub struct Navigator {
    pub history: JumpHistory,
    cycle: Option<Cycle>,
}

#[derive(Debug, Clone)]
struct Cycle {
    targets: Vec<Reference>,
    position: usize,
}

impl Navigator {
    /// G with `current` as the view and selection. `None` when the
    /// selection has nothing to jump to.
    pub fn go(&mut self, index: &XrefIndex, current: Place) -> Option<Go> {
        if let Some(cycle) = &mut self.cycle {
            let at_target = cycle.targets.get(cycle.position).map(|r| r.symbol) == current.symbol;
            if at_target && cycle.targets.len() > 1 {
                cycle.position = (cycle.position + 1) % cycle.targets.len();
                return Some(Go {
                    to: Place::of(&cycle.targets[cycle.position]),
                    targets: cycle.targets.clone(),
                    position: cycle.position,
                });
            }
        }
        let targets: Vec<Reference> = index
            .partners(current.symbol?)
            .into_iter()
            .cloned()
            .collect();
        let first = targets.first()?;
        let to = Place::of(first);
        self.history.record(current);
        self.cycle = Some(Cycle {
            targets: targets.clone(),
            position: 0,
        });
        Some(Go {
            to,
            targets,
            position: 0,
        })
    }

    /// Jumps to a reference picked from a list (Shift+G, Ctrl+K), recording
    /// `current` in the history.
    pub fn jump(&mut self, current: Place, to: &Reference) -> Place {
        self.cycle = None;
        self.history.record(current);
        Place::of(to)
    }

    /// Alt+Left.
    pub fn back(&mut self, current: Place) -> Option<Place> {
        self.cycle = None;
        self.history.back(current)
    }

    /// Alt+Right.
    pub fn forward(&mut self, current: Place) -> Option<Place> {
        self.cycle = None;
        self.history.forward(current)
    }

    /// Shift+G: every occurrence of the selection's device, in the order
    /// the list shows them.
    pub fn all<'i>(&self, index: &'i XrefIndex, current: Place) -> Vec<&'i Reference> {
        current
            .symbol
            .map(|s| index.all_references(s))
            .unwrap_or_default()
    }

    /// Ends a G cycle (call when the selection changes by other means).
    pub fn reset_cycle(&mut self) {
        self.cycle = None;
    }
}

//! The selection set.

use crate::scene::{ItemId, Scene};
use std::collections::HashSet;

/// How a pick or box result combines with the current selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectOp {
    Replace,
    Add,
    Remove,
    Toggle,
}

impl SelectOp {
    /// Usual desktop modifiers: Shift adds, Ctrl (Cmd on macOS) toggles.
    pub fn from_modifiers(shift: bool, command: bool) -> Self {
        match (shift, command) {
            (_, true) => SelectOp::Toggle,
            (true, false) => SelectOp::Add,
            _ => SelectOp::Replace,
        }
    }
}

/// Selected items, remembering the order they were selected in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Selection {
    order: Vec<ItemId>,
    set: HashSet<ItemId>,
    revision: u64,
}

impl Selection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    pub fn contains(&self, id: ItemId) -> bool {
        self.set.contains(&id)
    }

    /// Selected ids in selection order.
    pub fn ids(&self) -> &[ItemId] {
        &self.order
    }

    /// Bumped whenever the selection changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn clear(&mut self) {
        if !self.order.is_empty() {
            self.order.clear();
            self.set.clear();
            self.revision += 1;
        }
    }

    pub fn apply(&mut self, ids: impl IntoIterator<Item = ItemId>, op: SelectOp) {
        let before = self.order.clone();
        if op == SelectOp::Replace {
            self.order.clear();
            self.set.clear();
        }
        for id in ids {
            match op {
                SelectOp::Replace | SelectOp::Add => {
                    if self.set.insert(id) {
                        self.order.push(id);
                    }
                }
                SelectOp::Remove => {
                    if self.set.remove(&id) {
                        self.order.retain(|&x| x != id);
                    }
                }
                SelectOp::Toggle => {
                    if self.set.remove(&id) {
                        self.order.retain(|&x| x != id);
                    } else {
                        self.set.insert(id);
                        self.order.push(id);
                    }
                }
            }
        }
        if self.order != before {
            self.revision += 1;
        }
    }

    /// Drops ids that no longer exist in the scene.
    pub fn retain_existing(&mut self, scene: &Scene) {
        let n = self.order.len();
        self.order.retain(|id| scene.contains(*id));
        if self.order.len() != n {
            self.set = self.order.iter().copied().collect();
            self.revision += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops() {
        let mut s = Selection::new();
        s.apply([ItemId(1), ItemId(2)], SelectOp::Replace);
        assert_eq!(s.ids(), &[ItemId(1), ItemId(2)]);
        s.apply([ItemId(3)], SelectOp::Add);
        s.apply([ItemId(1)], SelectOp::Toggle);
        assert_eq!(s.ids(), &[ItemId(2), ItemId(3)]);
        s.apply([ItemId(1)], SelectOp::Toggle);
        assert!(s.contains(ItemId(1)));
        s.apply([ItemId(2)], SelectOp::Remove);
        assert_eq!(s.ids(), &[ItemId(3), ItemId(1)]);
        let r = s.revision();
        s.apply([ItemId(3)], SelectOp::Add);
        assert_eq!(s.revision(), r, "no-op must not bump the revision");
        s.apply([], SelectOp::Replace);
        assert!(s.is_empty());
    }

    #[test]
    fn modifiers() {
        assert_eq!(SelectOp::from_modifiers(false, false), SelectOp::Replace);
        assert_eq!(SelectOp::from_modifiers(true, false), SelectOp::Add);
        assert_eq!(SelectOp::from_modifiers(true, true), SelectOp::Toggle);
    }
}

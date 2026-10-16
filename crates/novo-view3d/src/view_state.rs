//! What the user is doing with the view, kept outside the scene.

use crate::scene::{ItemId, ItemKind};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    /// Draw edges over the faces.
    pub edges: bool,
    /// Draw tag labels over the image.
    pub labels: bool,
    /// Soft shadows from the key light (software renderers skip them).
    pub shadows: bool,
    /// Draw measuring lines: the enclosure's size and, for one selected part,
    /// its size and its distances to neighbours and the plate edge.
    pub dimensions: bool,
    /// Show the "?" badge (no part data) on every device. Off: only on the
    /// device under the pointer or in the selection.
    pub missing_marks: bool,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            edges: true,
            labels: true,
            shadows: true,
            dimensions: false,
            missing_marks: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ViewState {
    pub selection: BTreeSet<ItemId>,
    pub hover: Option<ItemId>,
    /// Layer switches: hide the door, the shell, wires.
    pub hidden: BTreeSet<ItemKind>,
    /// Items related to the selection (what a coil switches), drawn softer.
    pub related: BTreeSet<ItemId>,
    pub display: Display,
}

impl ViewState {
    pub fn is_visible(&self, kind: ItemKind) -> bool {
        !self.hidden.contains(&kind)
    }
}

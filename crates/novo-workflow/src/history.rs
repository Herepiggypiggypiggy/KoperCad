//! Back and forward through jumps, like a browser (Alt+Left, Alt+Right).
//!
//! Every jump (G, Shift+G, a palette pick, a page change by number) pushes
//! where the user was. Going back and then jumping somewhere new drops the
//! forward part, as in a browser.

/// A jump history over any place type (a page and view, a symbol).
#[derive(Debug, Clone)]
pub struct JumpHistory<T> {
    back: Vec<T>,
    forward: Vec<T>,
    limit: usize,
}

/// How many places are kept by default.
pub const DEFAULT_LIMIT: usize = 100;

impl<T> Default for JumpHistory<T> {
    fn default() -> Self {
        JumpHistory::with_limit(DEFAULT_LIMIT)
    }
}

impl<T> JumpHistory<T> {
    pub fn with_limit(limit: usize) -> Self {
        JumpHistory {
            back: Vec::new(),
            forward: Vec::new(),
            limit: limit.max(1),
        }
    }

    /// Records a jump away from `from`. Clears the forward list.
    pub fn jumped_from(&mut self, from: T) {
        self.back.push(from);
        if self.back.len() > self.limit {
            self.back.remove(0);
        }
        self.forward.clear();
    }

    /// Goes back: `current` moves to the forward list and the previous
    /// place is returned. `None` when there is nothing to go back to; then
    /// `current` is dropped and the caller keeps its own copy.
    pub fn back(&mut self, current: T) -> Option<T> {
        let previous = self.back.pop()?;
        self.forward.push(current);
        Some(previous)
    }

    /// Goes forward again after [`JumpHistory::back`].
    pub fn forward(&mut self, current: T) -> Option<T> {
        let next = self.forward.pop()?;
        self.back.push(current);
        Some(next)
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    pub fn clear(&mut self) {
        self.back.clear();
        self.forward.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_like_a_browser() {
        let mut h = JumpHistory::default();
        // On page 1, G jumps to page 3.
        h.jumped_from(1);
        assert_eq!(h.back(3), Some(1));
        assert_eq!(h.forward(1), Some(3));
        assert!(!h.can_go_forward());
        // Back to 1, then a new jump to 7 drops the forward entry.
        assert_eq!(h.back(3), Some(1));
        h.jumped_from(1);
        assert!(!h.can_go_forward());
        assert_eq!(h.back(7), Some(1));
        assert_eq!(h.back(1), None);
    }

    #[test]
    fn keeps_at_most_the_limit() {
        let mut h = JumpHistory::with_limit(2);
        for page in 0..5 {
            h.jumped_from(page);
        }
        assert_eq!(h.back(9), Some(4));
        assert_eq!(h.back(4), Some(3));
        assert_eq!(h.back(3), None);
    }
}

//! What an import or export did, and what it could not carry.
//!
//! Every exchange format loses something (STEP carries no wires, a DXF no
//! devices, AutomationML no page layout). Nothing here is dropped silently:
//! each engine returns a [`Report`] with the counts it moved and one
//! [`Lossy`] line per kind of thing it left out, and the app shows it after
//! the file is written or before the import is applied.

use std::fmt::Write as _;

/// A kind of thing the format could not carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lossy {
    /// What was left out, for example `wires`.
    pub what: String,
    pub count: usize,
    /// Why, in the user's words.
    pub why: String,
}

/// The outcome of one import or export.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// What went through, as `(noun, count)` in a stable order.
    pub moved: Vec<(String, usize)>,
    /// What stayed behind.
    pub lossy: Vec<Lossy>,
    /// Things worth knowing that lose nothing.
    pub notes: Vec<String>,
}

impl Report {
    pub fn moved(&mut self, what: impl Into<String>, count: usize) {
        if count > 0 {
            self.moved.push((what.into(), count));
        }
    }

    /// Records something left out; a count of zero records nothing.
    pub fn lost(&mut self, what: impl Into<String>, count: usize, why: impl Into<String>) {
        if count > 0 {
            self.lossy.push(Lossy {
                what: what.into(),
                count,
                why: why.into(),
            });
        }
    }

    pub fn note(&mut self, note: impl Into<String>) {
        self.notes.push(note.into());
    }

    /// How many of `what` went through, 0 when none did.
    pub fn count_of(&self, what: &str) -> usize {
        self.moved
            .iter()
            .filter(|(w, _)| w == what)
            .map(|(_, n)| n)
            .sum()
    }

    pub fn is_lossless(&self) -> bool {
        self.lossy.is_empty()
    }

    /// One line: `12 devices, 40 terminals. Left out: 6 wires.`
    pub fn summary(&self) -> String {
        let mut s = if self.moved.is_empty() {
            "Nothing found".to_string()
        } else {
            let parts: Vec<String> = self.moved.iter().map(|(w, n)| format!("{n} {w}")).collect();
            parts.join(", ")
        };
        s.push('.');
        if !self.lossy.is_empty() {
            let parts: Vec<String> = self
                .lossy
                .iter()
                .map(|l| format!("{} {}", l.count, l.what))
                .collect();
            let _ = write!(s, " Left out: {}.", parts.join(", "));
        }
        s
    }

    /// The full text for a dialog or a `.txt` next to the file.
    pub fn text(&self) -> String {
        let mut s = self.summary();
        for l in &self.lossy {
            let _ = write!(s, "\n- {} {}: {}", l.count, l.what, l.why);
        }
        for n in &self.notes {
            let _ = write!(s, "\n- {n}");
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_names_what_moved_and_what_stayed() {
        let mut r = Report::default();
        r.moved("devices", 12);
        r.moved("wires", 0);
        r.lost("wires", 6, "a STEP file has no wires");
        assert_eq!(r.summary(), "12 devices. Left out: 6 wires.");
        assert!(r.text().contains("- 6 wires: a STEP file has no wires"));
        assert!(!r.is_lossless());
    }

    #[test]
    fn an_empty_report_says_so() {
        assert_eq!(Report::default().summary(), "Nothing found.");
    }
}

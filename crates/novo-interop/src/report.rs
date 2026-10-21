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

/// Singular forms of the nouns the reports count, for a count of one.
const SINGULAR: [(&str, &str); 22] = [
    ("cables", "cable"),
    ("cores", "core"),
    ("nodes", "node"),
    ("devices", "device"),
    ("boxes", "box"),
    ("symbols", "symbol"),
    ("texts", "text"),
    ("points", "point"),
    ("items", "item"),
    ("solids", "solid"),
    ("strips", "strip"),
    ("terminals", "terminal"),
    ("wires", "wire"),
    ("nets", "net"),
    ("pages", "page"),
    ("locations", "location"),
    ("sections", "section"),
    ("footprints", "footprint"),
    ("rails", "rail"),
    ("ducts", "duct"),
    ("routes", "route"),
    ("sizes", "size"),
];

/// `n` and what it counts, in the right number: "1 symbol drawn as labelled
/// box", "3 symbols drawn as labelled boxes". `what` is written in the plural;
/// a list ("lines, rectangles and arcs") is left as it is.
pub fn counted(n: usize, what: &str) -> String {
    if n != 1 || what.contains(',') {
        return format!("{n} {what}");
    }
    let words: Vec<&str> = what
        .split(' ')
        .map(|w| {
            SINGULAR
                .iter()
                .find(|(plural, _)| *plural == w)
                .map_or(w, |(_, one)| *one)
        })
        .collect();
    format!("{n} {}", words.join(" "))
}

impl Lossy {
    /// "3 symbols drawn as labelled boxes: why", in the right number.
    pub fn line(&self) -> String {
        format!("{}: {}", counted(self.count, &self.what), self.why)
    }
}

/// The outcome of one import or export.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// What went through, as `(noun, count)` in a stable order.
    pub moved: Vec<(String, usize)>,
    /// What stayed behind.
    pub lossy: Vec<Lossy>,
    /// What came across with a value the file did not state: a box where no
    /// library symbol fits, a default size, a route the importer drew.
    /// Nothing is missing, but it is not exact either.
    pub guessed: Vec<Lossy>,
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

    /// Records something kept with a guessed or defaulted value; a count of
    /// zero records nothing.
    pub fn guess(&mut self, what: impl Into<String>, count: usize, why: impl Into<String>) {
        if count > 0 {
            self.guessed.push(Lossy {
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

    /// Nothing left out and nothing guessed.
    pub fn is_exact(&self) -> bool {
        self.lossy.is_empty() && self.guessed.is_empty()
    }

    /// One line: `12 devices, 40 terminals. Left out: 6 wires.`
    pub fn summary(&self) -> String {
        let mut s = if self.moved.is_empty() {
            "Nothing found".to_string()
        } else {
            let parts: Vec<String> = self.moved.iter().map(|(w, n)| counted(*n, w)).collect();
            parts.join(", ")
        };
        s.push('.');
        if !self.lossy.is_empty() {
            let parts: Vec<String> = self
                .lossy
                .iter()
                .map(|l| counted(l.count, &l.what))
                .collect();
            let _ = write!(s, " Left out: {}.", parts.join(", "));
        }
        if !self.guessed.is_empty() {
            let parts: Vec<String> = self
                .guessed
                .iter()
                .map(|l| counted(l.count, &l.what))
                .collect();
            let _ = write!(s, " Guessed: {}.", parts.join(", "));
        }
        s
    }

    /// The full text for a dialog or a `.txt` next to the file.
    pub fn text(&self) -> String {
        let mut s = self.summary();
        for l in &self.lossy {
            let _ = write!(s, "\n- {}", l.line());
        }
        for l in &self.guessed {
            let _ = write!(s, "\n- {} (guessed): {}", counted(l.count, &l.what), l.why);
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
    fn guesses_are_listed_apart_from_what_was_left_out() {
        let mut r = Report::default();
        r.moved("symbols", 10);
        r.guess(
            "symbols as boxes",
            3,
            "no library symbol has that pin count",
        );
        r.guess("sizes", 0, "never shown");
        assert_eq!(r.summary(), "10 symbols. Guessed: 3 symbols as boxes.");
        assert!(
            r.text()
                .contains("- 3 symbols as boxes (guessed): no library")
        );
        assert!(r.is_lossless(), "a guess loses nothing");
        assert!(!r.is_exact());
        assert!(Report::default().is_exact());
    }

    #[test]
    fn an_empty_report_says_so() {
        assert_eq!(Report::default().summary(), "Nothing found.");
    }

    #[test]
    fn a_count_of_one_reads_in_the_singular() {
        assert_eq!(
            counted(1, "symbols drawn as labelled boxes"),
            "1 symbol drawn as labelled box"
        );
        assert_eq!(
            counted(3, "symbols drawn as labelled boxes"),
            "3 symbols drawn as labelled boxes"
        );
        assert_eq!(
            counted(0, "devices without a footprint"),
            "0 devices without a footprint"
        );
        assert_eq!(
            counted(1, "devices without a footprint"),
            "1 device without a footprint"
        );
        assert_eq!(counted(1, "sections or locations"), "1 section or location");
        // A list is left alone rather than half converted.
        assert_eq!(
            counted(1, "lines, rectangles and location boxes"),
            "1 lines, rectangles and location boxes"
        );
        let mut r = Report::default();
        r.guess("symbols as boxes", 1, "no library symbol fits");
        assert_eq!(r.summary(), "Nothing found. Guessed: 1 symbol as box.");
        assert_eq!(
            r.guessed[0].line(),
            "1 symbol as box: no library symbol fits"
        );
    }
}

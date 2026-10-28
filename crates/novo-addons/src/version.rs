//! Versions and ranges. `major.minor.patch` only (a missing part is 0); a range is comparators
//! joined by commas, for example `>=0.1.0, <0.3.0`. No pre-release or build tags: an add-on is
//! compatible with a release, not with a nightly.

use std::fmt;

/// A `major.minor.patch` version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub const fn new(major: u32, minor: u32, patch: u32) -> Version {
        Version {
            major,
            minor,
            patch,
        }
    }

    /// Parses `1`, `1.2` or `1.2.3`. Anything else (tags, signs, spaces) is refused.
    pub fn parse(text: &str) -> Option<Version> {
        let mut parts = text.trim().split('.');
        let mut next = |required: bool| -> Option<u32> {
            match parts.next() {
                None if required => None,
                None => Some(0),
                Some(p) if !p.is_empty() && p.len() <= 9 && p.bytes().all(|b| b.is_ascii_digit()) => {
                    p.parse().ok()
                }
                Some(_) => None,
            }
        };
        let major = next(true)?;
        let minor = next(false)?;
        let patch = next(false)?;
        parts.next().is_none().then_some(Version::new(major, minor, patch))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Ge,
    Gt,
    Le,
    Lt,
    Eq,
}

/// A range of versions, for example the program versions an add-on works with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    text: String,
    parts: Vec<(Op, Version)>,
}

impl Requirement {
    /// Parses `>=0.1.0, <0.3`. A bare version means exactly that version. `*` or empty = any.
    pub fn parse(text: &str) -> Option<Requirement> {
        let trimmed = text.trim();
        let mut parts = Vec::new();
        if !trimmed.is_empty() && trimmed != "*" {
            for piece in trimmed.split(',') {
                let piece = piece.trim();
                let (op, rest) = if let Some(r) = piece.strip_prefix(">=") {
                    (Op::Ge, r)
                } else if let Some(r) = piece.strip_prefix("<=") {
                    (Op::Le, r)
                } else if let Some(r) = piece.strip_prefix('>') {
                    (Op::Gt, r)
                } else if let Some(r) = piece.strip_prefix('<') {
                    (Op::Lt, r)
                } else if let Some(r) = piece.strip_prefix('=') {
                    (Op::Eq, r)
                } else {
                    (Op::Eq, piece)
                };
                parts.push((op, Version::parse(rest)?));
            }
        }
        Some(Requirement {
            text: if parts.is_empty() {
                "*".to_owned()
            } else {
                trimmed.to_owned()
            },
            parts,
        })
    }

    pub fn matches(&self, v: Version) -> bool {
        self.parts.iter().all(|&(op, want)| match op {
            Op::Ge => v >= want,
            Op::Gt => v > want,
            Op::Le => v <= want,
            Op::Lt => v < want,
            Op::Eq => v == want,
        })
    }

    /// The lowest version the range names, for "needs KoperCAD 0.2.0 or newer".
    pub fn lower_bound(&self) -> Option<Version> {
        self.parts
            .iter()
            .filter_map(|&(op, v)| matches!(op, Op::Ge | Op::Gt | Op::Eq).then_some(v))
            .max()
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl fmt::Display for Requirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_parse_and_order() {
        assert_eq!(Version::parse("1.2.3"), Some(Version::new(1, 2, 3)));
        assert_eq!(Version::parse("2"), Some(Version::new(2, 0, 0)));
        assert_eq!(Version::parse("0.10"), Some(Version::new(0, 10, 0)));
        for bad in ["", "a", "1.2.3.4", "1..2", "-1", "1.2.3-rc1", "+1", "99999999999"] {
            assert_eq!(Version::parse(bad), None, "{bad:?}");
        }
        assert!(Version::new(0, 10, 0) > Version::new(0, 9, 9));
    }

    #[test]
    fn ranges_match() {
        let r = Requirement::parse(">=0.1.0, <0.3").unwrap();
        assert!(r.matches(Version::new(0, 1, 0)));
        assert!(r.matches(Version::new(0, 2, 9)));
        assert!(!r.matches(Version::new(0, 3, 0)));
        assert!(!r.matches(Version::new(0, 0, 9)));
        assert_eq!(r.lower_bound(), Some(Version::new(0, 1, 0)));
        assert!(Requirement::parse("*").unwrap().matches(Version::new(9, 9, 9)));
        assert!(Requirement::parse("1.2.3").unwrap().matches(Version::new(1, 2, 3)));
        assert!(!Requirement::parse("1.2.3").unwrap().matches(Version::new(1, 2, 4)));
        assert!(Requirement::parse(">=x").is_none());
        assert!(Requirement::parse(">=1,").is_none());
    }
}

//! Finding devices, cables, terminals and pages by tag, designation or name.

use crate::id::AnyId;
use crate::project::{Project, natural_cmp};

/// How well a search hit matches, best first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Match {
    /// The whole text, or a whole part of it, equals the query: `K1` finds
    /// `=A1-K1`, `-X2:1` finds terminal 1 of `-X2`.
    Exact,
    /// The text, or a part of it, starts with the query.
    Prefix,
    /// The query appears somewhere in the text.
    Contains,
}

/// One search result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub id: AnyId,
    /// What matched: the tag, designation (`-X2:1`) or page name and title.
    pub text: String,
    pub quality: Match,
}

/// Grades `text` against a lower-case query. Parts are the pieces between
/// tag prefixes and separators, so `=A1+CAB1-K1` has parts `A1`, `CAB1`,
/// `K1` and `-K1` matches as a part too.
fn grade(text: &str, query: &str) -> Option<Match> {
    let text = text.to_lowercase();
    if text == query {
        return Some(Match::Exact);
    }
    let separators = ['=', '+', '-', ':', ' '];
    let parts: Vec<&str> = text.split(separators).filter(|p| !p.is_empty()).collect();
    let starts: Vec<&str> = text
        .match_indices(separators)
        .map(|(i, _)| &text[i..])
        .chain([text.as_str()])
        .collect();
    let bare = query.trim_start_matches(separators);
    if parts.contains(&bare) || starts.contains(&query) {
        Some(Match::Exact)
    } else if starts.iter().any(|s| s.starts_with(query))
        || parts.iter().any(|p| p.starts_with(bare))
    {
        Some(Match::Prefix)
    } else if text.contains(query) {
        Some(Match::Contains)
    } else {
        None
    }
}

impl Project {
    /// Devices, cables, terminals and pages whose tag, designation or page
    /// name and title contain `query`, ignoring case. Best matches first
    /// ([`Match`]), then in natural order of their text. An empty query
    /// finds nothing.
    pub fn search(&self, query: &str) -> Vec<SearchHit> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        let mut hits = Vec::new();
        let mut consider = |id: AnyId, text: String| {
            if let Some(quality) = grade(&text, &query) {
                hits.push(SearchHit { id, text, quality });
            }
        };
        for device in self.devices() {
            consider(device.id.into(), device.tag.to_string());
        }
        for cable in self.cables() {
            consider(cable.id.into(), cable.tag.to_string());
        }
        for terminal in self.terminals() {
            if let Some(designation) = self.terminal_designation(terminal.id) {
                consider(terminal.id.into(), designation);
            }
        }
        for page in self.pages() {
            let text = if page.title.is_empty() {
                page.name.clone()
            } else {
                format!("{} {}", page.name, page.title)
            };
            consider(page.id.into(), text);
        }
        hits.sort_by(|a, b| {
            a.quality
                .cmp(&b.quality)
                .then_with(|| natural_cmp(&a.text, &b.text))
                .then_with(|| a.id.cmp(&b.id))
        });
        hits
    }
}

#[cfg(test)]
mod tests {
    use super::{Match, grade};

    #[test]
    fn grades() {
        assert_eq!(grade("=A1+CAB1-K1", "=a1+cab1-k1"), Some(Match::Exact));
        assert_eq!(grade("=A1+CAB1-K1", "k1"), Some(Match::Exact));
        assert_eq!(grade("=A1+CAB1-K1", "-k1"), Some(Match::Exact));
        assert_eq!(grade("-X2:1", "-x2:1"), Some(Match::Exact));
        assert_eq!(grade("-X2:1", ":1"), Some(Match::Exact));
        assert_eq!(grade("=A1+CAB1-K12", "k1"), Some(Match::Prefix));
        assert_eq!(grade("=A1+CAB1-K12", "+cab"), Some(Match::Prefix));
        assert_eq!(grade("-XK12", "k1"), Some(Match::Contains));
        assert_eq!(grade("-K1", "q1"), None);
    }
}

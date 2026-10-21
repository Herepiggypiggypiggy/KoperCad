//! Small fuzzy matcher for the palette and the insert search.
//!
//! A query matches when its characters appear in order in the text,
//! ignoring case and spaces. The score rewards what people type on purpose:
//! an exact match, a prefix, matches at word starts and runs of adjacent
//! characters. Higher is better.

/// From this many typed letters a match must be more than scattered.
const SCATTER_FROM: usize = 4;

/// Score of `query` against `text`, or `None` when it does not match. An
/// empty query matches everything with score 0.
pub fn score(query: &str, text: &str) -> Option<i32> {
    let query: Vec<char> = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    if query.is_empty() {
        return Some(0);
    }
    let lower: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
    let compact: String = lower.iter().filter(|c| !c.is_whitespace()).collect();
    let q: String = query.iter().collect();

    if compact == q {
        return Some(1000);
    }
    let mut total = 0;
    if compact.starts_with(&q) {
        total += 500;
    } else if let Some(word) = word_starts(&lower).find(|&i| starts_with_at(&lower, i, &query)) {
        // A whole word prefix, like "fit" in "Zoom to fit".
        total += 300 - i32::try_from(word.min(100)).unwrap_or(100);
    }

    let mut qi = 0;
    let mut previous: Option<usize> = None;
    let mut first: Option<usize> = None;
    let mut last = 0usize;
    let mut starts = 0usize;
    for (i, &c) in lower.iter().enumerate() {
        if qi == query.len() {
            break;
        }
        if c != query[qi] {
            continue;
        }
        let at_word_start = i == 0 || !lower[i - 1].is_alphanumeric();
        total += if at_word_start { 15 } else { 1 };
        starts += usize::from(at_word_start);
        if previous.is_some_and(|p| p + 1 == i) {
            total += 10;
        }
        first.get_or_insert(i);
        last = i;
        previous = Some(i);
        qi += 1;
    }
    if qi < query.len() {
        return None;
    }
    // A longer query whose letters are scattered over several words (and are
    // not the initials of them) is noise, not a typo: a typo stays inside one
    // word. `option` is not in "Reset note position" or "Interruption point".
    if query.len() >= SCATTER_FROM
        && !compact.contains(&q)
        && starts < query.len()
        && first.is_some_and(|f| lower[f..=last].iter().any(|c| !c.is_alphanumeric()))
    {
        return None;
    }
    // Shorter texts win ties: "Wire" before "Wire numbering".
    Some(total - i32::try_from(lower.len().min(200)).unwrap_or(200) / 10)
}

fn word_starts(text: &[char]) -> impl Iterator<Item = usize> + '_ {
    (0..text.len())
        .filter(|&i| text[i].is_alphanumeric() && (i == 0 || !text[i - 1].is_alphanumeric()))
}

fn starts_with_at(text: &[char], at: usize, query: &[char]) -> bool {
    let rest: Vec<char> = text[at..]
        .iter()
        .copied()
        .filter(|c| !c.is_whitespace())
        .collect();
    rest.starts_with(query)
}

/// The best score of `query` against any of `texts`.
pub fn best_score<'a>(query: &str, texts: impl IntoIterator<Item = &'a str>) -> Option<i32> {
    texts.into_iter().filter_map(|t| score(query, t)).max()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_beats_prefix_beats_word_beats_scattered() {
        let exact = score("wire", "Wire").unwrap();
        let prefix = score("wire", "Wire numbering").unwrap();
        let word = score("fit", "Zoom to fit").unwrap();
        let scattered = score("zf", "Zoom to fit").unwrap();
        assert!(exact > prefix);
        assert!(prefix > word);
        assert!(word > scattered);
    }

    #[test]
    fn order_matters_and_case_does_not() {
        assert!(score("NP", "new page").is_some());
        assert!(score("pn", "new page").is_none());
        assert_eq!(score("", "anything"), Some(0));
    }

    #[test]
    fn spaces_in_the_query_are_ignored() {
        assert!(score("new page", "New page").unwrap() >= 1000);
        assert!(score("newp", "New page").is_some());
    }
}

//! Small fuzzy matcher for the palette and the insert search.
//!
//! A query matches when its characters appear in order in the text,
//! ignoring case and spaces. The score rewards what people type on purpose:
//! an exact match, a prefix, matches at word starts and runs of adjacent
//! characters. Higher is better.

/// From this many typed letters a match must lie in the words of the text,
/// not be scattered across them.
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
    for (i, &c) in lower.iter().enumerate() {
        if qi == query.len() {
            break;
        }
        if c != query[qi] {
            continue;
        }
        let at_word_start = i == 0 || !lower[i - 1].is_alphanumeric();
        total += if at_word_start { 15 } else { 1 };
        if previous.is_some_and(|p| p + 1 == i) {
            total += 10;
        }
        previous = Some(i);
        qi += 1;
    }
    if qi < query.len() {
        return None;
    }
    if query.len() >= SCATTER_FROM && !compact.contains(&q) && !fits_in_words(&query, &lower) {
        return None;
    }
    // Shorter texts win ties: "Wire" before "Wire numbering".
    Some(total - i32::try_from(lower.len().min(200)).unwrap_or(200) / 10)
}

/// Whether the query can be laid over the words of the text in order, each
/// word taking a piece of it: a typo stays inside a word and a small word of
/// the label may be left out ("zoom fit" finds "Zoom to fit"), but a lone
/// letter picked out of the middle of a word is noise. `option` is not in
/// "Reset note position", which holds an `o` only in the middle of "note".
fn fits_in_words(query: &[char], text: &[char]) -> bool {
    if !query.iter().all(|c| c.is_alphanumeric()) {
        return true;
    }
    let mut words: Vec<&[char]> = Vec::new();
    let mut at = 0;
    while at < text.len() {
        if text[at].is_alphanumeric() {
            let end = text[at..]
                .iter()
                .position(|c| !c.is_alphanumeric())
                .map_or(text.len(), |n| at + n);
            words.push(&text[at..end]);
            at = end;
        } else {
            at += 1;
        }
    }
    // `free[n]`: the first n query letters are placed in the words so far.
    // `lone[n]`: the same, and the last of them is a single initial taken from
    // the previous word, so the next word must take the following letters too
    // ("wnum" is "Wire numbering"; the `o` of "or" in "Show or hide the
    // Position box" is not the start of "option").
    let mut free = vec![false; query.len() + 1];
    free[0] = true;
    let mut lone = vec![false; query.len() + 1];
    for word in words {
        let (mut next_free, mut next_lone) =
            (vec![false; query.len() + 1], vec![false; query.len() + 1]);
        for from in 0..query.len() {
            if !free[from] && !lone[from] {
                continue;
            }
            let mut w = 0;
            for k in 1..=query.len() - from {
                let Some(found) = word[w..].iter().position(|&c| c == query[from + k - 1]) else {
                    break;
                };
                w += found + 1;
                // A piece of a longer match starts its word; the one piece
                // that is the whole query may start anywhere.
                if k == query.len() {
                    return true;
                }
                if k >= 2 && word[0] == query[from] {
                    next_free[from + k] = true;
                } else if found == 0 {
                    next_lone[from + k] = true;
                }
            }
        }
        // A word left out keeps the free states only.
        for n in 0..=query.len() {
            next_free[n] |= free[n];
        }
        (free, lone) = (next_free, next_lone);
    }
    free[query.len()] || lone[query.len()]
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

    #[test]
    fn a_small_word_of_the_label_may_be_left_out() {
        assert!(score("zoom fit", "Zoom to fit").is_some());
        assert!(score("export pdf", "Export to PDF").is_some());
        assert!(score("cable run", "Show cable and wire runs in 3D").is_some());
        assert!(score("wire runs", "Show cable and wire runs in 3D").is_some());
    }

    #[test]
    fn a_typo_in_any_word_or_an_abbreviation_still_matches() {
        assert!(score("new proect", "New project").is_some());
        assert!(score("renme page", "Rename page").is_some());
        assert!(score("close proect", "Close project").is_some());
        assert!(score("nw proect", "New project").is_some());
        assert!(score("saas", "Save as").is_some());
        assert!(score("opepro", "Open project").is_some());
        assert!(score("duplcate", "Duplicate").is_some());
    }

    #[test]
    fn letters_picked_out_of_the_middle_of_words_do_not_match() {
        assert!(score("option", "Reset note position").is_none());
        assert!(score("option", "Interruption point").is_none());
        assert!(score("option", "Options").is_some());
    }
}

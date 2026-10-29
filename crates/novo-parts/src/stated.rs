//! Technical data a part's own text states, in the keys the device
//! description recipes read ([`crate::describe::keys`], [`crate::eng`]).
//!
//! Databases of makers and shops keep most technical data in free text
//! (`Coil 24 V DC | 2 auxiliary contacts 1 NO + 1 NC | 3 poles`). This reads
//! only what a piece of text says outright, label and value together; a
//! number without its label (`230 VAC` could be a coil or a rated voltage)
//! states nothing, and the key is left out. Dutch and English labels.

use novo_core::{Properties, Value};

use crate::describe::{contact_counts, first_number, hertz_in, keys};
use crate::fit::parse_supply;
use crate::schema::eng;

/// Words that mark a segment as being about the coil.
const COIL: &[&str] = &["coil", "spoel", "control voltage", "stuurspanning"];
/// Words that mark the auxiliary contacts.
const AUX: &[&str] = &["auxiliary", "aux.", "aux ", "hulpcontact", "hulp"];
/// Words that mark the main contacts.
const MAIN: &[&str] = &["main contact", "hoofdcontact", "main pole"];
/// Words that mark the rated operating voltage.
const RATED: &[&str] = &[
    "rated voltage",
    "rated operational voltage",
    "rated operating voltage",
    "nominal voltage",
    "nominale spanning",
    "bedrijfsspanning",
];

/// What `texts` state, keyed for the description recipes. Values already in
/// the result are not replaced by later texts.
pub fn stated_data(texts: &[&str]) -> Properties {
    let mut out = Properties::new();
    for text in texts {
        for segment in text.split(['|', ';', '\n']) {
            read_segment(segment.trim(), &mut out);
        }
    }
    out
}

fn has(lower: &str, words: &[&str]) -> bool {
    words.iter().any(|w| lower.contains(w))
}

fn read_segment(segment: &str, out: &mut Properties) {
    if segment.is_empty() {
        return;
    }
    let lower = segment.to_lowercase();
    if let Some(poles) = poles_in(&lower) {
        out.entry(eng::POLES.into()).or_insert(Value::Int(poles));
    }
    if has(&lower, COIL) {
        // The number next to the label: after it, else in the clause before it
        // (`24 V AC coil`), never another number of the segment (`3 poles`).
        let (before, after) = split_at_label(&lower, COIL);
        let near_before = before.rsplit(',').next().unwrap_or("");
        let found = parse_supply(after)
            .map(|s| (s, after))
            .or_else(|| parse_supply(near_before).map(|s| (s, near_before)));
        if let Some((supply, clause)) = found {
            out.entry(eng::COIL_VOLTAGE_V.into())
                .or_insert(Value::Float(supply.volts));
            let kind = match (supply.ac, supply.dc) {
                (true, true) => "AC/DC",
                (true, false) => "AC",
                _ => "DC",
            };
            out.entry(keys::COIL_KIND.into())
                .or_insert(Value::Text(kind.into()));
            if let Some(hz) = hertz_in(clause) {
                let value = hz
                    .parse::<i64>()
                    .map_or_else(|_| Value::Text(hz), Value::Int);
                out.entry(keys::COIL_HZ.into()).or_insert(value);
            }
        }
    } else if has(&lower, RATED)
        && let Some(v) = first_number(split_at_label(&lower, RATED).1).filter(|v| *v > 0.0)
    {
        out.entry(keys::VOLTAGE_V.into()).or_insert(Value::Float(v));
    }
    if has(&lower, AUX)
        && let Some((no, nc)) = counts_near(&lower, AUX)
    {
        out.entry(keys::AUX_NO.into())
            .or_insert(Value::Int(i64::from(no)));
        out.entry(keys::AUX_NC.into())
            .or_insert(Value::Int(i64::from(nc)));
    }
    if has(&lower, MAIN)
        && let Some((no, 0)) = counts_near(&lower, MAIN)
        && no > 0
    {
        out.entry(keys::MAIN_NO.into())
            .or_insert(Value::Int(i64::from(no)));
    }
}

/// The text before and after the first of `words` in `lower`.
fn split_at_label<'a>(lower: &'a str, words: &[&str]) -> (&'a str, &'a str) {
    words
        .iter()
        .filter_map(|w| lower.find(w).map(|i| (i, w.len())))
        .min_by_key(|(i, _)| *i)
        .map_or((lower, ""), |(i, n)| (&lower[..i], &lower[i + n..]))
}

/// `1 NO + 1 NC` written after (or before) the marking word.
fn counts_near(lower: &str, words: &[&str]) -> Option<(u32, u32)> {
    let (at, word) = words
        .iter()
        .filter_map(|w| lower.find(w).map(|i| (i, *w)))
        .min_by_key(|(i, _)| *i)?;
    let after = lower[at + word.len()..]
        .trim_start_matches(|c: char| c.is_alphabetic() || c == '.' || c == ':' || c == '=')
        .trim();
    let before = lower[..at].trim();
    let joined = format!("{before} {after}");
    [after, before, joined.as_str()]
        .into_iter()
        .filter(|t| t.chars().any(|c| c.is_ascii_digit()) && (t.contains("no") || t.contains("nc")))
        .find_map(contact_counts)
}

/// `3P`, `3-pole`, `3 poles`, `3-polig`: the pole count when stated outright.
fn poles_in(lower: &str) -> Option<i64> {
    let tokens: Vec<&str> = lower
        .split(|c: char| c.is_whitespace() || matches!(c, '-' | ',' | '(' | ')' | '/'))
        .filter(|t| !t.is_empty())
        .collect();
    let valid = |n: i64| (1..=8).contains(&n).then_some(n);
    for (i, t) in tokens.iter().enumerate() {
        if let Some(n) = t.strip_suffix('p').and_then(|d| d.parse().ok()) {
            return valid(n);
        }
        if matches!(*t, "pole" | "poles" | "polig" | "pol" | "poler")
            && let Some(n) = i.checked_sub(1).and_then(|j| tokens[j].parse().ok())
        {
            return valid(n);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get<'a>(p: &'a Properties, key: &str) -> Option<&'a Value> {
        p.get(key)
    }

    #[test]
    fn labelled_values_are_read() {
        let d = stated_data(&[
            "Coil 24 V DC | 3 poles | Auxiliary contacts 1 NO + 1 NC | Rated voltage 690 V",
        ]);
        assert_eq!(get(&d, "coil_voltage_v"), Some(&Value::Float(24.0)));
        assert_eq!(
            get(&d, "coil_current_type"),
            Some(&Value::Text("DC".into()))
        );
        assert_eq!(get(&d, "poles"), Some(&Value::Int(3)));
        assert_eq!(get(&d, "aux_contacts_no"), Some(&Value::Int(1)));
        assert_eq!(get(&d, "aux_contacts_nc"), Some(&Value::Int(1)));
        assert_eq!(get(&d, "rated_voltage_v"), Some(&Value::Float(690.0)));
    }

    #[test]
    fn dutch_labels_and_frequency() {
        let d = stated_data(&["Spoel 230 V AC 50/60 Hz; 4-polig; hulpcontacten 2NO+2NC"]);
        assert_eq!(get(&d, "coil_voltage_v"), Some(&Value::Float(230.0)));
        assert_eq!(
            get(&d, "coil_current_type"),
            Some(&Value::Text("AC".into()))
        );
        assert_eq!(
            get(&d, "coil_frequency_hz"),
            Some(&Value::Text("50/60".into()))
        );
        assert_eq!(get(&d, "poles"), Some(&Value::Int(4)));
        assert_eq!(get(&d, "aux_contacts_no"), Some(&Value::Int(2)));
        assert_eq!(get(&d, "aux_contacts_nc"), Some(&Value::Int(2)));
    }

    #[test]
    fn the_number_next_to_the_label_is_the_one_read() {
        let d = stated_data(&["3-pole contactor, coil 24 V DC"]);
        assert_eq!(get(&d, "coil_voltage_v"), Some(&Value::Float(24.0)));
        assert_eq!(get(&d, "poles"), Some(&Value::Int(3)));
        let d = stated_data(&["3 poles, rated voltage 690 V"]);
        assert_eq!(get(&d, "rated_voltage_v"), Some(&Value::Float(690.0)));
        // The value written before its label, in its own clause.
        let d = stated_data(&["3 poles, 24 V AC coil"]);
        assert_eq!(get(&d, "coil_voltage_v"), Some(&Value::Float(24.0)));
        assert_eq!(
            get(&d, "coil_current_type"),
            Some(&Value::Text("AC".into()))
        );
        // A coil label with no supply next to it states nothing.
        let d = stated_data(&["3-pole contactor, coil"]);
        assert!(!d.contains_key("coil_voltage_v"));
    }

    #[test]
    fn a_frequency_alone_is_a_number() {
        let d = stated_data(&["Coil 24 V AC 50 Hz"]);
        assert_eq!(get(&d, "coil_frequency_hz"), Some(&Value::Int(50)));
    }

    #[test]
    fn an_unlabelled_number_states_nothing() {
        // 230 VAC could be the coil or the rated voltage; contacts could be main or auxiliary.
        let d = stated_data(&[
            "K22E, 2NO+2NC, 230 VAC",
            "Size S0, screw-type",
            "Setting range 4,5-6.3 A",
        ]);
        assert!(d.is_empty(), "{d:?}");
    }

    #[test]
    fn mixed_contacts_are_not_main_contacts() {
        let d = stated_data(&["Main contacts 3 NO + 1 NC"]);
        assert_eq!(get(&d, "main_contacts_no"), None);
        let d = stated_data(&["3 main contacts NO"]);
        assert_eq!(get(&d, "main_contacts_no"), Some(&Value::Int(3)));
    }

    #[test]
    fn nonsense_is_ignored() {
        assert!(
            stated_data(&["", "|;", "poles", "0P", "9 poles", "coil", "rated voltage"]).is_empty()
        );
    }

    #[test]
    fn the_first_text_wins() {
        let d = stated_data(&["3 poles", "4 poles"]);
        assert_eq!(get(&d, "poles"), Some(&Value::Int(3)));
    }
}

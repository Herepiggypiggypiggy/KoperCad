//! Reading one table row: which motion, direction, step, role or voltage its words name.

use super::table::{Row, parse_step_cell};
use super::words::{Dir, Role, Tag, WordList, tokenize};

/// Supply voltage named in a row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Voltage {
    pub volts: f64,
    /// `Some(true)` for AC, `Some(false)` for DC, `None` when the text only says V.
    pub ac: Option<bool>,
}

/// What the words of one row say.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reading {
    pub motions: Vec<String>,
    pub dirs: Vec<Dir>,
    /// Both directions of this motion named by one abbreviation (H/Z, L/R).
    pub pair: Option<String>,
    pub step: Option<u8>,
    pub role: Option<Role>,
    pub voltage: Option<Voltage>,
    pub kind: Option<String>,
    /// A bare number in the text that could be a step or a unit number (hoist 2).
    pub loose_number: Option<String>,
    /// The step column held something that is not a step.
    pub bad_step_cell: bool,
    /// The step column and the text name different steps.
    pub step_conflict: bool,
    /// At least one word was recognised.
    pub known: bool,
}

fn step_from_token(t: &str) -> Option<u8> {
    let digit = |s: &str| -> Option<u8> {
        let mut it = s.chars();
        let d = it.next()?.to_digit(10)?;
        it.next()
            .is_none()
            .then_some(d as u8)
            .filter(|n| (1..=5).contains(n))
    };
    if let Some(d) = t.strip_prefix('s').and_then(digit) {
        return Some(d);
    }
    let split = t.find(|c: char| !c.is_ascii_digit())?;
    let (num, suffix) = t.split_at(split);
    if matches!(suffix, "s" | "e" | "de" | "ste" | "st" | "nd" | "rd") {
        digit(num)
    } else {
        None
    }
}

fn voltage_from(tokens: &[String]) -> Option<Voltage> {
    for (i, t) in tokens.iter().enumerate() {
        let split = t.find(|c: char| !c.is_ascii_digit());
        let (num, unit) = match split {
            Some(s) if s > 0 => t.split_at(s),
            None => (t.as_str(), tokens.get(i + 1).map_or("", String::as_str)),
            _ => continue,
        };
        let ac = match unit {
            "v" | "volt" => None,
            "vac" => Some(true),
            "vdc" => Some(false),
            _ => continue,
        };
        if let Ok(volts) = num.parse::<f64>()
            && volts > 0.0
        {
            return Some(Voltage { volts, ac });
        }
    }
    None
}

/// Reads the words of `row` with `words`.
pub fn read_row(row: &Row, words: &WordList) -> Reading {
    let tokens = tokenize(&row.function);
    let hits = words.scan(&tokens);
    let mut r = Reading::default();
    let mut covered = vec![false; tokens.len()];
    let mut speed = None;
    for h in &hits {
        covered[h.start..h.start + h.len].fill(true);
        r.known = true;
        match &h.tag {
            Tag::Motion(m) => r.motions.push(m.clone()),
            Tag::Dir(d) => r.dirs.push(*d),
            Tag::Combo(m, d) => {
                r.motions.push(m.clone());
                r.dirs.push(*d);
            }
            Tag::Pair(m) => r.pair = r.pair.clone().or(Some(m.clone())),
            Tag::Speed(n) => speed = speed.or(Some(*n)),
            Tag::Role(role) => r.role = r.role.or(Some(*role)),
            Tag::Kind(k) => r.kind = r.kind.clone().or(Some(k.clone())),
        }
    }
    r.motions.dedup();
    r.dirs.dedup();
    // "main hoist" names a hoist, not the main contactor.
    if !r.motions.is_empty() && r.role == Some(Role::Main) {
        r.role = None;
    }
    if r.role.is_none()
        && matches!(
            row.terminal.trim().to_lowercase().as_str(),
            "com" | "common"
        )
        && r.motions.is_empty()
    {
        r.role = Some(Role::Common);
        r.known = true;
    }
    r.voltage = voltage_from(&tokens);

    // Step: the step column wins; then speed words; then 1S, S2, "step 2".
    let cell = row.step.trim();
    let mut column_step = None;
    if !cell.is_empty() {
        column_step = parse_step_cell(cell);
        if column_step.is_none() {
            let cell_tokens = tokenize(cell);
            column_step = words.scan(&cell_tokens).iter().find_map(|h| match h.tag {
                Tag::Speed(n) => Some(n),
                _ => None,
            });
        }
        r.bad_step_cell = column_step.is_none();
    }
    let mut text_step = speed;
    for (i, t) in tokens.iter().enumerate() {
        if covered[i] || text_step.is_some() {
            continue;
        }
        if let Some(n) = step_from_token(t) {
            text_step = Some(n);
            continue;
        }
        if t.len() == 1 && t.chars().all(|c| c.is_ascii_digit()) {
            let near_word = |j: Option<usize>| {
                j.and_then(|j| tokens.get(j)).is_some_and(|w| {
                    matches!(w.as_str(), "step" | "stap" | "stand" | "notch" | "speed")
                })
            };
            if near_word(i.checked_sub(1)) || near_word(Some(i + 1)) {
                text_step = t.parse().ok().filter(|n| (1..=5).contains(n));
            } else {
                r.loose_number = Some(t.clone());
            }
        }
    }
    r.step_conflict = matches!((column_step, text_step), (Some(a), Some(b)) if a != b);
    r.step = column_step.or(text_step);
    if r.step.is_some() {
        r.loose_number = None;
    }
    r.known |= r.step.is_some() || r.voltage.is_some();
    r
}

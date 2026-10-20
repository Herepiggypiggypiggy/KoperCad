//! Tokens, estimated cost and the monthly budget cap.

use crate::models::{self, CACHE_WRITE_FACTOR};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Token counts of one response (the Messages API `usage` object).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
}

impl Usage {
    pub fn add(&mut self, o: &Usage) {
        self.input_tokens += o.input_tokens;
        self.output_tokens += o.output_tokens;
        self.cache_read_input_tokens += o.cache_read_input_tokens;
        self.cache_creation_input_tokens += o.cache_creation_input_tokens;
    }

    pub fn total(&self) -> u64 {
        self.input_tokens
            + self.output_tokens
            + self.cache_read_input_tokens
            + self.cache_creation_input_tokens
    }

    /// Estimated cost in USD at the model's list prices. An unknown model id
    /// is priced as `fallback_model`.
    pub fn cost_usd(&self, model: &str, fallback_model: &str) -> f64 {
        let m = models::find(model).unwrap_or_else(|| models::resolve(fallback_model));
        let prompt =
            self.input_tokens + self.cache_read_input_tokens + self.cache_creation_input_tokens;
        let long = m.long_prompt_tokens > 0 && prompt > m.long_prompt_tokens;
        let input = if long {
            m.long_input.unwrap_or(m.input)
        } else {
            m.input
        };
        let output = if long {
            m.long_output.unwrap_or(m.output)
        } else {
            m.output
        };
        let per = 1.0 / 1_000_000.0;
        self.input_tokens as f64 * input * per
            + self.output_tokens as f64 * output * per
            + self.cache_read_input_tokens as f64 * m.cache_read * per
            + self.cache_creation_input_tokens as f64 * input * CACHE_WRITE_FACTOR * per
    }
}

/// What this month has used, kept on this computer.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ledger {
    /// `YYYY-MM`; a new month starts from zero.
    pub month: String,
    pub requests: u64,
    pub tokens: Usage,
    pub cost_usd: f64,
}

impl Ledger {
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, serde_json::to_string_pretty(self).unwrap_or_default())
    }

    /// Records one response in `month` (`YYYY-MM`), resetting at a month change.
    pub fn record(&mut self, month: &str, usage: &Usage, cost_usd: f64) {
        if self.month != month {
            *self = Self {
                month: month.to_string(),
                ..Self::default()
            };
        }
        self.requests += 1;
        self.tokens.add(usage);
        self.cost_usd += cost_usd;
    }

    /// Spend counted for `month`.
    pub fn spent_in(&self, month: &str) -> f64 {
        if self.month == month {
            self.cost_usd
        } else {
            0.0
        }
    }
}

/// The cap the person set; 0 means no cap.
pub fn over_budget(spent_usd: f64, cap_usd: f64) -> bool {
    cap_usd > 0.0 && spent_usd >= cap_usd
}

/// `$0.0123`-style text: four decimals under a cent, else two.
pub fn format_usd(v: f64) -> String {
    if v < 0.1 && v > 0.0 {
        format!("${v:.4}")
    } else {
        format!("${v:.2}")
    }
}

/// `YYYY-MM` for a Unix time (UTC), without a date library.
pub fn month_of(unix_secs: u64) -> String {
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = (unix_secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_follows_the_list_prices() {
        let u = Usage {
            input_tokens: 1_000_000,
            output_tokens: 100_000,
            cache_read_input_tokens: 1_000_000,
            cache_creation_input_tokens: 0,
        };
        // Opus 5.5: $4 in, $20 out, $0.20 cache read.
        let c = u.cost_usd("claude-opus-5-5", "claude-opus-5-5");
        assert!((c - (4.0 + 2.0 + 0.2)).abs() < 1e-9, "{c}");
    }

    #[test]
    fn cache_writes_cost_a_quarter_more_than_input() {
        let u = Usage {
            cache_creation_input_tokens: 1_000_000,
            ..Usage::default()
        };
        let c = u.cost_usd("claude-sonnet-5-5", "claude-sonnet-5-5");
        assert!((c - 2.5).abs() < 1e-9);
    }

    #[test]
    fn haiku_prices_step_up_for_a_long_prompt() {
        let short = Usage {
            input_tokens: 100_000,
            ..Usage::default()
        };
        let long = Usage {
            input_tokens: 100_001,
            ..Usage::default()
        };
        let a = short.cost_usd("claude-haiku-5-5", "");
        let b = long.cost_usd("claude-haiku-5-5", "");
        assert!((a - 0.01).abs() < 1e-9);
        assert!((b - 0.0500005).abs() < 1e-7);
    }

    #[test]
    fn an_unknown_model_is_priced_as_the_fallback() {
        let u = Usage {
            output_tokens: 1_000_000,
            ..Usage::default()
        };
        assert!((u.cost_usd("claude-future", "claude-sonnet-5-5") - 10.0).abs() < 1e-9);
    }

    #[test]
    fn the_ledger_resets_in_a_new_month_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        let mut l = Ledger::default();
        let u = Usage {
            input_tokens: 10,
            output_tokens: 5,
            ..Usage::default()
        };
        l.record("2026-10", &u, 0.5);
        l.record("2026-10", &u, 0.25);
        assert_eq!(l.requests, 2);
        assert!((l.spent_in("2026-10") - 0.75).abs() < 1e-9);
        assert_eq!(l.spent_in("2026-11"), 0.0);
        l.save(&path).unwrap();
        assert_eq!(Ledger::load(&path), l);
        l.record("2026-11", &u, 0.1);
        assert_eq!(l.requests, 1);
    }

    #[test]
    fn the_cap_zero_means_unlimited() {
        assert!(!over_budget(100.0, 0.0));
        assert!(over_budget(5.0, 5.0));
        assert!(!over_budget(4.99, 5.0));
    }

    #[test]
    fn months_come_from_unix_time() {
        assert_eq!(month_of(0), "1970-01");
        assert_eq!(month_of(1_792_000_000), "2026-10");
        assert_eq!(month_of(1_798_761_600), "2027-01");
    }

    #[test]
    fn money_formats_small_amounts_with_more_places() {
        assert_eq!(format_usd(0.0123), "$0.0123");
        assert_eq!(format_usd(1.5), "$1.50");
        assert_eq!(format_usd(0.0), "$0.00");
    }
}

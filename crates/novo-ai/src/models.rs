//! Models the assistant can use and what they cost.
//!
//! Prices are Anthropic's published list prices per million tokens, kept here
//! as a table and shown in the app as estimates: the bill on the person's
//! Anthropic account is the truth.

/// One selectable model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModelInfo {
    pub id: &'static str,
    pub label: &'static str,
    /// One line on when to pick it.
    pub note: &'static str,
    /// USD per million input tokens.
    pub input: f64,
    /// USD per million output tokens.
    pub output: f64,
    /// USD per million tokens read from the prompt cache.
    pub cache_read: f64,
    /// Input price per million above `long_prompt_tokens`, when it differs.
    pub long_input: Option<f64>,
    pub long_output: Option<f64>,
    /// Prompt size above which the long prices apply.
    pub long_prompt_tokens: u64,
    /// Whether the server-side refusal fallback may be requested.
    pub refusal_fallback: bool,
}

/// Cache writes cost this multiple of the input price (five-minute cache).
pub const CACHE_WRITE_FACTOR: f64 = 1.25;

pub const DEFAULT_MODEL: &str = "claude-opus-5-5";

pub const MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "claude-opus-5-5",
        label: "Claude Opus 5.5",
        note: "Most careful; best for design review and multi-step edits",
        input: 4.0,
        output: 20.0,
        cache_read: 0.20,
        long_input: None,
        long_output: None,
        long_prompt_tokens: 0,
        refusal_fallback: true,
    },
    ModelInfo {
        id: "claude-sonnet-5-5",
        label: "Claude Sonnet 5.5",
        note: "Faster and about half the price; good for everyday questions",
        input: 2.0,
        output: 10.0,
        cache_read: 0.20,
        long_input: None,
        long_output: None,
        long_prompt_tokens: 0,
        refusal_fallback: true,
    },
    ModelInfo {
        id: "claude-haiku-5-5",
        label: "Claude Haiku 5.5",
        note: "Cheapest; quick lookups and simple questions",
        input: 0.10,
        output: 0.50,
        cache_read: 0.01,
        long_input: Some(0.50),
        long_output: Some(2.50),
        long_prompt_tokens: 100_000,
        refusal_fallback: false,
    },
    ModelInfo {
        id: "claude-fable-5-1",
        label: "Claude Fable 5.1",
        note: "Most capable and most expensive; long hard problems only",
        input: 10.0,
        output: 50.0,
        cache_read: 0.25,
        long_input: None,
        long_output: None,
        long_prompt_tokens: 0,
        refusal_fallback: true,
    },
];

/// The model with this id, if it is one of ours.
pub fn find(id: &str) -> Option<&'static ModelInfo> {
    MODELS.iter().find(|m| m.id == id)
}

/// The model to use for a stored id: unknown or empty ids fall back to the default.
pub fn resolve(id: &str) -> &'static ModelInfo {
    find(id).unwrap_or_else(|| find(DEFAULT_MODEL).expect("default model is in the table"))
}

//! The optional Claude integration.
//!
//! Everything here is off until the person turns it on, uses their own
//! Anthropic API key and sends nothing they have not agreed to. The crate has
//! no UI and no network use in its tests: the HTTP call sits behind
//! [`api::Transport`], and a scripted transport drives every test.
//!
//! - [`models`]: the model list with prices (labelled as list prices).
//! - [`key`]: where the API key lives on this computer.
//! - [`consent`]: which project data may be sent, and what was agreed.
//! - [`usage`]: tokens, estimated cost and the monthly budget cap.
//! - [`api`]: the Messages API request and response.
//! - [`tools`]: what the assistant may look up and propose.
//! - [`agent`]: one assistant turn, with the tool loop.

pub mod agent;
pub mod api;
pub mod config;
pub mod consent;
pub mod datasheet;
pub mod engineer;
pub mod eval;
pub mod eval_cli;
pub mod image;
pub mod key;
pub mod models;
pub mod stream;
pub mod tools;
pub mod usage;

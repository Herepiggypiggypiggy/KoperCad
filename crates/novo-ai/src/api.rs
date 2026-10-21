//! The Messages API: request, response and errors.
//!
//! The HTTP call itself is behind [`Transport`], so tests script the replies
//! and never touch the network. [`UreqTransport`] is the real one (HTTPS with
//! rustls, no system TLS or admin rights).

use crate::key::ApiKey;
use crate::models::ModelInfo;
use crate::usage::Usage;
use serde_json::{Value, json};
use std::time::Duration;

pub const BASE_URL: &str = "https://api.anthropic.com";
pub const API_VERSION: &str = "2023-06-01";
/// Beta that lets the server retry a refused request on another model.
pub const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
/// Output ceiling per response. Kept under the size that needs streaming.
pub const MAX_TOKENS: u32 = 16_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

#[derive(Clone, Debug)]
pub struct HttpRequest {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

#[derive(Clone, Debug, Default)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl HttpResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Sends one HTTP request. `Err` is a network failure; HTTP error statuses
/// come back as a response.
pub trait Transport: Send {
    fn send(&mut self, req: &HttpRequest) -> Result<HttpResponse, String>;
}

impl<T: Transport + ?Sized> Transport for Box<T> {
    fn send(&mut self, req: &HttpRequest) -> Result<HttpResponse, String> {
        (**self).send(req)
    }
}

/// The real HTTPS transport.
pub struct UreqTransport {
    agent: ureq::Agent,
}

impl Default for UreqTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl UreqTransport {
    pub fn new() -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(15)))
            // A long answer with thinking can take minutes.
            .timeout_global(Some(Duration::from_secs(600)))
            .build()
            .into();
        Self { agent }
    }
}

impl Transport for UreqTransport {
    fn send(&mut self, req: &HttpRequest) -> Result<HttpResponse, String> {
        let result = match req.method {
            Method::Get => {
                let mut r = self.agent.get(&req.url);
                for (k, v) in &req.headers {
                    r = r.header(k, v);
                }
                r.call()
            }
            Method::Post => {
                let mut r = self.agent.post(&req.url);
                for (k, v) in &req.headers {
                    r = r.header(k, v);
                }
                r.send(req.body.as_bytes())
            }
        };
        let mut resp = result.map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let headers = resp
            .headers()
            .iter()
            .filter_map(|(k, v)| Some((k.as_str().to_string(), v.to_str().ok()?.to_string())))
            .collect();
        let body = resp
            .body_mut()
            .read_to_string()
            .map_err(|e| e.to_string())?;
        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ApiError {
    #[error("No internet connection to Anthropic: {0}")]
    Network(String),
    #[error("Anthropic did not accept the API key. Check it in Settings > Claude.")]
    Auth,
    #[error("The API key's account has no access to this model or feature: {0}")]
    Permission(String),
    #[error("Rate limited by Anthropic. Try again in a minute.")]
    RateLimited,
    #[error("Anthropic is overloaded right now. Try again in a minute.")]
    Overloaded,
    #[error("The request was not valid: {0}")]
    BadRequest(String),
    #[error("Anthropic had a problem (status {0}): {1}")]
    Server(u16, String),
    #[error("The reply could not be read: {0}")]
    Parse(String),
    #[error("Claude declined this request{0}.")]
    Refused(String),
    #[error("The monthly budget you set is used up. Raise it in Settings > Claude to continue.")]
    BudgetReached,
}

impl ApiError {
    /// Worth waiting and sending again.
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::RateLimited | Self::Overloaded | Self::Server(500..=599, _)
        )
    }
}

fn error_message(body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| body.chars().take(200).collect())
}

/// One answer from the model.
#[derive(Clone, Debug, PartialEq)]
pub struct Reply {
    /// The content blocks exactly as received. They go back unchanged into
    /// the conversation (thinking blocks must not be edited).
    pub content: Vec<Value>,
    pub stop_reason: String,
    pub usage: Usage,
    /// The model that actually answered (a fallback can differ from the request).
    pub model: String,
}

impl Reply {
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter(|b| b["type"] == "text")
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// `(id, name, input)` of each tool call.
    pub fn tool_uses(&self) -> Vec<(String, String, Value)> {
        self.content
            .iter()
            .filter(|b| b["type"] == "tool_use")
            .map(|b| {
                (
                    b["id"].as_str().unwrap_or_default().to_string(),
                    b["name"].as_str().unwrap_or_default().to_string(),
                    b["input"].clone(),
                )
            })
            .collect()
    }
}

/// What one request is made of.
pub struct RequestSpec<'a> {
    pub model: &'a ModelInfo,
    pub effort: &'a str,
    pub system: &'a str,
    pub messages: &'a [Value],
    pub tools: &'a [Value],
}

impl RequestSpec<'_> {
    pub fn body(&self) -> Value {
        let mut b = json!({
            "model": self.model.id,
            "max_tokens": MAX_TOKENS,
            "system": self.system,
            "messages": self.messages,
            "thinking": {"type": "adaptive"},
            "output_config": {"effort": self.effort},
            // Caches the growing conversation prefix between turns.
            "cache_control": {"type": "ephemeral"},
        });
        if !self.tools.is_empty() {
            b["tools"] = Value::Array(self.tools.to_vec());
        }
        if self.model.refusal_fallback {
            b["fallbacks"] = json!("default");
        }
        b
    }
}

/// Sends requests with the person's key.
pub struct Client<T: Transport> {
    transport: T,
    key: ApiKey,
    base_url: String,
    sleep: Box<dyn FnMut(Duration) + Send>,
}

const MAX_ATTEMPTS: u32 = 3;

impl<T: Transport> Client<T> {
    pub fn new(transport: T, key: ApiKey) -> Self {
        Self {
            transport,
            key,
            base_url: BASE_URL.to_string(),
            sleep: Box::new(std::thread::sleep),
        }
    }

    /// Replaces the wait between retries (tests skip it).
    pub fn with_sleep(mut self, sleep: impl FnMut(Duration) + Send + 'static) -> Self {
        self.sleep = Box::new(sleep);
        self
    }

    fn headers(&self, fallback: bool) -> Vec<(String, String)> {
        let mut h = vec![
            ("x-api-key".to_string(), self.key.expose().to_string()),
            ("anthropic-version".to_string(), API_VERSION.to_string()),
            ("content-type".to_string(), "application/json".to_string()),
        ];
        if fallback {
            h.push(("anthropic-beta".to_string(), FALLBACK_BETA.to_string()));
        }
        h
    }

    fn send_checked(&mut self, req: &HttpRequest) -> Result<HttpResponse, ApiError> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            let result: Result<HttpResponse, ApiError> = match self.transport.send(req) {
                Err(e) => Err(ApiError::Network(e)),
                Ok(r) => match r.status {
                    200..=299 => return Ok(r),
                    401 => Err(ApiError::Auth),
                    403 => Err(ApiError::Permission(error_message(&r.body))),
                    429 => Err(ApiError::RateLimited),
                    529 => Err(ApiError::Overloaded),
                    400 | 404 | 413 | 422 => Err(ApiError::BadRequest(error_message(&r.body))),
                    s => Err(ApiError::Server(s, error_message(&r.body))),
                },
            };
            let Err(err) = result else { unreachable!() };
            if attempt >= MAX_ATTEMPTS || !(err.retryable() || matches!(err, ApiError::Network(_)))
            {
                return Err(err);
            }
            (self.sleep)(Duration::from_secs(2u64.pow(attempt)));
        }
    }

    /// Checks the key without spending anything: lists one model.
    pub fn check_key(&mut self) -> Result<(), ApiError> {
        let req = HttpRequest {
            method: Method::Get,
            url: format!("{}/v1/models?limit=1", self.base_url),
            headers: self.headers(false),
            body: String::new(),
        };
        self.send_checked(&req).map(|_| ())
    }

    /// One Messages API call.
    pub fn messages(&mut self, spec: &RequestSpec) -> Result<Reply, ApiError> {
        let req = HttpRequest {
            method: Method::Post,
            url: format!("{}/v1/messages", self.base_url),
            headers: self.headers(spec.model.refusal_fallback),
            body: spec.body().to_string(),
        };
        let resp = self.send_checked(&req)?;
        parse_reply(&resp.body)
    }
}

pub fn parse_reply(body: &str) -> Result<Reply, ApiError> {
    let v: Value = serde_json::from_str(body).map_err(|e| ApiError::Parse(e.to_string()))?;
    let content = v["content"]
        .as_array()
        .cloned()
        .ok_or_else(|| ApiError::Parse("no content".into()))?;
    let usage: Usage = serde_json::from_value(v["usage"].clone()).unwrap_or_default();
    let stop_reason = v["stop_reason"].as_str().unwrap_or("end_turn").to_string();
    let model = v["model"].as_str().unwrap_or_default().to_string();
    if stop_reason == "refusal" {
        let why = v["stop_details"]["explanation"]
            .as_str()
            .map(|e| format!(": {e}"))
            .unwrap_or_default();
        return Err(ApiError::Refused(why));
    }
    Ok(Reply {
        content,
        stop_reason,
        usage,
        model,
    })
}

#[cfg(test)]
pub mod testing {
    //! A scripted transport for tests in this crate and its users.
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    pub struct Script {
        pub replies: Arc<Mutex<VecDeque<Result<HttpResponse, String>>>>,
        pub seen: Arc<Mutex<Vec<HttpRequest>>>,
    }

    impl Script {
        pub fn push(&self, status: u16, body: Value) {
            self.replies.lock().unwrap().push_back(Ok(HttpResponse {
                status,
                headers: vec![],
                body: body.to_string(),
            }));
        }
        pub fn requests(&self) -> Vec<HttpRequest> {
            self.seen.lock().unwrap().clone()
        }
    }

    impl Transport for Script {
        fn send(&mut self, req: &HttpRequest) -> Result<HttpResponse, String> {
            self.seen.lock().unwrap().push(req.clone());
            self.replies
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Err("script ran out of replies".into()))
        }
    }

    pub fn key() -> ApiKey {
        ApiKey::parse("sk-ant-api03-test-key-0000000000").unwrap()
    }

    pub fn text_reply(model: &str, text: &str, input: u64, output: u64) -> Value {
        json!({
            "id": "msg_1", "type": "message", "role": "assistant", "model": model,
            "content": [{"type": "text", "text": text}],
            "stop_reason": "end_turn",
            "usage": {"input_tokens": input, "output_tokens": output}
        })
    }

    pub fn tool_reply(model: &str, id: &str, name: &str, input: Value) -> Value {
        json!({
            "id": "msg_2", "type": "message", "role": "assistant", "model": model,
            "content": [
                {"type": "thinking", "thinking": "", "signature": "sig"},
                {"type": "tool_use", "id": id, "name": name, "input": input}
            ],
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 100, "output_tokens": 20}
        })
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;
    use crate::models;

    fn client(s: &Script) -> Client<Script> {
        Client::new(s.clone(), key()).with_sleep(|_| {})
    }

    fn spec<'a>(msgs: &'a [Value], tools: &'a [Value], model: &'a ModelInfo) -> RequestSpec<'a> {
        RequestSpec {
            model,
            effort: "medium",
            system: "sys",
            messages: msgs,
            tools,
        }
    }

    #[test]
    fn the_request_carries_the_key_version_and_model_rules() {
        let s = Script::default();
        s.push(200, text_reply("claude-opus-5-5", "hi", 10, 2));
        let msgs = [json!({"role":"user","content":"hello"})];
        let m = models::resolve("claude-opus-5-5");
        let r = client(&s).messages(&spec(&msgs, &[], m)).unwrap();
        assert_eq!(r.text(), "hi");
        let req = &s.requests()[0];
        assert_eq!(req.url, "https://api.anthropic.com/v1/messages");
        let h = |n: &str| {
            req.headers
                .iter()
                .find(|(k, _)| k == n)
                .map(|(_, v)| v.clone())
        };
        assert_eq!(h("x-api-key").unwrap(), key().expose());
        assert_eq!(h("anthropic-version").unwrap(), "2023-06-01");
        assert_eq!(h("anthropic-beta").unwrap(), FALLBACK_BETA);
        let body: Value = serde_json::from_str(&req.body).unwrap();
        assert_eq!(body["model"], "claude-opus-5-5");
        assert_eq!(body["thinking"]["type"], "adaptive");
        assert_eq!(body["output_config"]["effort"], "medium");
        assert_eq!(body["fallbacks"], "default");
        // Removed on current models: sampling parameters, budget_tokens, forced tools.
        assert!(body.get("temperature").is_none());
        assert!(body["thinking"].get("budget_tokens").is_none());
        assert!(body.get("tool_choice").is_none());
    }

    #[test]
    fn haiku_requests_carry_no_fallback() {
        let s = Script::default();
        s.push(200, text_reply("claude-haiku-5-5", "ok", 1, 1));
        let msgs = [json!({"role":"user","content":"x"})];
        client(&s)
            .messages(&spec(&msgs, &[], models::resolve("claude-haiku-5-5")))
            .unwrap();
        let req = &s.requests()[0];
        assert!(!req.headers.iter().any(|(k, _)| k == "anthropic-beta"));
        assert!(!req.body.contains("fallbacks"));
    }

    #[test]
    fn status_codes_become_plain_errors() {
        for (status, want) in [
            (401, ApiError::Auth),
            (400, ApiError::BadRequest("bad".into())),
        ] {
            let s = Script::default();
            s.push(status, json!({"error": {"message": "bad"}}));
            let msgs = [json!({"role":"user","content":"x"})];
            let e = client(&s)
                .messages(&spec(&msgs, &[], models::resolve("")))
                .unwrap_err();
            assert_eq!(e, want);
            assert_eq!(s.requests().len(), 1, "not retried");
        }
    }

    #[test]
    fn rate_limits_and_overload_are_retried_then_succeed() {
        let s = Script::default();
        s.push(429, json!({"error": {"message": "slow down"}}));
        s.push(529, json!({"error": {"message": "busy"}}));
        s.push(200, text_reply("claude-opus-5-5", "done", 1, 1));
        let msgs = [json!({"role":"user","content":"x"})];
        let r = client(&s)
            .messages(&spec(&msgs, &[], models::resolve("")))
            .unwrap();
        assert_eq!(r.text(), "done");
        assert_eq!(s.requests().len(), 3);
    }

    #[test]
    fn retries_give_up_after_three_attempts() {
        let s = Script::default();
        for _ in 0..5 {
            s.push(500, json!({"error": {"message": "boom"}}));
        }
        let msgs = [json!({"role":"user","content":"x"})];
        let e = client(&s)
            .messages(&spec(&msgs, &[], models::resolve("")))
            .unwrap_err();
        assert!(matches!(e, ApiError::Server(500, _)));
        assert_eq!(s.requests().len(), 3);
    }

    #[test]
    fn a_dead_network_is_reported_not_panicked() {
        let s = Script::default();
        let msgs = [json!({"role":"user","content":"x"})];
        let e = client(&s)
            .messages(&spec(&msgs, &[], models::resolve("")))
            .unwrap_err();
        assert!(matches!(e, ApiError::Network(_)));
    }

    #[test]
    fn a_refusal_is_an_error_with_the_reason() {
        let s = Script::default();
        s.push(
            200,
            json!({"content": [], "stop_reason": "refusal", "model": "m",
                   "usage": {}, "stop_details": {"type":"refusal","explanation":"no"}}),
        );
        let msgs = [json!({"role":"user","content":"x"})];
        let e = client(&s)
            .messages(&spec(&msgs, &[], models::resolve("")))
            .unwrap_err();
        assert_eq!(e, ApiError::Refused(": no".into()));
    }

    #[test]
    fn checking_the_key_lists_models_and_costs_nothing() {
        let s = Script::default();
        s.push(200, json!({"data": []}));
        client(&s).check_key().unwrap();
        let req = &s.requests()[0];
        assert_eq!(req.method, Method::Get);
        assert!(req.url.ends_with("/v1/models?limit=1"));
    }

    #[test]
    fn tool_calls_are_read_from_the_reply_and_blocks_kept_verbatim() {
        let r = parse_reply(&tool_reply("m", "toolu_1", "x", json!({"a": 1})).to_string()).unwrap();
        assert_eq!(
            r.tool_uses(),
            vec![("toolu_1".into(), "x".into(), json!({"a": 1}))]
        );
        assert_eq!(r.content[0]["type"], "thinking");
        assert_eq!(r.content[0]["signature"], "sig");
    }
}

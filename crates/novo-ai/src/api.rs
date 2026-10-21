//! The Messages API: request, response and errors.
//!
//! The HTTP call itself is behind [`Transport`], so tests script the replies
//! and never touch the network. [`UreqTransport`] is the real one (HTTPS with
//! rustls, no system TLS or admin rights).

use crate::key::ApiKey;
use crate::models::ModelInfo;
use crate::stream::{Assembler, Delta};
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

    /// Like [`send`](Transport::send), but a successful body is handed over
    /// line by line as it arrives (`on_line` returns false to stop reading)
    /// and the returned `body` is empty. An error status returns its whole
    /// body, as `send` does.
    fn send_stream(
        &mut self,
        req: &HttpRequest,
        on_line: &mut dyn FnMut(&str) -> bool,
    ) -> Result<HttpResponse, String> {
        let mut r = self.send(req)?;
        if (200..300).contains(&r.status) {
            for line in std::mem::take(&mut r.body).lines() {
                if !on_line(line) {
                    break;
                }
            }
        }
        Ok(r)
    }
}

impl<T: Transport + ?Sized> Transport for Box<T> {
    fn send(&mut self, req: &HttpRequest) -> Result<HttpResponse, String> {
        (**self).send(req)
    }
    fn send_stream(
        &mut self,
        req: &HttpRequest,
        on_line: &mut dyn FnMut(&str) -> bool,
    ) -> Result<HttpResponse, String> {
        (**self).send_stream(req, on_line)
    }
}

/// Longest single line of a streamed reply that is accepted.
const MAX_LINE_BYTES: usize = 1024 * 1024;

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

impl UreqTransport {
    fn call(&self, req: &HttpRequest) -> Result<ureq::http::Response<ureq::Body>, String> {
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
        result.map_err(|e| e.to_string())
    }
}

fn head_of(resp: &ureq::http::Response<ureq::Body>) -> (u16, Vec<(String, String)>) {
    let headers = resp
        .headers()
        .iter()
        .filter_map(|(k, v)| Some((k.as_str().to_string(), v.to_str().ok()?.to_string())))
        .collect();
    (resp.status().as_u16(), headers)
}

impl Transport for UreqTransport {
    fn send(&mut self, req: &HttpRequest) -> Result<HttpResponse, String> {
        let mut resp = self.call(req)?;
        let (status, headers) = head_of(&resp);
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

    fn send_stream(
        &mut self,
        req: &HttpRequest,
        on_line: &mut dyn FnMut(&str) -> bool,
    ) -> Result<HttpResponse, String> {
        use std::io::{BufRead, BufReader, Read};
        let mut resp = self.call(req)?;
        let (status, headers) = head_of(&resp);
        if !(200..300).contains(&status) {
            let body = resp
                .body_mut()
                .read_to_string()
                .map_err(|e| e.to_string())?;
            return Ok(HttpResponse {
                status,
                headers,
                body,
            });
        }
        // Lines are read with a cap so a hostile or broken stream cannot
        // make one line grow without bound.
        let mut reader = BufReader::new(resp.body_mut().as_reader());
        loop {
            let mut raw = Vec::new();
            let n = (&mut reader)
                .take(MAX_LINE_BYTES as u64 + 1)
                .read_until(b'\n', &mut raw)
                .map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            if raw.len() > MAX_LINE_BYTES {
                return Err("a line of the reply is longer than allowed".into());
            }
            let line = String::from_utf8_lossy(&raw);
            if !on_line(line.trim_end_matches(['\r', '\n'])) {
                break;
            }
        }
        Ok(HttpResponse {
            status,
            headers,
            body: String::new(),
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
    #[error("Stopped.")]
    Cancelled(Usage),
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
        self.send_with(req, &|| false, &mut |t, r| t.send(r))
    }

    /// Sends with retries. `go` performs one attempt. `no_retry` is asked
    /// before a retry: after output started streaming a retry would repeat it.
    fn send_with(
        &mut self,
        req: &HttpRequest,
        no_retry: &dyn Fn() -> bool,
        go: &mut dyn FnMut(&mut T, &HttpRequest) -> Result<HttpResponse, String>,
    ) -> Result<HttpResponse, ApiError> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            let result: Result<HttpResponse, ApiError> = match go(&mut self.transport, req) {
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
            if attempt >= MAX_ATTEMPTS
                || no_retry()
                || !(err.retryable() || matches!(err, ApiError::Network(_)))
            {
                return Err(err);
            }
            (self.sleep)(Duration::from_secs(2u64.pow(attempt)));
        }
    }

    /// One Messages API call, streamed: `on_delta` hears the answer as it is
    /// written. `cancel` set to true stops reading; the error then carries
    /// the tokens counted so far.
    pub fn messages_stream(
        &mut self,
        spec: &RequestSpec,
        cancel: &std::sync::atomic::AtomicBool,
        on_delta: &mut dyn FnMut(Delta),
    ) -> Result<Reply, ApiError> {
        use std::cell::Cell;
        use std::sync::atomic::Ordering;
        let mut body = spec.body();
        body["stream"] = Value::Bool(true);
        let req = HttpRequest {
            method: Method::Post,
            url: format!("{}/v1/messages", self.base_url),
            headers: self.headers(spec.model.refusal_fallback),
            body: body.to_string(),
        };
        let mut asm = Assembler::default();
        let started = Cell::new(false);
        self.send_with(&req, &|| started.get(), &mut |t, r| {
            t.send_stream(r, &mut |line| {
                started.set(true);
                asm.feed_line(line, on_delta);
                !cancel.load(Ordering::Relaxed)
            })
        })?;
        if cancel.load(Ordering::Relaxed) {
            return Err(ApiError::Cancelled(asm.usage()));
        }
        asm.finish()
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

/// A scripted transport for the tests of this crate and the crates that use it.
#[doc(hidden)]
pub mod testing {
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

        /// Streams a scripted complete message as the events the API would send.
        fn send_stream(
            &mut self,
            req: &HttpRequest,
            on_line: &mut dyn FnMut(&str) -> bool,
        ) -> Result<HttpResponse, String> {
            let mut r = self.send(req)?;
            if (200..300).contains(&r.status) {
                let body = std::mem::take(&mut r.body);
                let sse = match serde_json::from_str::<Value>(&body) {
                    Ok(m) if m["content"].is_array() => to_sse(&m),
                    _ => body,
                };
                for line in sse.lines() {
                    if !on_line(line) {
                        break;
                    }
                }
            }
            Ok(r)
        }
    }

    /// A complete message as server-sent events, text and tool input split
    /// in two pieces each so a reader has to join them.
    pub fn to_sse(m: &Value) -> String {
        fn ev(out: &mut String, v: Value) {
            out.push_str(&format!(
                "event: {}\ndata: {}\n\n",
                v["type"].as_str().unwrap(),
                v
            ));
        }
        fn halves(s: &str) -> (String, String) {
            let mid = s
                .char_indices()
                .nth(s.chars().count() / 2)
                .map_or(s.len(), |(i, _)| i);
            (s[..mid].to_string(), s[mid..].to_string())
        }
        let mut out = String::new();
        let usage = &m["usage"];
        ev(
            &mut out,
            json!({"type": "message_start", "message": {
            "id": "msg_1", "type": "message", "role": "assistant", "content": [],
            "model": m["model"],
            "usage": {"input_tokens": usage["input_tokens"].as_u64().unwrap_or(0),
                      "output_tokens": 1,
                      "cache_read_input_tokens": usage["cache_read_input_tokens"].as_u64().unwrap_or(0),
                      "cache_creation_input_tokens": usage["cache_creation_input_tokens"].as_u64().unwrap_or(0)}}}),
        );
        for (i, b) in m["content"].as_array().unwrap().iter().enumerate() {
            let kind = b["type"].as_str().unwrap();
            let start = match kind {
                "text" => json!({"type": "text", "text": ""}),
                "thinking" => json!({"type": "thinking", "thinking": "", "signature": ""}),
                _ => json!({"type": "tool_use", "id": b["id"], "name": b["name"], "input": {}}),
            };
            ev(
                &mut out,
                json!({"type": "content_block_start", "index": i, "content_block": start}),
            );
            let mut delta = |d: Value| {
                ev(
                    &mut out,
                    json!({"type": "content_block_delta", "index": i, "delta": d}),
                );
            };
            match kind {
                "text" => {
                    let (a, c) = halves(b["text"].as_str().unwrap());
                    delta(json!({"type": "text_delta", "text": a}));
                    delta(json!({"type": "text_delta", "text": c}));
                }
                "thinking" => {
                    let t = b["thinking"].as_str().unwrap_or_default();
                    if !t.is_empty() {
                        delta(json!({"type": "thinking_delta", "thinking": t}));
                    }
                    delta(json!({"type": "signature_delta", "signature": b["signature"]}));
                }
                _ => {
                    let (a, c) = halves(&b["input"].to_string());
                    delta(json!({"type": "input_json_delta", "partial_json": a}));
                    delta(json!({"type": "input_json_delta", "partial_json": c}));
                }
            }
            ev(&mut out, json!({"type": "content_block_stop", "index": i}));
        }
        let mut md = json!({"type": "message_delta",
            "delta": {"stop_reason": m["stop_reason"]},
            "usage": {"output_tokens": usage["output_tokens"].as_u64().unwrap_or(0)}});
        if !m["stop_details"].is_null() {
            md["delta"]["stop_details"] = m["stop_details"].clone();
        }
        ev(&mut out, md);
        ev(&mut out, json!({"type": "message_stop"}));
        out
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

    #[test]
    fn a_streamed_request_sets_stream_and_retries_only_before_output_starts() {
        let s = Script::default();
        s.push(529, json!({"error": {"message": "busy"}}));
        s.push(200, text_reply("claude-opus-5-5", "late but whole", 1, 1));
        let msgs = [json!({"role":"user","content":"x"})];
        let mut got = String::new();
        let r = client(&s)
            .messages_stream(
                &spec(&msgs, &[], models::resolve("")),
                &std::sync::atomic::AtomicBool::new(false),
                &mut |d| {
                    if let crate::stream::Delta::Text(t) = d {
                        got.push_str(&t)
                    }
                },
            )
            .unwrap();
        assert_eq!(r.text(), "late but whole");
        assert_eq!(got, "late but whole");
        assert_eq!(s.requests().len(), 2);
        assert!(s.requests()[1].body.contains("\"stream\":true"));
    }
}

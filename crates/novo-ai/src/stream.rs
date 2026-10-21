//! Reads a streamed Messages API reply (server-sent events) back into the
//! same [`Reply`] a plain request gives, handing text to the caller as it
//! arrives.

use crate::api::{ApiError, Reply};
use crate::usage::Usage;
use serde_json::{Value, json};

/// Something worth showing while the reply is still coming.
#[derive(Clone, Debug, PartialEq)]
pub enum Delta {
    /// More of the answer text.
    Text(String),
    /// The model is reasoning (its reasoning is not shown).
    Thinking,
}

/// Builds a [`Reply`] from event lines fed one at a time.
#[derive(Default)]
pub struct Assembler {
    blocks: Vec<Value>,
    json: Vec<String>,
    model: String,
    usage: Usage,
    stop_reason: Option<String>,
    stop_details: Value,
    error: Option<ApiError>,
    stopped: bool,
}

impl Assembler {
    /// Tokens counted so far (the input side is known from the first event).
    pub fn usage(&self) -> Usage {
        self.usage
    }

    /// One line of the event stream. Lines other than `data:` carry nothing
    /// we need: each data payload names its own event type.
    pub fn feed_line(&mut self, line: &str, on_delta: &mut dyn FnMut(Delta)) {
        let Some(data) = line.strip_prefix("data:") else {
            return;
        };
        let Ok(ev) = serde_json::from_str::<Value>(data.trim()) else {
            return;
        };
        let index = ev["index"].as_u64().unwrap_or(0) as usize;
        match ev["type"].as_str().unwrap_or_default() {
            "message_start" => {
                self.model = ev["message"]["model"].as_str().unwrap_or_default().into();
                self.set_usage(&ev["message"]["usage"]);
            }
            "content_block_start" => {
                if self.blocks.len() <= index {
                    self.blocks.resize(index + 1, Value::Null);
                    self.json.resize(index + 1, String::new());
                }
                if ev["content_block"]["type"] == "thinking" {
                    on_delta(Delta::Thinking);
                }
                self.blocks[index] = ev["content_block"].clone();
            }
            "content_block_delta" => self.delta(index, &ev["delta"], on_delta),
            "content_block_stop" => self.close(index),
            "message_delta" => {
                if let Some(r) = ev["delta"]["stop_reason"].as_str() {
                    self.stop_reason = Some(r.to_string());
                }
                if !ev["delta"]["stop_details"].is_null() {
                    self.stop_details = ev["delta"]["stop_details"].clone();
                }
                self.set_usage(&ev["usage"]);
            }
            "message_stop" => self.stopped = true,
            "error" => {
                let msg = ev["error"]["message"].as_str().unwrap_or("stream error");
                self.error = Some(match ev["error"]["type"].as_str().unwrap_or_default() {
                    "overloaded_error" => ApiError::Overloaded,
                    "rate_limit_error" => ApiError::RateLimited,
                    "authentication_error" => ApiError::Auth,
                    _ => ApiError::Server(0, msg.to_string()),
                });
            }
            _ => {}
        }
    }

    fn set_usage(&mut self, u: &Value) {
        let put = |key: &str, slot: &mut u64| {
            if let Some(n) = u[key].as_u64() {
                *slot = n;
            }
        };
        put("input_tokens", &mut self.usage.input_tokens);
        put("output_tokens", &mut self.usage.output_tokens);
        put(
            "cache_read_input_tokens",
            &mut self.usage.cache_read_input_tokens,
        );
        put(
            "cache_creation_input_tokens",
            &mut self.usage.cache_creation_input_tokens,
        );
    }

    fn delta(&mut self, index: usize, d: &Value, on_delta: &mut dyn FnMut(Delta)) {
        let Some(block) = self.blocks.get_mut(index) else {
            return;
        };
        let append = |block: &mut Value, key: &str, extra: &str| {
            let mut s = block[key].as_str().unwrap_or_default().to_string();
            s.push_str(extra);
            block[key] = Value::String(s);
        };
        match d["type"].as_str().unwrap_or_default() {
            "text_delta" => {
                let t = d["text"].as_str().unwrap_or_default();
                append(block, "text", t);
                if !t.is_empty() {
                    on_delta(Delta::Text(t.to_string()));
                }
            }
            "thinking_delta" => append(
                block,
                "thinking",
                d["thinking"].as_str().unwrap_or_default(),
            ),
            "signature_delta" => append(
                block,
                "signature",
                d["signature"].as_str().unwrap_or_default(),
            ),
            "input_json_delta" => {
                if let Some(buf) = self.json.get_mut(index) {
                    buf.push_str(d["partial_json"].as_str().unwrap_or_default());
                }
            }
            _ => {}
        }
    }

    fn close(&mut self, index: usize) {
        let Some(buf) = self.json.get_mut(index) else {
            return;
        };
        if self.blocks[index]["type"] != "tool_use" {
            return;
        }
        let text = std::mem::take(buf);
        match if text.trim().is_empty() {
            Ok(json!({}))
        } else {
            serde_json::from_str::<Value>(&text)
        } {
            Ok(v) => self.blocks[index]["input"] = v,
            Err(e) => self.error = Some(ApiError::Parse(format!("tool input: {e}"))),
        }
    }

    /// The finished reply, or why there is none.
    pub fn finish(self) -> Result<Reply, ApiError> {
        if let Some(e) = self.error {
            return Err(e);
        }
        if !self.stopped {
            return Err(ApiError::Network(
                "the reply stopped before it was complete".into(),
            ));
        }
        let stop_reason = self.stop_reason.unwrap_or_else(|| "end_turn".into());
        if stop_reason == "refusal" {
            let why = self.stop_details["explanation"]
                .as_str()
                .map(|e| format!(": {e}"))
                .unwrap_or_default();
            return Err(ApiError::Refused(why));
        }
        Ok(Reply {
            content: self.blocks.into_iter().filter(|b| !b.is_null()).collect(),
            stop_reason,
            usage: self.usage,
            model: self.model,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::testing::to_sse;

    fn run(sse: &str) -> (Result<Reply, ApiError>, Vec<Delta>) {
        let mut a = Assembler::default();
        let mut deltas = vec![];
        for line in sse.lines() {
            a.feed_line(line, &mut |d| deltas.push(d));
        }
        (a.finish(), deltas)
    }

    #[test]
    fn text_arrives_in_pieces_and_joins_into_the_reply() {
        let msg = json!({"model": "m", "stop_reason": "end_turn",
            "content": [{"type": "text", "text": "Hello there"}],
            "usage": {"input_tokens": 10, "output_tokens": 4}});
        let (r, deltas) = run(&to_sse(&msg));
        let r = r.unwrap();
        assert_eq!(r.text(), "Hello there");
        assert!(deltas.len() >= 2, "{deltas:?}");
        let joined: String = deltas
            .iter()
            .filter_map(|d| {
                if let Delta::Text(t) = d {
                    Some(t.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(joined, "Hello there");
        assert_eq!((r.usage.input_tokens, r.usage.output_tokens), (10, 4));
        assert_eq!(r.model, "m");
    }

    #[test]
    fn thinking_and_tool_use_blocks_are_rebuilt_exactly() {
        let msg = json!({"model": "m", "stop_reason": "tool_use",
            "content": [
                {"type": "thinking", "thinking": "hmm", "signature": "sigABC"},
                {"type": "tool_use", "id": "toolu_1", "name": "read_project",
                 "input": {"section": "devices", "limit": 5}}],
            "usage": {"input_tokens": 1, "output_tokens": 2}});
        let (r, deltas) = run(&to_sse(&msg));
        let r = r.unwrap();
        assert_eq!(r.content[0]["signature"], "sigABC");
        assert_eq!(r.content[0]["thinking"], "hmm");
        assert_eq!(
            r.content[1]["input"],
            json!({"section": "devices", "limit": 5})
        );
        assert_eq!(r.stop_reason, "tool_use");
        assert!(deltas.contains(&Delta::Thinking));
    }

    #[test]
    fn a_tool_with_no_arguments_gets_an_empty_object() {
        let msg = json!({"model": "m", "stop_reason": "tool_use",
            "content": [{"type": "tool_use", "id": "t", "name": "project_outline", "input": {}}],
            "usage": {}});
        let r = run(&to_sse(&msg)).0.unwrap();
        assert_eq!(r.content[0]["input"], json!({}));
    }

    #[test]
    fn a_cut_stream_is_an_error_not_a_short_answer() {
        let msg = json!({"model": "m", "stop_reason": "end_turn",
            "content": [{"type": "text", "text": "abc def"}], "usage": {}});
        let sse = to_sse(&msg);
        let cut: String = sse.lines().take(6).collect::<Vec<_>>().join("\n");
        assert!(matches!(run(&cut).0, Err(ApiError::Network(_))));
    }

    #[test]
    fn an_error_event_maps_to_the_plain_error() {
        let sse = "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"x\"}}\n";
        assert_eq!(run(sse).0.unwrap_err(), ApiError::Overloaded);
    }

    #[test]
    fn a_refusal_is_reported_with_its_reason() {
        let mut msg = json!({"model": "m", "stop_reason": "refusal", "content": [], "usage": {}});
        msg["stop_details"] = json!({"explanation": "no"});
        assert_eq!(
            run(&to_sse(&msg)).0.unwrap_err(),
            ApiError::Refused(": no".into())
        );
    }

    #[test]
    fn pings_and_unknown_events_are_ignored() {
        let msg = json!({"model": "m", "stop_reason": "end_turn",
            "content": [{"type": "text", "text": "ok"}], "usage": {}});
        let sse = format!(
            "event: ping\ndata: {{\"type\":\"ping\"}}\n\n{}",
            to_sse(&msg)
        );
        assert_eq!(run(&sse).0.unwrap().text(), "ok");
    }
}

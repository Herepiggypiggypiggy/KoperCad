//! Reading a part's datasheet (a PDF the person chooses) into a draft part.
//!
//! The PDF goes to Claude as a document block, nothing else from the project
//! is sent. Claude answers through one tool, `report_part`, with each value
//! next to the page and the exact words it came from. The result is only a
//! [`Draft`]: the person checks every field against the datasheet and ticks
//! what to keep before anything is added to the project.

use serde_json::{Value, json};

use crate::agent::SYSTEM_DATASHEET;
use crate::api::{ApiError, Client, RequestSpec, Transport};
use crate::config::AiSettings;
use crate::image::base64;
use crate::models;
use crate::usage::{Usage, over_budget};

/// Largest datasheet accepted (the API takes up to 32 MB per request).
pub const MAX_PDF_BYTES: usize = 20 * 1024 * 1024;

/// What a field holds.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldValue {
    Text(String),
    Number(f64),
}

/// One value Claude read, with where it says so.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    /// The part property key (`width_mm`) or `manufacturer`, `order_number`,
    /// `type_number`, `description` for the part itself.
    pub key: &'static str,
    pub label: &'static str,
    /// The unit shown beside a number, empty for text.
    pub unit: &'static str,
    pub value: FieldValue,
    pub page: Option<u32>,
    /// The words of the datasheet this came from.
    pub quote: String,
}

/// What came back, to be checked by the person.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Draft {
    pub fields: Vec<Field>,
    /// Claude's note, for example that a value was not on the sheet.
    pub notes: String,
    /// Fields Claude sent that were refused, and why.
    pub refused: Vec<String>,
}

impl Draft {
    pub fn get(&self, key: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.key == key)
    }
}

/// The part's own fields and then the properties Claude may report:
/// (key, label, unit, largest plausible value; 0 for text).
const FIELDS: &[(&str, &str, &str, f64)] = &[
    ("manufacturer", "Manufacturer", "", 0.0),
    ("order_number", "Order number", "", 0.0),
    ("type_number", "Type number", "", 0.0),
    ("description", "Description", "", 0.0),
    ("width_mm", "Width", "mm", 5_000.0),
    ("height_mm", "Height", "mm", 5_000.0),
    ("depth_mm", "Depth", "mm", 5_000.0),
    ("rated_current_a", "Rated current", "A", 20_000.0),
    ("rated_voltage_v", "Rated voltage", "V", 100_000.0),
    ("power_loss_w", "Power loss", "W", 100_000.0),
    ("max_ambient_c", "Highest ambient temperature", "°C", 200.0),
];

const MAX_TEXT: usize = 200;
const MAX_DESCRIPTION: usize = 500;
const MAX_QUOTE: usize = 400;

fn static_str(s: &str) -> Option<(&'static str, &'static str, &'static str, f64)> {
    FIELDS
        .iter()
        .find(|f| f.0 == s)
        .map(|f| (f.0, f.1, f.2, f.3))
}

/// The one tool Claude answers through.
pub fn tool() -> Value {
    let mut props = serde_json::Map::new();
    for (key, label, unit, _) in FIELDS {
        let numeric = !unit.is_empty();
        props.insert(
            (*key).into(),
            json!({
                "type": "object",
                "description": format!("{label}{}. Leave this out when the datasheet does not say.",
                    if numeric { format!(" in {unit}") } else { String::new() }),
                "properties": {
                    "value": {"type": if numeric { "number" } else { "string" }},
                    "page": {"type": "integer", "description": "page of the PDF, from 1"},
                    "quote": {"type": "string",
                              "description": "the exact words or table cell the value comes from"}
                },
                "required": ["value", "quote"]
            }),
        );
    }
    json!({
        "name": "report_part",
        "description": "Reports what the datasheet says about the part. Call it once. \
                        Only report values the datasheet states; never estimate or convert \
                        from memory.",
        "input_schema": {
            "type": "object",
            "properties": {
                "fields": {"type": "object", "properties": props},
                "notes": {"type": "string",
                          "description": "anything the person should check, such as a table \
                                          with several variants"}
            },
            "required": ["fields"]
        }
    })
}

/// Turns the tool input into a checked draft. Unknown keys are ignored;
/// values that are not plausible are listed in `refused` instead of kept.
pub fn parse_draft(input: &Value) -> Draft {
    let mut draft = Draft {
        notes: input["notes"]
            .as_str()
            .unwrap_or_default()
            .chars()
            .take(MAX_DESCRIPTION)
            .collect(),
        ..Draft::default()
    };
    let Some(fields) = input["fields"].as_object() else {
        return draft;
    };
    for (key, raw) in fields {
        let Some((key, label, unit, max)) = static_str(key) else {
            continue;
        };
        let quote: String = raw["quote"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .chars()
            .take(MAX_QUOTE)
            .collect();
        if quote.is_empty() {
            draft
                .refused
                .push(format!("{label}: no words from the datasheet were given"));
            continue;
        }
        let value = if unit.is_empty() {
            let text = raw["value"].as_str().unwrap_or_default().trim();
            let cap = if key == "description" {
                MAX_DESCRIPTION
            } else {
                MAX_TEXT
            };
            if text.is_empty() || text.chars().count() > cap {
                draft.refused.push(format!("{label}: not usable text"));
                continue;
            }
            FieldValue::Text(text.to_string())
        } else {
            match raw["value"].as_f64() {
                Some(n) if n.is_finite() && n > 0.0 && n <= max => FieldValue::Number(n),
                _ => {
                    draft
                        .refused
                        .push(format!("{label}: not a plausible number in {unit}"));
                    continue;
                }
            }
        };
        draft.fields.push(Field {
            key,
            label,
            unit,
            value,
            page: raw["page"].as_u64().and_then(|p| u32::try_from(p).ok()),
            quote,
        });
    }
    // Keep the table's order, not the map's.
    draft
        .fields
        .sort_by_key(|f| FIELDS.iter().position(|x| x.0 == f.key));
    draft
}

/// The result of one reading, with what it cost.
#[derive(Debug)]
pub struct Reading {
    pub draft: Draft,
    pub usage: Usage,
    pub cost_usd: f64,
}

/// Sends the datasheet and returns Claude's draft. One request, no project data.
pub fn read<T: Transport>(
    client: &mut Client<T>,
    settings: &AiSettings,
    spent_before: f64,
    file_name: &str,
    pdf: &[u8],
) -> Result<Reading, ApiError> {
    if pdf.is_empty() || pdf.len() > MAX_PDF_BYTES {
        return Err(ApiError::BadRequest(format!(
            "the datasheet must be a PDF of at most {} MB",
            MAX_PDF_BYTES / 1024 / 1024
        )));
    }
    if !pdf.starts_with(b"%PDF") {
        return Err(ApiError::BadRequest("that file is not a PDF".into()));
    }
    if over_budget(spent_before, settings.monthly_cap_usd) {
        return Err(ApiError::BudgetReached);
    }
    let model = models::resolve(&settings.model);
    let title: String = file_name.chars().take(100).collect();
    let messages = vec![json!({"role": "user", "content": [
        {"type": "document",
         "source": {"type": "base64", "media_type": "application/pdf", "data": base64(pdf)},
         "title": title},
        {"type": "text",
         "text": "Read this datasheet and report the part with report_part."}
    ]})];
    let reply = client.messages(&RequestSpec {
        model,
        effort: settings.effort_or_default(),
        system: SYSTEM_DATASHEET,
        messages: &messages,
        tools: &[tool()],
    })?;
    let cost_usd = reply.usage.cost_usd(&reply.model, model.id);
    let calls = reply.tool_uses();
    let Some((_, _, input)) = calls.iter().find(|(_, name, _)| name == "report_part") else {
        let say = reply.text();
        return Err(ApiError::Parse(if say.is_empty() {
            "Claude did not report a part from this file".into()
        } else {
            format!(
                "Claude did not report a part: {}",
                say.chars().take(300).collect::<String>()
            )
        }));
    };
    Ok(Reading {
        draft: parse_draft(input),
        usage: reply.usage,
        cost_usd,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::testing::*;

    fn settings() -> AiSettings {
        AiSettings {
            enabled: true,
            ..AiSettings::default()
        }
    }

    fn client() -> (Script, Client<Script>) {
        let s = Script::default();
        (s.clone(), Client::new(s, key()).with_sleep(|_| {}))
    }

    fn pdf() -> Vec<u8> {
        b"%PDF-1.4 test".to_vec()
    }

    #[test]
    fn a_datasheet_becomes_a_draft_with_evidence() {
        let (s, mut c) = client();
        s.push(
            200,
            tool_reply(
                "claude-opus-5-5",
                "t1",
                "report_part",
                json!({"fields": {
                    "manufacturer": {"value": "Acme", "page": 1, "quote": "Acme GmbH"},
                    "order_number": {"value": "A-100", "page": 1, "quote": "Order no. A-100"},
                    "width_mm": {"value": 45.0, "page": 2, "quote": "W 45 mm"},
                    "depth_mm": {"value": -3.0, "page": 2, "quote": "D -3"},
                    "power_loss_w": {"value": 12.0, "quote": ""},
                    "colour": {"value": "red", "quote": "red"}
                }, "notes": "Two variants in the table"}),
            ),
        );
        let r = read(&mut c, &settings(), 0.0, "a.pdf", &pdf()).unwrap();
        let d = r.draft;
        assert_eq!(
            d.get("manufacturer").unwrap().value,
            FieldValue::Text("Acme".into())
        );
        assert_eq!(d.get("width_mm").unwrap().value, FieldValue::Number(45.0));
        assert_eq!(d.get("width_mm").unwrap().page, Some(2));
        assert!(d.get("depth_mm").is_none(), "a negative size is refused");
        assert!(
            d.get("power_loss_w").is_none(),
            "a value with no quote is refused"
        );
        assert!(d.get("colour").is_none(), "unknown keys are ignored");
        assert_eq!(d.refused.len(), 2);
        assert_eq!(d.notes, "Two variants in the table");
        // Table order, not map order.
        assert_eq!(d.fields[0].key, "manufacturer");
        assert!(r.cost_usd > 0.0);
    }

    #[test]
    fn the_request_carries_the_pdf_and_no_project_data() {
        let (s, mut c) = client();
        s.push(
            200,
            tool_reply(
                "claude-opus-5-5",
                "t1",
                "report_part",
                json!({"fields": {}}),
            ),
        );
        read(&mut c, &settings(), 0.0, "a.pdf", &pdf()).unwrap();
        let body: Value = serde_json::from_str(&s.requests()[0].body).unwrap();
        let content = &body["messages"][0]["content"];
        assert_eq!(content[0]["type"], "document");
        assert_eq!(content[0]["source"]["media_type"], "application/pdf");
        assert_eq!(body["tools"].as_array().unwrap().len(), 1);
        assert_eq!(body["tools"][0]["name"], "report_part");
        assert!(!s.requests()[0].body.contains("sk-ant-"));
    }

    #[test]
    fn bad_files_and_an_empty_budget_send_nothing() {
        let (s, mut c) = client();
        assert!(read(&mut c, &settings(), 0.0, "a.pdf", b"not a pdf").is_err());
        assert!(read(&mut c, &settings(), 0.0, "a.pdf", &[]).is_err());
        let capped = AiSettings {
            monthly_cap_usd: 1.0,
            ..settings()
        };
        assert_eq!(
            read(&mut c, &capped, 2.0, "a.pdf", &pdf()).unwrap_err(),
            ApiError::BudgetReached
        );
        assert!(s.requests().is_empty());
    }

    #[test]
    fn no_tool_call_is_an_error_not_an_empty_part() {
        let (s, mut c) = client();
        s.push(
            200,
            text_reply("claude-opus-5-5", "This is a recipe.", 10, 5),
        );
        let e = read(&mut c, &settings(), 0.0, "a.pdf", &pdf()).unwrap_err();
        assert!(e.to_string().contains("recipe"), "{e}");
    }
}

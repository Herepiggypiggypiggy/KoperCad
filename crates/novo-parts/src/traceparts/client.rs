//! The TraceParts Product Explorer API Gateway client (blocking; call it
//! from a worker thread). Endpoints and field names follow
//! developers.traceparts.com as read on 2026-10-30. Not yet exercised with a
//! real key: the shapes are tested against a mock server only.

use std::time::{Duration, Instant};

use serde::Deserialize;

use super::error::TpError;
use super::keystore::Credentials;
use super::transport::{Method, Request, Response, Transport, TransportError};

pub const DEFAULT_BASE: &str = "https://api-gateway.traceparts.com";
/// Documented lifetime is 24 hours; refresh early.
const TOKEN_LIFETIME: Duration = Duration::from_secs(23 * 3600);
/// Largest CAD download we accept.
pub const MAX_DOWNLOAD: usize = 200 * 1024 * 1024;
const MAX_JSON: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct View {
    #[serde(rename = "Name", default)]
    pub name: Option<String>,
    #[serde(rename = "Description", default)]
    pub description: Option<String>,
    #[serde(rename = "URL", default)]
    pub url: Option<String>,
}

/// Answer of `CADViews/ByItemNumber`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Views {
    #[serde(rename = "List", default)]
    pub list: Option<Vec<View>>,
    #[serde(rename = "ItemURL", default)]
    pub item_url: Option<String>,
    #[serde(rename = "ErrorMessage", default)]
    pub error_message: Option<String>,
    #[serde(rename = "Status", default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Format {
    #[serde(rename = "Id", default)]
    pub id: Option<String>,
    #[serde(rename = "Extension", default)]
    pub extension: Option<String>,
    #[serde(rename = "Description", default)]
    pub description: Option<String>,
    #[serde(rename = "MimeType", default)]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Item {
    #[serde(rename = "Id", default)]
    pub id: i64,
    #[serde(rename = "Number", default)]
    pub number: Option<String>,
    #[serde(rename = "Name", default)]
    pub name: Option<String>,
    #[serde(rename = "DisplayName", default)]
    pub display_name: Option<String>,
    #[serde(rename = "URL", default)]
    pub url: Option<String>,
}

/// Which model to generate. `cad_part_unique_id` and `logic_name` come from
/// TraceParts' own answers; `view` is a view description such as `3D`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CadTarget {
    pub cad_part_unique_id: String,
    pub logic_name: String,
    pub language: String,
    pub view: String,
    pub format_id: String,
}

impl Format {
    /// STEP, or a ZIP of it, which is how TraceParts often delivers files.
    pub fn is_step(&self) -> bool {
        let ext = self.extension.as_deref().unwrap_or("").to_ascii_lowercase();
        let id = self.id.as_deref().unwrap_or("").to_ascii_lowercase();
        matches!(ext.as_str(), "stp" | "step") || id.starts_with("step")
    }
}

/// The best STEP format on offer: the newest AP (highest number in the id),
/// else the first.
pub fn pick_step_format(formats: &[Format]) -> Option<&Format> {
    let digits = |f: &&Format| -> u32 {
        f.id.as_deref()
            .unwrap_or("")
            .chars()
            .filter(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .unwrap_or(0)
    };
    formats.iter().filter(|f| f.is_step()).max_by_key(digits)
}

/// Pulls `cadPartUniqueId` and `logicName` out of a view or viewer URL, when
/// TraceParts put them there. UNVERIFIED against real answers: callers must
/// handle `None`.
pub fn ids_from_view_url(url: &str) -> Option<(String, String)> {
    let query = url.split_once('?')?.1;
    let mut id = None;
    let mut logic = None;
    for pair in query.split('&') {
<<<<<<< HEAD
        let (k, v) = pair.split_once('=')?;
=======
        let Some((k, v)) = pair.split_once('=') else {
            continue;
        };
>>>>>>> origin/main
        match k.to_ascii_lowercase().as_str() {
            "cadpartuniqueid" => id = Some(percent_decode(v)),
            "logicname" => logic = Some(percent_decode(v)),
            _ => {}
        }
    }
    Some((id?, logic?))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Some(v) = std::str::from_utf8(&b[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

<<<<<<< HEAD
=======
/// https only; plain http is allowed for loopback (the mock gateway in
/// tests), so a model cannot be swapped in transit.
pub fn is_safe_url(url: &str) -> bool {
    if url.starts_with("https://") {
        return true;
    }
    let Some(rest) = url.strip_prefix("http://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit_once('@').map_or(host, |(_, h)| h);
    let name = if host.starts_with('[') {
        host.split(']').next().map_or(host, |h| &h[1..])
    } else {
        host.split(':').next().unwrap_or(host)
    };
    matches!(name, "localhost" | "127.0.0.1" | "::1")
}

>>>>>>> origin/main
pub fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub struct Client<T: Transport> {
    transport: T,
    base: String,
    creds: Credentials,
    token: Option<(String, Instant)>,
    /// Pauses between retries of transient failures. Empty = no retry.
    pub retry_delays: Vec<Duration>,
}

impl<T: Transport> Client<T> {
    pub fn new(transport: T, creds: Credentials) -> Self {
        Client {
            transport,
            base: DEFAULT_BASE.to_string(),
            creds,
            token: None,
            retry_delays: vec![Duration::from_millis(500), Duration::from_secs(2)],
        }
    }

    /// For tests and for a gateway behind a proxy.
    pub fn with_base(mut self, base: &str) -> Self {
        self.base = base.trim_end_matches('/').to_string();
        self
    }

    fn token(&mut self) -> Result<String, TpError> {
        if let Some((t, at)) = &self.token
            && at.elapsed() < TOKEN_LIFETIME
        {
            return Ok(t.clone());
        }
        #[derive(Deserialize)]
        struct Tok {
            token: String,
        }
        let body = serde_json::json!({
            "tenantUid": self.creds.tenant_uid,
            "apiKey": self.creds.api_key,
        })
        .to_string();
        let req = Request {
            method: Method::Post,
            url: format!("{}/v2/RequestToken", self.base),
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: Some(body),
        };
        let resp = self.send_with_retry(&req, MAX_JSON)?;
        let tok: Tok = parse_json(&resp)?;
        if tok.token.is_empty() {
            return Err(TpError::BadResponse("empty token".into()));
        }
        self.token = Some((tok.token.clone(), Instant::now()));
        Ok(tok.token)
    }

    fn send_with_retry(&mut self, req: &Request, max: usize) -> Result<Response, TpError> {
        let mut attempt = 0;
        loop {
            let result = match self.transport.send(req, max) {
                Ok(r) => check_status(r),
                Err(TransportError::Offline(w)) => Err(TpError::Offline(w)),
                Err(TransportError::Timeout) => Err(TpError::Timeout),
                Err(TransportError::Other(w)) => Err(TpError::BadResponse(w)),
            };
            match result {
                Err(e) if e.is_transient() && attempt < self.retry_delays.len() => {
                    std::thread::sleep(self.retry_delays[attempt]);
                    attempt += 1;
                }
                other => return other,
            }
        }
    }

    /// An authorised call. A 401 drops the cached token and tries once more
    /// with a fresh one (tokens can be revoked before they expire).
    fn call(
        &mut self,
        method: Method,
        path: &str,
        body: Option<String>,
        max: usize,
    ) -> Result<Response, TpError> {
        for fresh in [false, true] {
            let token = self.token()?;
            let mut headers = vec![("Authorization".to_string(), format!("Bearer {token}"))];
            if body.is_some() {
                headers.push(("Content-Type".into(), "application/json".into()));
            }
            let req = Request {
                method,
                url: format!("{}{}", self.base, path),
                headers,
                body: body.clone(),
            };
            match self.send_with_retry(&req, max) {
                Err(TpError::Unauthorized) if !fresh => self.token = None,
                other => return other,
            }
        }
        Err(TpError::Unauthorized)
    }

    /// Checks the key by asking for a token.
    pub fn check_key(&mut self) -> Result<(), TpError> {
        self.token = None;
        self.token().map(|_| ())
    }

    pub fn cad_views_by_item_number(&mut self, number: &str) -> Result<Views, TpError> {
        let path = format!(
            "/v1/CADViews/ByItemNumber?itemNumber={}",
            url_encode(number)
        );
        let views: Views = parse_json(&self.call(Method::Get, &path, None, MAX_JSON)?)?;
        let empty = views.list.as_ref().is_none_or(Vec::is_empty);
        if empty {
<<<<<<< HEAD
            return Err(TpError::NotFound);
=======
            return Err(TpError::NoModel);
>>>>>>> origin/main
        }
        Ok(views)
    }

    pub fn item_by_attribute(
        &mut self,
        attribute_id: i32,
        data: &str,
        catalog_id: Option<i32>,
    ) -> Result<Item, TpError> {
        let mut path = format!(
            "/v1/Item/ByAttribute?attributeId={attribute_id}&data={}",
            url_encode(data)
        );
        if let Some(c) = catalog_id {
            path.push_str(&format!("&catalogId={c}"));
        }
        parse_json(&self.call(Method::Get, &path, None, MAX_JSON)?)
    }

    pub fn is_cad_available(&mut self, cad_part_unique_id: &str) -> Result<bool, TpError> {
        let path = format!(
            "/v1/CADServices/IsCADAvailable?cadPartUniqueId={}",
            url_encode(cad_part_unique_id)
        );
        parse_json(&self.call(Method::Get, &path, None, MAX_JSON)?)
    }

    pub fn available_formats(&mut self, view_description: &str) -> Result<Vec<Format>, TpError> {
        let path = format!(
            "/v1/CADServices/AvailableFormats?viewDescription={}",
            url_encode(view_description)
        );
        parse_json(&self.call(Method::Get, &path, None, MAX_JSON)?)
    }

    /// Asks TraceParts to generate the file and returns its download URL.
    pub fn cad_file_url(&mut self, t: &CadTarget) -> Result<String, TpError> {
        let path = format!(
            "/v1/CADServices/CADFileUrl?cadPartUniqueId={}&logicName={}&language={}&viewDescription={}&formatId={}",
            url_encode(&t.cad_part_unique_id),
            url_encode(&t.logic_name),
            url_encode(&t.language),
            url_encode(&t.view),
            url_encode(&t.format_id),
        );
        let resp = self.call(Method::Post, &path, Some("{}".into()), MAX_JSON)?;
        // The documented answer is a JSON string; accept a bare URL too.
        let text = String::from_utf8_lossy(&resp.body);
        let url = serde_json::from_str::<String>(&text).unwrap_or_else(|_| text.trim().to_string());
<<<<<<< HEAD
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err(TpError::BadResponse("the file address is not a URL".into()));
=======
        if !is_safe_url(&url) {
            return Err(TpError::BadResponse(
                "the file address is not an https URL".into(),
            ));
>>>>>>> origin/main
        }
        Ok(url)
    }

    /// Downloads a file URL from `cad_file_url`. Those URLs carry their own
    /// authorisation, so the bearer token is not sent.
    pub fn download(&mut self, url: &str) -> Result<Vec<u8>, TpError> {
<<<<<<< HEAD
=======
        if !is_safe_url(url) {
            return Err(TpError::BadResponse(
                "the file address is not an https URL".into(),
            ));
        }
>>>>>>> origin/main
        let req = Request {
            method: Method::Get,
            url: url.to_string(),
            headers: vec![],
            body: None,
        };
        Ok(self.send_with_retry(&req, MAX_DOWNLOAD)?.body)
    }
}

fn check_status(r: Response) -> Result<Response, TpError> {
    let text = || {
        String::from_utf8_lossy(&r.body)
            .chars()
            .take(200)
            .collect::<String>()
    };
    match r.status {
        200..=299 => Ok(r),
        401 => Err(TpError::Unauthorized),
        403 => Err(TpError::Forbidden),
        404 => Err(TpError::NotFound),
        400 | 422 => Err(TpError::Rejected(text())),
        429 => Err(TpError::RateLimited(
            r.header("Retry-After").and_then(|v| v.trim().parse().ok()),
        )),
        s if s >= 500 => Err(TpError::Server(s, text())),
        s => Err(TpError::BadResponse(format!("status {s}"))),
    }
}

fn parse_json<V: for<'de> Deserialize<'de>>(r: &Response) -> Result<V, TpError> {
    serde_json::from_slice(&r.body).map_err(|e| TpError::BadResponse(e.to_string()))
}
<<<<<<< HEAD
=======

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_or_loopback_http_is_safe() {
        assert!(is_safe_url("https://files.traceparts.com/a.stp"));
        assert!(is_safe_url("http://127.0.0.1:8080/a"));
        assert!(is_safe_url("http://localhost/a"));
        assert!(is_safe_url("http://[::1]:9/a"));
        assert!(!is_safe_url("http://files.traceparts.com/a.stp"));
        assert!(!is_safe_url("http://127.0.0.1@evil.example/a"));
        assert!(!is_safe_url("ftp://x/a"));
        assert!(!is_safe_url("file.stp"));
    }

    #[test]
    fn ids_survive_a_pair_without_equals() {
        let got = ids_from_view_url("https://v/?flag&cadPartUniqueId=A%2F1&logicName=c+1");
        assert_eq!(got, Some(("A/1".into(), "c 1".into())));
    }
}
>>>>>>> origin/main

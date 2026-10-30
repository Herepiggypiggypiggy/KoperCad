//! The HTTP layer behind a trait, so the client is tested without a network.
//! The real one uses `ureq` with rustls: no system TLS, no admin rights.

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

#[derive(Debug, Clone)]
pub struct Request {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    Offline(String),
    Timeout,
    Other(String),
}

pub trait Transport: Send {
    /// Sends one request. HTTP error statuses are `Ok` responses; only
    /// failures to get an answer are errors. `max_body` caps the bytes read.
    fn send(&mut self, req: &Request, max_body: usize) -> Result<Response, TransportError>;
}

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
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_global(Some(Duration::from_secs(120)))
            .build()
            .into();
        Self { agent }
    }
}

fn classify(e: ureq::Error) -> TransportError {
    match e {
        ureq::Error::Timeout(_) => TransportError::Timeout,
        ureq::Error::HostNotFound | ureq::Error::ConnectionFailed | ureq::Error::Io(_) => {
            TransportError::Offline(e.to_string())
        }
        other => TransportError::Other(other.to_string()),
    }
}

impl Transport for UreqTransport {
    fn send(&mut self, req: &Request, max_body: usize) -> Result<Response, TransportError> {
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
                r.send(req.body.clone().unwrap_or_default().as_bytes())
            }
        };
        let mut resp = result.map_err(classify)?;
        let status = resp.status().as_u16();
        let headers = resp
            .headers()
            .iter()
            .filter_map(|(k, v)| Some((k.as_str().to_string(), v.to_str().ok()?.to_string())))
            .collect();
        let body = resp
            .body_mut()
            .with_config()
            .limit(max_body as u64)
            .read_to_vec()
            .map_err(classify)?;
        Ok(Response {
            status,
            headers,
            body,
        })
    }
}

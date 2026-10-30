//! The TraceParts client against a mock gateway on localhost, through the
//! real `ureq` transport.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use novo_parts::traceparts::*;

const TENANT: &str = "123e4567-e89b-12d3-a456-426614174000";
const STEP: &str = "ISO-10303-21;\nHEADER;\nENDSEC;\nEND-ISO-10303-21;\n";

type Reply = (u16, Vec<(&'static str, String)>, Vec<u8>);
type Log = Arc<Mutex<Vec<String>>>;

/// `handler(method, target, authorization, body)`.
fn serve(handler: impl Fn(&str, &str, &str, &str) -> Reply + Send + 'static) -> (String, Log) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let log: Log = Arc::default();
    let log2 = log.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { break };
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            let (head_end, len) = loop {
                let n = s.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break (0, 0);
                }
                buf.extend_from_slice(&chunk[..n]);
                if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..p]).to_ascii_lowercase();
                    let len = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse().ok())
                        .unwrap_or(0usize);
                    break (p + 4, len);
                }
            };
            if head_end == 0 {
                continue;
            }
            while buf.len() < head_end + len {
                let n = s.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
            let body = String::from_utf8_lossy(&buf[head_end..]).into_owned();
            let mut first = head.lines().next().unwrap().split(' ');
            let (method, target) = (first.next().unwrap(), first.next().unwrap());
            let auth = head
                .lines()
                .find_map(|l| {
                    l.strip_prefix("authorization: ")
                        .or(l.strip_prefix("Authorization: "))
                })
                .unwrap_or("");
            log2.lock().unwrap().push(format!("{method} {target}"));
            let (status, headers, payload) = handler(method, target, auth, &body);
            let mut out = format!(
                "HTTP/1.1 {status} X\r\nConnection: close\r\nContent-Length: {}\r\n",
                payload.len()
            );
            for (k, v) in headers {
                out.push_str(&format!("{k}: {v}\r\n"));
            }
            out.push_str("\r\n");
            let _ = s.write_all(out.as_bytes());
            let _ = s.write_all(&payload);
        }
    });
    (base, log)
}

fn json(v: serde_json::Value) -> Reply {
    (
        200,
        vec![("Content-Type", "application/json".into())],
        v.to_string().into_bytes(),
    )
}

fn creds() -> Credentials {
    Credentials::new(TENANT, "test-key").unwrap()
}

fn client(base: &str) -> Client<UreqTransport> {
    let mut c = Client::new(UreqTransport::new(), creds()).with_base(base);
    c.retry_delays = vec![Duration::ZERO, Duration::ZERO];
    c
}

/// A gateway that knows one part, LC1D09BD.
fn gateway(
    base_cell: Arc<Mutex<String>>,
    download_ok: bool,
) -> impl Fn(&str, &str, &str, &str) -> Reply + Send + 'static {
    move |method, target, auth, body| {
        let path = target.split('?').next().unwrap();
        if path == "/v2/RequestToken" {
            let b: serde_json::Value = serde_json::from_str(body).unwrap();
            return if b["apiKey"] == "test-key" && b["tenantUid"] == TENANT {
                json(serde_json::json!({"token": "T1", "expiryDate": "2099-01-01T00:00:00Z"}))
            } else {
                (401, vec![], b"bad key".to_vec())
            };
        }
        if !path.starts_with("/files/") && auth != "Bearer T1" {
            return (401, vec![], b"no token".to_vec());
        }
        let base = base_cell.lock().unwrap().clone();
        match (method, path) {
            ("GET", "/v1/CADViews/ByItemNumber") if target.contains("itemNumber=LC1D09BD") => {
                json(serde_json::json!({
                    "List": [
                        {"Name": "2D", "Description": "2D front view", "URL": "https://x/2d"},
                        {"Name": "3D", "Description": "3D",
                         "URL": "https://viewer/?cadPartUniqueId=ABC%2F1&logicName=cfg+1"}],
                    "ItemURL": "https://x/item", "Status": "OK"
                }))
            }
            ("GET", "/v1/CADViews/ByItemNumber") => {
                json(serde_json::json!({"List": [], "Status": "NotFound"}))
            }
            ("GET", "/v1/CADServices/IsCADAvailable") => json(serde_json::json!(true)),
            ("GET", "/v1/CADServices/AvailableFormats") => json(serde_json::json!([
                {"Id": "sat70", "Extension": "sat", "Description": "SAT", "MimeType": "x"},
                {"Id": "step214", "Extension": "stp", "Description": "STEP AP214", "MimeType": "application/STEP"},
                {"Id": "step203", "Extension": "stp", "Description": "STEP AP203", "MimeType": "application/STEP"}
            ])),
            ("POST", "/v1/CADServices/CADFileUrl") => {
                assert!(target.contains("cadPartUniqueId=ABC%2F1"), "{target}");
                assert!(target.contains("logicName=cfg%201"), "{target}");
                assert!(target.contains("formatId=step214"), "{target}");
                json(serde_json::json!(format!("{base}/files/model.stp")))
            }
            ("GET", "/files/model.stp") if download_ok => {
                assert_eq!(auth, "", "the bearer token must not go to the file host");
                (200, vec![], STEP.as_bytes().to_vec())
            }
            ("GET", "/files/model.stp") => (200, vec![], b"<html>please sign in</html>".to_vec()),
            _ => (404, vec![], b"nope".to_vec()),
        }
    }
}

fn service(base: &str) -> (Service<UreqTransport>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let svc = Service::new(DownloadCache::at(dir.path()), Some(client(base)));
    (svc, dir)
}

#[test]
fn fetches_a_step_file_and_then_serves_it_from_the_cache() {
    let cell = Arc::new(Mutex::new(String::new()));
    let (base, log) = serve(gateway(cell.clone(), true));
    *cell.lock().unwrap() = base.clone();
    let (mut svc, _d) = service(&base);

    let first = svc.fetch_step("LC1D09BD").unwrap();
    assert!(!first.from_cache);
    assert_eq!(std::fs::read_to_string(&first.path).unwrap(), STEP);
    let calls = log.lock().unwrap().len();

    // Second time: same file, no request, and no key needed either.
    svc.client = None;
    let second = svc.fetch_step("lc1d09 bd").unwrap();
    assert!(second.from_cache);
    assert_eq!(second.path, first.path);
    assert_eq!(log.lock().unwrap().len(), calls);
    // One token request only for the whole chain.
    assert_eq!(
        log.lock()
            .unwrap()
            .iter()
            .filter(|l| l.contains("RequestToken"))
            .count(),
        1
    );
}

#[test]
fn unknown_part_is_not_found_and_remembered() {
    let cell = Arc::new(Mutex::new(String::new()));
    let (base, log) = serve(gateway(cell, true));
    let (mut svc, _d) = service(&base);
<<<<<<< HEAD
    assert_eq!(svc.fetch_step("NOPE1").unwrap_err(), TpError::NotFound);
    let calls = log.lock().unwrap().len();
    assert_eq!(svc.fetch_step("NOPE1").unwrap_err(), TpError::NotFound);
=======
    assert_eq!(svc.fetch_step("NOPE1").unwrap_err(), TpError::NoModel);
    let calls = log.lock().unwrap().len();
    assert_eq!(svc.fetch_step("NOPE1").unwrap_err(), TpError::NoModel);
>>>>>>> origin/main
    assert_eq!(
        log.lock().unwrap().len(),
        calls,
        "second miss must not ask again"
    );
}

#[test]
fn a_wrong_key_is_unauthorized() {
    let cell = Arc::new(Mutex::new(String::new()));
    let (base, _) = serve(gateway(cell, true));
    let mut c = Client::new(
        UreqTransport::new(),
        Credentials::new(TENANT, "wrong-key").unwrap(),
    )
    .with_base(&base);
    c.retry_delays.clear();
    assert_eq!(c.check_key().unwrap_err(), TpError::Unauthorized);
}

#[test]
fn a_login_page_instead_of_a_model_is_refused() {
    let cell = Arc::new(Mutex::new(String::new()));
    let (base, _) = serve(gateway(cell.clone(), false));
    *cell.lock().unwrap() = base.clone();
    let (mut svc, d) = service(&base);
    assert!(matches!(
        svc.fetch_step("LC1D09BD").unwrap_err(),
        TpError::NotCad(_)
    ));
    assert!(svc.cache.is_empty());
    drop(d);
}

#[test]
fn no_key_and_cache_only_are_clear_errors() {
    let dir = tempfile::tempdir().unwrap();
    let mut svc: Service<UreqTransport> = Service::new(DownloadCache::at(dir.path()), None);
    assert_eq!(svc.fetch_step("A1").unwrap_err(), TpError::NoKey);
    svc.mode = Mode::CacheOnly;
    assert_eq!(svc.fetch_step("A1").unwrap_err(), TpError::NotCached);
}

#[test]
fn rate_limit_reports_retry_after_and_server_errors_are_retried() {
    let hits = Arc::new(Mutex::new(0));
    let h = hits.clone();
    let (base, _) = serve(move |_, target, _, _| {
        if target.contains("RequestToken") {
            let mut n = h.lock().unwrap();
            *n += 1;
            // Two 503s, then a token.
            if *n <= 2 {
                return (503, vec![], b"busy".to_vec());
            }
            return json(serde_json::json!({"token": "T1"}));
        }
        (429, vec![("Retry-After", "7".into())], vec![])
    });
    let mut c = client(&base);
    assert_eq!(
        c.is_cad_available("x").unwrap_err(),
        TpError::RateLimited(Some(7))
    );
    assert_eq!(*hits.lock().unwrap(), 3);
}

#[test]
fn a_revoked_token_is_renewed_once() {
    let tokens = Arc::new(Mutex::new(0));
    let t = tokens.clone();
    let (base, _) = serve(move |_, target, auth, _| {
        if target.contains("RequestToken") {
            let mut n = t.lock().unwrap();
            *n += 1;
            return json(serde_json::json!({"token": format!("T{n}")}));
        }
        if auth == "Bearer T2" {
            json(serde_json::json!(true))
        } else {
            (401, vec![], vec![])
        }
    });
    let mut c = client(&base);
    assert!(c.is_cad_available("x").unwrap());
    assert_eq!(*tokens.lock().unwrap(), 2);
}

#[test]
fn offline_is_reported_as_offline() {
    // Nothing listens on this port any more.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mut c = client(&format!("http://127.0.0.1:{port}"));
    c.retry_delays.clear();
    assert!(matches!(c.check_key().unwrap_err(), TpError::Offline(_)));
}

#[test]
fn a_garbled_answer_is_a_bad_response_not_a_panic() {
    let (base, _) = serve(|_, target, _, _| {
        if target.contains("RequestToken") {
            json(serde_json::json!({"token": "T"}))
        } else {
            (200, vec![], b"<<<not json".to_vec())
        }
    });
    let mut c = client(&base);
    assert!(matches!(
        c.available_formats("3D").unwrap_err(),
        TpError::BadResponse(_)
    ));
}
<<<<<<< HEAD
=======

#[test]
fn a_bare_404_is_not_remembered_as_no_model() {
    let (base, log) = serve(|_, target, _, _| {
        if target.contains("RequestToken") {
            json(serde_json::json!({"token": "T"}))
        } else {
            (404, vec![], b"wrong endpoint".to_vec())
        }
    });
    let (mut svc, _d) = service(&base);
    assert_eq!(svc.fetch_step("A1").unwrap_err(), TpError::NotFound);
    let calls = log.lock().unwrap().len();
    assert_eq!(svc.fetch_step("A1").unwrap_err(), TpError::NotFound);
    assert!(
        log.lock().unwrap().len() > calls,
        "a 404 must be asked again"
    );
}

#[test]
fn plain_http_download_addresses_are_refused() {
    let (base, _) = serve(|_, target, _, _| {
        if target.contains("RequestToken") {
            json(serde_json::json!({"token": "T"}))
        } else {
            json(serde_json::json!("http://files.example.com/a.stp"))
        }
    });
    let mut c = client(&base);
    let t = CadTarget {
        cad_part_unique_id: "a".into(),
        logic_name: "b".into(),
        language: "EN".into(),
        view: "3D".into(),
        format_id: "step214".into(),
    };
    assert!(matches!(
        c.cad_file_url(&t).unwrap_err(),
        TpError::BadResponse(_)
    ));
    assert!(matches!(
        c.download("http://files.example.com/a.stp").unwrap_err(),
        TpError::BadResponse(_)
    ));
}
>>>>>>> origin/main

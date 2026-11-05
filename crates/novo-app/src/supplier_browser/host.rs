//! The supplier browser window (Windows, WebView2). See the module doc of
//! `supplier_browser` for the rules. This file must stay free of anything
//! that reads or drives a page; a test enforces it.

use std::convert::Infallible;
use std::path::PathBuf;

use novo_parts::supplier_site::{self, Supplier};
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::{NewWindowResponse, WebContext, WebViewBuilder};

#[derive(Debug)]
pub enum HostError {
    /// The WebView2 runtime is not installed.
    NoRuntime,
    Failed(String),
}

/// What the page handlers ask the window to do (they run inside the web view
/// and must not touch it directly).
#[derive(Debug)]
enum UserEvent {
    /// A pop-up or `target=_blank` link: open it in this window.
    Open(String),
    /// The page's own title changed.
    Title(String),
    /// A line about the last download, shown in the title bar.
    Status(String),
}

pub fn runtime_available() -> bool {
    wry::webview_version().is_ok()
}

fn stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Pages that may load: plain web pages, plus the two internal addresses a
/// page uses while it works (`about:blank`, `blob:` downloads).
fn navigation_ok(url: &str) -> bool {
    supplier_site::url_allowed(url) || url == "about:blank" || url.starts_with("blob:http")
}

/// Opens the window and runs until it is closed.
pub fn run(
    supplier: &Supplier,
    url: &str,
    profile: PathBuf,
    landing: PathBuf,
) -> Result<Infallible, HostError> {
    if !runtime_available() {
        return Err(HostError::NoRuntime);
    }
    let host = supplier.host().unwrap_or_default();
    let base = format!("{} ({host}) - KoperCAD", supplier.name);
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title(base.clone())
        .with_inner_size(LogicalSize::new(1180.0, 820.0))
        .build(&event_loop)
        .map_err(|e| HostError::Failed(e.to_string()))?;

    // Cookies and site storage stay in this supplier's own folder.
    let mut context = WebContext::new(Some(profile));
    let open_proxy = event_loop.create_proxy();
    let title_proxy = event_loop.create_proxy();
    let status_proxy = event_loop.create_proxy();
    let done_proxy = event_loop.create_proxy();
    let accepting = supplier.clone();
    let webview = WebViewBuilder::new_with_web_context(&mut context)
        .with_url(url)
        .with_navigation_handler(|u| navigation_ok(&u))
        .with_new_window_req_handler(move |u, _| {
            if supplier_site::url_allowed(&u) {
                let _ = open_proxy.send_event(UserEvent::Open(u));
            }
            NewWindowResponse::Deny
        })
        .with_document_title_changed_handler(move |t| {
            let _ = title_proxy.send_event(UserEvent::Title(t));
        })
        .with_download_started_handler(move |_url, path| {
            let suggested = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            match accepting.check_file(&suggested) {
                Ok(name) => {
                    *path = supplier_site::partial_path(&landing, &name, stamp());
                    let _ =
                        status_proxy.send_event(UserEvent::Status(format!("Downloading {name}")));
                    true
                }
                Err(why) => {
                    let _ = status_proxy.send_event(UserEvent::Status(format!("Not kept: {why}")));
                    false
                }
            }
        })
        .with_download_completed_handler(move |_url, path, ok| {
            let msg = match (path, ok) {
                (Some(partial), true) => match supplier_site::final_path(&partial) {
                    // Renamed only when complete, so Parts never sees half a file.
                    Some(done) if std::fs::rename(&partial, &done).is_ok() => {
                        "Saved. Parts takes it in the KoperCAD window".to_owned()
                    }
                    _ => "The file could not be saved".to_owned(),
                },
                (Some(partial), false) => {
                    let _ = std::fs::remove_file(partial);
                    "The download did not finish".to_owned()
                }
                (None, _) => "The download did not finish".to_owned(),
            };
            let _ = done_proxy.send_event(UserEvent::Status(msg));
        })
        .build(&window)
        .map_err(|e| HostError::Failed(e.to_string()))?;

    let mut page_title = String::new();
    let mut status = String::new();
    event_loop.run(move |event, _, flow| {
        // Keeps the context alive as long as the window.
        let _ = &context;
        *flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *flow = ControlFlow::Exit,
            Event::UserEvent(UserEvent::Open(u)) => {
                let _ = webview.load_url(&u);
            }
            Event::UserEvent(UserEvent::Title(t)) => {
                page_title = t;
                window.set_title(&title_line(&base, &page_title, &status));
            }
            Event::UserEvent(UserEvent::Status(s)) => {
                status = s;
                window.set_title(&title_line(&base, &page_title, &status));
            }
            _ => {}
        }
    })
}

fn title_line(base: &str, page: &str, status: &str) -> String {
    let mut out = String::new();
    if !status.is_empty() {
        out.push_str(status);
        out.push_str(" - ");
    }
    if !page.is_empty() {
        out.push_str(page);
        out.push_str(" - ");
    }
    out.push_str(base);
    out
}

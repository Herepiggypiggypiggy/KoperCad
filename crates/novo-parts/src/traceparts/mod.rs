//! TraceParts integration, lane L1: API client, per-user key store and
//! download cache. Pure and blocking; the UI calls it from a worker thread.
//!
//! - [`Service::fetch_step`]: part number in, cached STEP file path out.
//! - [`KeyStore`]: the key, per user, outside every project and the repo.
//! - [`DownloadCache`]: files under `%LOCALAPPDATA%`, capped, atomic.
//! - [`Client`]: the REST calls, over a [`Transport`] (real: `ureq`).
//!
//! Status: written from the public developer docs; no key has been tried.
//! The step from a CAD view to `cadPartUniqueId`/`logicName` is the least
//! certain part ([`ids_from_view_url`]). Nothing here scrapes traceparts.com.
//! Downloaded models stay personal: not bundled, not exported in `.kcpl`.

mod cache;
mod client;
mod error;
mod keystore;
mod service;
mod transport;

pub use cache::{DEFAULT_CAP_BYTES, DownloadCache, Entry, MISS_TTL_SECS, looks_like_cad};
pub use client::{
    CadTarget, Client, DEFAULT_BASE, Format, Item, MAX_DOWNLOAD, View, Views, ids_from_view_url,
<<<<<<< HEAD
    pick_step_format, url_encode,
=======
    is_safe_url, pick_step_format, url_encode,
>>>>>>> origin/main
};
pub use error::TpError;
pub use keystore::{Credentials, ENV_KEY, ENV_TENANT, KeySource, KeyStore};
pub use service::{Fetched, Mode, Service};
pub use transport::{Method, Request, Response, Transport, TransportError, UreqTransport};

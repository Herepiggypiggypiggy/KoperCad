//! The one call the UI makes: "get me the STEP model for this part".
//!
//! Order: cache first (always works offline); then, unless offline mode is
//! on, the API. A key is only needed to go past the cache.

use std::path::PathBuf;

use super::cache::DownloadCache;
use super::client::{CadTarget, Client, Format, ids_from_view_url, pick_step_format};
use super::error::TpError;
use super::transport::Transport;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    pub path: PathBuf,
    pub from_cache: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Online,
    /// Use the cache only; never touch the network.
    CacheOnly,
}

pub struct Service<T: Transport> {
    pub cache: DownloadCache,
    /// `None` when no key is set: cache hits still work.
    pub client: Option<Client<T>>,
    pub mode: Mode,
    pub language: String,
}

impl<T: Transport> Service<T> {
    pub fn new(cache: DownloadCache, client: Option<Client<T>>) -> Self {
        Service {
            cache,
            client,
            mode: Mode::Online,
            language: "EN".into(),
        }
    }

    /// Cached STEP file for the part, no network.
    pub fn cached(&self, part_number: &str) -> Option<PathBuf> {
        self.cache.lookup(part_number, "step")
    }

    /// Finds, generates and downloads the STEP model for a manufacturer part
    /// number, or returns the cached copy.
    pub fn fetch_step(&mut self, part_number: &str) -> Result<Fetched, TpError> {
        if let Some(path) = self.cached(part_number) {
            return Ok(Fetched {
                path,
                from_cache: true,
            });
        }
        if self.mode == Mode::CacheOnly {
            return Err(TpError::NotCached);
        }
        if self.cache.recent_miss(part_number) {
            return Err(TpError::NotFound);
        }
        let language = self.language.clone();
        let client = self.client.as_mut().ok_or(TpError::NoKey)?;
        let result = (|| {
            let views = client.cad_views_by_item_number(part_number)?;
            let list = views.list.unwrap_or_default();
            let view = list
                .iter()
                .find(|v| {
                    v.description
                        .as_deref()
                        .is_some_and(|d| d.eq_ignore_ascii_case("3D"))
                })
                .or(list.first())
                .ok_or(TpError::NotFound)?;
            let description = view.description.clone().unwrap_or_else(|| "3D".into());
            let (id, logic) = view
                .url
                .as_deref()
                .and_then(ids_from_view_url)
                .ok_or_else(|| {
                    TpError::BadResponse("the view address carries no part ids".into())
                })?;
            if !client.is_cad_available(&id)? {
                return Err(TpError::NotFound);
            }
            let formats: Vec<Format> = client.available_formats(&description)?;
            let format = pick_step_format(&formats)
                .ok_or_else(|| TpError::Rejected("no STEP format on offer".into()))?;
            let target = CadTarget {
                cad_part_unique_id: id,
                logic_name: logic,
                language,
                view: description,
                format_id: format.id.clone().unwrap_or_default(),
            };
            let url = client.cad_file_url(&target)?;
            let bytes = client.download(&url)?;
            let ext = if bytes.starts_with(b"PK") {
                "zip"
            } else {
                "stp"
            };
            self.cache.store(part_number, "step", ext, &bytes)
        })();
        match result {
            Ok(path) => Ok(Fetched {
                path,
                from_cache: false,
            }),
            Err(TpError::NotFound) => {
                let _ = self.cache.note_miss(part_number);
                Err(TpError::NotFound)
            }
            Err(e) => Err(e),
        }
    }
}

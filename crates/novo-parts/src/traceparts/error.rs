//! Everything that can go wrong talking to TraceParts, in plain words.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TpError {
    /// No API key is stored (and no environment override).
    NoKey,
    /// The stored key or tenant is malformed (caught before any request).
    BadKey(String),
    /// 401: the key is wrong, expired or not yet validated by TraceParts.
    Unauthorized,
    /// 403: the key is valid but the contract does not cover this call.
    Forbidden,
<<<<<<< HEAD
    /// 404 or an empty answer: TraceParts does not know the part.
    NotFound,
=======
    /// A bare 404: the address or part is unknown. Never remembered as a
    /// miss (a wrong endpoint would otherwise poison the cache).
    NotFound,
    /// A proper answer that there is no model: an empty view list, or
    /// `IsCADAvailable` false. Remembered for a while.
    NoModel,
>>>>>>> origin/main
    /// 400 or 422: TraceParts rejected the request.
    Rejected(String),
    /// 429: too many requests. Seconds to wait, when TraceParts said.
    RateLimited(Option<u64>),
    /// No connection (or the host does not resolve).
    Offline(String),
    /// The server did not answer in time.
    Timeout,
    /// 5xx.
    Server(u16, String),
    /// The answer was not what the documentation says.
    BadResponse(String),
    /// The downloaded file is not a STEP file or ZIP.
    NotCad(String),
    /// Offline mode and the file is not in the cache.
    NotCached,
    /// A cache or key file could not be read or written.
    Io(String),
}

impl fmt::Display for TpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TpError::NoKey => write!(
                f,
                "No TraceParts API key is set. Add it in the Parts window settings, or use the browser hand-off."
            ),
            TpError::BadKey(why) => write!(f, "The TraceParts key is not valid: {why}"),
            TpError::Unauthorized => write!(
                f,
                "TraceParts refused the key (401). It may be wrong, expired, or not yet approved by TraceParts."
            ),
            TpError::Forbidden => write!(
                f,
                "TraceParts says this key may not do that (403). Check what your contract covers."
            ),
<<<<<<< HEAD
            TpError::NotFound => write!(f, "TraceParts has no CAD model for this part."),
=======
            TpError::NotFound => write!(f, "TraceParts did not find that part or address (404)."),
            TpError::NoModel => write!(f, "TraceParts has no CAD model for this part."),
>>>>>>> origin/main
            TpError::Rejected(why) => write!(f, "TraceParts rejected the request: {why}"),
            TpError::RateLimited(Some(s)) => {
                write!(f, "TraceParts asks to slow down; try again in {s} s.")
            }
            TpError::RateLimited(None) => {
                write!(f, "TraceParts asks to slow down; try again later.")
            }
            TpError::Offline(why) => write!(
                f,
                "No connection to TraceParts ({why}). Cached files still work."
            ),
            TpError::Timeout => write!(f, "TraceParts did not answer in time."),
            TpError::Server(code, why) => write!(f, "TraceParts had an error ({code}): {why}"),
            TpError::BadResponse(why) => write!(f, "Unexpected answer from TraceParts: {why}"),
            TpError::NotCad(why) => write!(f, "The download is not a CAD file: {why}"),
            TpError::NotCached => write!(f, "Offline, and this model is not in the cache yet."),
            TpError::Io(why) => write!(f, "Could not read or write a TraceParts file: {why}"),
        }
    }
}

impl std::error::Error for TpError {}

impl TpError {
    /// Worth trying again after a pause (network trouble, server errors).
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            TpError::Offline(_) | TpError::Timeout | TpError::Server(..)
        )
    }
}

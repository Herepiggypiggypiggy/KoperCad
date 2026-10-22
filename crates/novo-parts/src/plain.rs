//! Operating system errors in words a person can act on.

use std::io::{Error, ErrorKind};

/// What went wrong reading a file or folder, without the platform's error
/// number (`Not a directory (os error 20)` becomes "it is a file, not a
/// folder").
pub fn plain_io_error(e: &Error) -> String {
    // Windows says ERROR_DIRECTORY (267) for a file used as a folder.
    if e.kind() == ErrorKind::NotADirectory || e.raw_os_error() == Some(267) {
        return "it is a file, not a folder".into();
    }
    match e.kind() {
        ErrorKind::IsADirectory => "it is a folder, not a file".into(),
        ErrorKind::NotFound => "it does not exist (moved or deleted?)".into(),
        ErrorKind::PermissionDenied => "this account may not read it".into(),
        ErrorKind::InvalidData => "its contents cannot be read as text".into(),
        _ => {
            let text = e.to_string();
            // Drop " (os error 5)" and keep the platform's sentence.
            match text.rfind(" (os error") {
                Some(at) => text[..at].to_lowercase(),
                None => text,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_errors_are_said_in_words() {
        // Built from the error kind, not an OS error number: numbers mean
        // different things on Windows and Linux.
        let e = Error::from(ErrorKind::NotADirectory);
        let text = plain_io_error(&e);
        assert!(text.contains("file, not a folder"), "{text}");
        assert!(!text.contains("os error"), "{text}");
        let e = Error::from(ErrorKind::IsADirectory);
        assert!(plain_io_error(&e).contains("folder, not a file"));
        let e = Error::from(ErrorKind::NotFound);
        assert!(plain_io_error(&e).contains("does not exist"));
        let e = Error::from(ErrorKind::PermissionDenied);
        assert!(plain_io_error(&e).contains("may not read"));
        // Any other error loses its "(os error N)" tail on every platform.
        let e = Error::from_raw_os_error(28);
        assert!(!plain_io_error(&e).contains("os error"));
        // Windows says 267 for a file used as a folder.
        assert!(plain_io_error(&Error::from_raw_os_error(267)).contains("file, not a folder"));
    }
}

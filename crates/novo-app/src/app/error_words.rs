//! Plain words for operating system errors.
//!
//! Many messages end in the raw text of a file error, such as `Access is
//! denied. (os error 5)`. [`plain`] swaps that tail for a sentence that
//! says what to do. The raw text still goes to the log.

/// What to tell the person for an OS error code, by platform.
fn advice(code: i32) -> Option<&'static str> {
    if cfg!(windows) {
        Some(match code {
            2 | 3 => "the file or folder is not there any more. Check the path and try again",
            5 => "Windows does not let KoperCAD write there. Pick a folder under Documents",
            19 => "the disk is write protected. Pick another location",
            32 | 33 => "another program has the file open. Close it there and try again",
            39 | 112 => "the disk is full. Free some space or pick another drive",
            123 | 206 => "the file name or path is not allowed or is too long. Shorten it",
            _ => return None,
        })
    } else {
        Some(match code {
            2 => "the file or folder is not there any more. Check the path and try again",
            13 => "the system does not let KoperCAD write there. Pick a folder you own",
            28 => "the disk is full. Free some space or pick another location",
            30 => "the disk is read only. Pick another location",
            36 => "the file name or path is too long. Shorten it",
            _ => return None,
        })
    }
}

/// `text` with a trailing `<os text> (os error N)` replaced by advice, when
/// the code is one we know.
pub(super) fn plain(text: &str) -> String {
    let Some(end) = text.rfind(" (os error ") else {
        return text.to_owned();
    };
    let digits = &text[end + " (os error ".len()..];
    let Some(code) = digits.strip_suffix(')').and_then(|d| d.parse::<i32>().ok()) else {
        return text.to_owned();
    };
    let Some(advice) = advice(code) else {
        return text.to_owned();
    };
    // The OS text is the last sentence or clause before the code: it starts
    // after the last ": " of the message.
    let head = &text[..end];
    let cut = head.rfind(": ").map_or(0, |i| i + 2);
    if cut == 0 {
        return capitalised(advice);
    }
    format!("{}{advice}", &head[..cut])
}

fn capitalised(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

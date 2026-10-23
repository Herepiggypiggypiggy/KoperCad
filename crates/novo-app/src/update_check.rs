//! Help > About > Check for updates.
//!
//! The program works offline and never phones home on its own. This check runs
//! only when the user presses the button, asks GitHub's public release list
//! for the project, and reports whether a newer stable version exists. It does
//! not download or install anything; the answer links to the releases page.
//!
//! Release candidates (`v0.1.0-rc3`) share the crate version of the final
//! release, so a running release candidate can only be told about a newer
//! *version*, not about a newer candidate of the same version.

use std::time::Duration;

/// The release list of the public repository.
pub const FEED_URL: &str =
    "https://api.github.com/repos/Herepiggypiggypiggy/kopercad/releases?per_page=30";
/// Where the answer sends the user.
pub const RELEASES_PAGE: &str = "https://github.com/Herepiggypiggypiggy/kopercad/releases";
/// A file path (or an `https://` URL) read instead of GitHub; a QA hook that
/// no screen or setting mentions. Plain `http://` is refused.
pub const FEED_ENV: &str = "KOPERCAD_UPDATE_FEED";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    UpToDate,
    Newer { tag: String, url: String },
    Failed(String),
}

/// `major.minor.patch` with an optional release candidate number. A final
/// release sorts after its own candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Version {
    core: (u32, u32, u32),
    /// `u32::MAX` for a final release, otherwise the rc number.
    rank: u32,
}

fn parse_tag(tag: &str) -> Option<Version> {
    let t = tag.strip_prefix('v').unwrap_or(tag);
    let (core, rank) = match t.split_once("-rc") {
        Some((core, n)) => (core, n.parse::<u32>().ok()?),
        None => (t, u32::MAX),
    };
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(Version {
        core: (major, minor, patch),
        rank,
    })
}

/// The newest stable release in a GitHub release list that is newer than
/// `current` (the crate version, e.g. "0.1.0"). Drafts, candidates and tags
/// that are not versions (such as `experimental`) are ignored.
pub fn newest_newer(current: &str, feed_json: &str) -> Outcome {
    let Some(current) = parse_tag(current) else {
        return Outcome::Failed(format!(
            "This program's version {current:?} is not readable"
        ));
    };
    let list: serde_json::Value = match serde_json::from_str(feed_json) {
        Ok(v) => v,
        Err(e) => return Outcome::Failed(format!("The release list could not be read: {e}")),
    };
    let Some(items) = list.as_array() else {
        let why = list
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("it is not a list of releases");
        return Outcome::Failed(format!("The release list could not be used: {why}"));
    };
    let mut best: Option<(Version, String, String)> = None;
    for item in items {
        let flag = |k: &str| item.get(k).and_then(|v| v.as_bool()).unwrap_or(false);
        if flag("draft") || flag("prerelease") {
            continue;
        }
        let Some(tag) = item.get("tag_name").and_then(|t| t.as_str()) else {
            continue;
        };
        let Some(v) = parse_tag(tag) else { continue };
        if v.rank != u32::MAX || v <= current {
            continue;
        }
        if best.as_ref().is_none_or(|(b, _, _)| v > *b) {
            // Only a page on github.com becomes a link.
            let url = item
                .get("html_url")
                .and_then(|u| u.as_str())
                .filter(|u| u.starts_with("https://github.com/"))
                .unwrap_or(RELEASES_PAGE)
                .to_string();
            best = Some((v, tag.to_string(), url));
        }
    }
    match best {
        Some((_, tag, url)) => Outcome::Newer { tag, url },
        None => Outcome::UpToDate,
    }
}

fn read_feed(source: Option<&str>) -> Result<String, String> {
    match source {
        Some(src) if src.starts_with("http://") => Err(format!(
            "{FEED_ENV}: plain http is not used, give an https address or a file"
        )),
        Some(src) if !src.starts_with("https://") => {
            std::fs::read_to_string(src).map_err(|e| format!("{src}: {e}"))
        }
        Some(url) => get(url),
        None => get(FEED_URL),
    }
}

fn get(url: &str) -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .into();
    let mut resp = agent
        .get(url)
        .header(
            "User-Agent",
            concat!("KoperCAD/", env!("CARGO_PKG_VERSION")),
        )
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("No answer from GitHub ({e}). The program itself works offline"))?;
    if !resp.status().is_success() {
        let code = resp.status().as_u16();
        if code == 403 || code == 429 {
            return Err(format!(
                "GitHub refused the request (status {code}); it allows only a few unsigned requests per hour from one address, try again later"
            ));
        }
        return Err(format!("GitHub answered with status {code}"));
    }
    resp.body_mut()
        .read_to_string()
        .map_err(|e| format!("The answer could not be read: {e}"))
}

/// Blocking: run it on a thread.
pub fn check() -> Outcome {
    check_from(std::env::var(FEED_ENV).ok().as_deref())
}

fn check_from(source: Option<&str>) -> Outcome {
    match read_feed(source) {
        Ok(json) => newest_newer(env!("CARGO_PKG_VERSION"), &json),
        Err(e) => Outcome::Failed(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(entries: &[(&str, bool, bool)]) -> String {
        let items: Vec<String> = entries
            .iter()
            .map(|(t, d, p)| {
                format!(
                    r#"{{"tag_name":"{t}","draft":{d},"prerelease":{p},"html_url":"https://github.com/example/{t}"}}"#
                )
            })
            .collect();
        format!("[{}]", items.join(","))
    }

    #[test]
    fn a_newer_stable_release_is_offered_with_its_page() {
        let f = feed(&[("v0.1.1", false, false), ("v0.1.0", false, false)]);
        assert_eq!(
            newest_newer("0.1.0", &f),
            Outcome::Newer {
                tag: "v0.1.1".into(),
                url: "https://github.com/example/v0.1.1".into()
            }
        );
    }

    #[test]
    fn the_highest_of_several_is_chosen_whatever_the_order() {
        let f = feed(&[
            ("v0.2.0", false, false),
            ("v0.10.0", false, false),
            ("v0.9.0", false, false),
        ]);
        let Outcome::Newer { tag, .. } = newest_newer("0.1.0", &f) else {
            panic!("expected newer");
        };
        assert_eq!(tag, "v0.10.0");
    }

    #[test]
    fn the_same_or_an_older_version_is_up_to_date() {
        let f = feed(&[("v0.1.0", false, false), ("v0.0.9", false, false)]);
        assert_eq!(newest_newer("0.1.0", &f), Outcome::UpToDate);
    }

    #[test]
    fn candidates_drafts_prereleases_and_odd_tags_are_not_offered() {
        let f = feed(&[
            ("v0.3.0-rc1", false, false),
            ("v0.4.0", true, false),
            ("v0.5.0", false, true),
            ("experimental", false, true),
            ("latest-build", false, false),
            ("v1.2", false, false),
            ("v1.2.3.4", false, false),
        ]);
        assert_eq!(newest_newer("0.1.0", &f), Outcome::UpToDate);
    }

    #[test]
    fn a_final_release_sorts_after_its_own_candidates() {
        assert!(parse_tag("v0.1.0").unwrap() > parse_tag("v0.1.0-rc9").unwrap());
        assert!(parse_tag("v0.1.0-rc10").unwrap() > parse_tag("v0.1.0-rc2").unwrap());
        assert!(parse_tag("v0.1.0-rc1").unwrap() > parse_tag("v0.0.9").unwrap());
    }

    #[test]
    fn an_error_message_from_github_is_reported_not_hidden() {
        let out = newest_newer("0.1.0", r#"{"message":"API rate limit exceeded"}"#);
        let Outcome::Failed(why) = out else {
            panic!("expected failure");
        };
        assert!(why.contains("rate limit"), "{why}");
    }

    #[test]
    fn garbage_and_an_empty_list_are_handled() {
        assert!(matches!(
            newest_newer("0.1.0", "<html>"),
            Outcome::Failed(_)
        ));
        assert_eq!(newest_newer("0.1.0", "[]"), Outcome::UpToDate);
        assert!(matches!(newest_newer("x", "[]"), Outcome::Failed(_)));
    }

    #[test]
    fn a_link_off_github_falls_back_to_the_releases_page() {
        let f = r#"[{"tag_name":"v0.2.0","draft":false,"prerelease":false,"html_url":"https://evil.test/x"}]"#;
        assert_eq!(
            newest_newer("0.1.0", f),
            Outcome::Newer {
                tag: "v0.2.0".into(),
                url: RELEASES_PAGE.into()
            }
        );
    }

    #[test]
    fn plain_http_feeds_are_refused() {
        let Outcome::Failed(why) = check_from(Some("http://example.test/feed")) else {
            panic!("expected failure");
        };
        assert!(why.contains("plain http"), "{why}");
    }

    #[test]
    fn a_feed_file_stands_in_for_github() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("feed.json");
        std::fs::write(&p, feed(&[("v99.0.0", false, false)])).unwrap();
        let out = check_from(p.to_str());
        assert!(matches!(out, Outcome::Newer { .. }), "{out:?}");
    }
}

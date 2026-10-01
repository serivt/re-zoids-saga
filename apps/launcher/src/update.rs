//! Whether a newer version of the port is out: when the launcher's screen
//! opens it asks the project's repository for its tags, on a thread of its
//! own, and takes the highest of those named `vX.Y.Z` as the latest
//! release. When it is newer than this build the main screen offers a line
//! that opens the releases page in the web browser. Without the network,
//! or with an answer it cannot read, nothing shows.
//!
//! Source of knowledge: this project's own design; the tags come from
//! GitHub's REST API (`GET /repos/{owner}/{repo}/tags`), and the project
//! tags its releases `vMAJOR.MINOR.PATCH` (semantic versioning).

use std::fmt;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use serde::Deserialize;

/// The project's tags, most at once, and its releases page.
const TAGS_URL: &str = "https://api.github.com/repos/serivt/re-zoids-saga/tags?per_page=100";
/// The page the update line opens.
pub const RELEASES_URL: &str = "https://github.com/serivt/re-zoids-saga/releases";
/// How long the check may take, and how large its answer may be.
const TIMEOUT: Duration = Duration::from_secs(10);
const ANSWER_LIMIT: u64 = 256 * 1024;
/// What the requests say they come from, as GitHub's API asks.
const USER_AGENT: &str = concat!("re-zoids-saga-launcher/", env!("CARGO_PKG_VERSION"));

/// A release's version: major, minor and patch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u32, pub u32, pub u32);

impl Version {
    /// The version `text` names as `X.Y.Z`, three whole numbers without
    /// leading zeros, or `None`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split('.').map(|part| {
            let well_formed = !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (part == "0" || !part.starts_with('0'));
            well_formed.then(|| part.parse().ok()).flatten()
        });
        let version = Self(parts.next()??, parts.next()??, parts.next()??);
        parts.next().is_none().then_some(version)
    }

    /// The version a release's tag names: `vX.Y.Z`, or `None` for any
    /// other tag.
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        Self::parse(tag.strip_prefix('v')?)
    }

    /// This build's version.
    #[must_use]
    pub fn current() -> Option<Self> {
        Self::parse(env!("CARGO_PKG_VERSION"))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "v{}.{}.{}", self.0, self.1, self.2)
    }
}

#[derive(Deserialize)]
struct Tag {
    name: String,
}

/// The newest release `text`, the API's list of tags, names, when it is
/// newer than `current`.
///
/// # Errors
///
/// Returns the reason when `text` is not a list of tags.
pub fn newer_release(text: &str, current: Version) -> Result<Option<Version>, String> {
    let tags: Vec<Tag> = serde_json::from_str(text).map_err(|error| error.to_string())?;
    Ok(tags
        .iter()
        .filter_map(|tag| Version::from_tag(&tag.name))
        .max()
        .filter(|latest| *latest > current))
}

/// The check running on its own thread; its answer arrives while the
/// screen goes on.
pub struct UpdateCheck {
    answer: Receiver<Result<Option<Version>, String>>,
}

impl UpdateCheck {
    /// Starts asking for the project's tags.
    #[must_use]
    pub fn start() -> Self {
        let (sender, answer) = channel();
        std::thread::spawn(move || {
            let result = Version::current()
                .ok_or_else(|| "this build has no version".to_owned())
                .and_then(|current| {
                    let text = fetch()?;
                    newer_release(&text, current)
                });
            let _ = sender.send(result);
        });
        Self { answer }
    }

    /// The newer release once the check has finished, or why it could not
    /// tell; `None` while it runs.
    #[must_use]
    pub fn answer(&self) -> Option<Result<Option<Version>, String>> {
        self.answer.try_recv().ok()
    }
}

/// The project's tags as the API lists them.
fn fetch() -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .build()
        .into();
    let mut response = agent
        .get(TAGS_URL)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|error| error.to_string())?;
    response
        .body_mut()
        .with_config()
        .limit(ANSWER_LIMIT)
        .read_to_string()
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn only_v_tags_of_three_whole_numbers_are_versions() {
        assert_eq!(Version::from_tag("v0.3.0"), Some(Version(0, 3, 0)));
        assert_eq!(Version::from_tag("v10.2.11"), Some(Version(10, 2, 11)));
        for tag in [
            "0.1.0",
            "v0.3",
            "v0.3.0.1",
            "v0.4.0-rc1",
            "v01.2.3",
            "vx.y.z",
            "v",
            "",
        ] {
            assert_eq!(Version::from_tag(tag), None, "{tag}");
        }
        assert_eq!(Version(1, 2, 3).to_string(), "v1.2.3");
    }

    #[test]
    fn versions_order_by_their_numbers_not_their_text() {
        assert!(Version(0, 10, 0) > Version(0, 9, 9));
        assert!(Version(1, 0, 0) > Version(0, 99, 99));
        assert!(Version(0, 3, 1) > Version(0, 3, 0));
    }

    #[test]
    fn the_newest_release_counts_only_when_newer_than_this_build() {
        let tags = r#"[{"name":"v0.2.0"},{"name":"v0.10.0"},{"name":"0.99.0"},{"name":"v0.4.0-rc1"},{"name":"v0.9.1"}]"#;
        assert_eq!(
            newer_release(tags, Version(0, 3, 0)).unwrap(),
            Some(Version(0, 10, 0))
        );
        assert_eq!(newer_release(tags, Version(0, 10, 0)).unwrap(), None);
        assert_eq!(newer_release("[]", Version(0, 3, 0)).unwrap(), None);
        assert!(newer_release("{\"message\":\"rate limited\"}", Version(0, 3, 0)).is_err());
    }

    #[test]
    fn this_build_has_a_version() {
        assert!(Version::current().is_some());
    }

    /// Needs the network: `cargo test -p launcher -- --ignored`.
    #[test]
    #[ignore = "asks the project's repository for its tags"]
    fn reads_the_project_s_tags() {
        let check = UpdateCheck::start();
        let answer = (0..150).find_map(|_| {
            std::thread::sleep(Duration::from_millis(100));
            check.answer()
        });
        assert!(matches!(answer, Some(Ok(_))), "{answer:?}");
    }
}

//! Pure SemVer release-tag parsing and update comparison (`vX.Y.Z`).

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SemVerTag {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl SemVerTag {
    /// Parse a release tag like `v0.0.1` or `0.0.1` (exact `X.Y.Z`, no prerelease suffix).
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        let body = trimmed.strip_prefix('v').unwrap_or(trimmed);
        if !body.split('.').all(|part| {
            !part.is_empty()
                && part.bytes().all(|b| b.is_ascii_digit())
                && (part == "0" || !part.starts_with('0'))
        }) {
            return None;
        }
        let mut parts = body.split('.');
        let major = parts.next()?.parse::<u64>().ok()?;
        let minor = parts.next()?.parse::<u64>().ok()?;
        let patch = parts.next()?.parse::<u64>().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
        })
    }

    pub fn version_string(self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }

    pub fn tag_string(self) -> String {
        format!("v{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl fmt::Display for SemVerTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Parse `git ls-remote --tags` stdout into sorted unique release tags (`vX.Y.Z`).
/// Ignores peeled tag refs (`^{}`) and non-SemVer tags.
pub fn parse_ls_remote_tags(stdout: &str) -> Vec<SemVerTag> {
    let mut tags = Vec::new();
    for line in stdout.lines() {
        let Some(ref_name) = line.split_whitespace().nth(1) else {
            continue;
        };
        if ref_name.ends_with("^{}") {
            continue;
        }
        let Some(tag_name) = ref_name.strip_prefix("refs/tags/") else {
            continue;
        };
        if !tag_name.starts_with('v') {
            continue;
        }
        if let Some(parsed) = SemVerTag::parse(tag_name) {
            tags.push(parsed);
        }
    }
    tags.sort();
    tags.dedup();
    tags
}

/// Return the highest SemVer tag present in `tags`, if any.
pub fn latest_tag(tags: &[SemVerTag]) -> Option<SemVerTag> {
    tags.iter().copied().max()
}

/// Return the highest SemVer tag strictly newer than `current`, if any exists.
pub fn latest_newer_tag(current: SemVerTag, tags: &[SemVerTag]) -> Option<SemVerTag> {
    latest_tag(tags).filter(|&latest| latest > current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exact_semver_tags_only() {
        assert_eq!(
            SemVerTag::parse("v0.0.1"),
            Some(SemVerTag {
                major: 0,
                minor: 0,
                patch: 1
            })
        );
        assert_eq!(
            SemVerTag::parse("1.12.3"),
            Some(SemVerTag {
                major: 1,
                minor: 12,
                patch: 3
            })
        );
        assert_eq!(SemVerTag::parse("v0.0.1-rc.1"), None);
        assert_eq!(SemVerTag::parse("latest"), None);
        assert_eq!(SemVerTag::parse("v0.1"), None);
    }

    #[test]
    fn detects_newer_tags_numerically_from_ls_remote_output() {
        let ls_remote = "\
1111111111111111111111111111111111111111\trefs/tags/v0.0.1
2222222222222222222222222222222222222222\trefs/tags/v0.0.2
3333333333333333333333333333333333333333\trefs/tags/v0.0.2^{}
4444444444444444444444444444444444444444\trefs/tags/v0.0.10
5555555555555555555555555555555555555555\trefs/tags/not-a-tag
6666666666666666666666666666666666666666\trefs/tags/v0.0.11-rc.1
";
        let tags = parse_ls_remote_tags(ls_remote);
        assert_eq!(
            tags.iter().map(|t| t.tag_string()).collect::<Vec<_>>(),
            vec!["v0.0.1", "v0.0.2", "v0.0.10"]
        );
        let current = SemVerTag::parse("v0.0.1").expect("valid tag");
        assert_eq!(
            latest_newer_tag(current, &tags).map(|t| t.tag_string()),
            Some("v0.0.10".to_owned())
        );
        let up_to_date = SemVerTag::parse("v0.0.10").expect("valid tag");
        assert_eq!(latest_newer_tag(up_to_date, &tags), None);
    }
}

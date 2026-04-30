//! Parse various git remote URL forms into a structured [`RepoSlug`].

use crate::error::GitError;

/// A parsed git remote pointing at a CNB-style repository.
///
/// `owner_path` may contain `/` to express subgroups (e.g. `cnb/sub/deep`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoSlug {
    pub host: String,
    pub owner_path: String,
    pub repo: String,
}

impl RepoSlug {
    /// `owner/path/.../repo` (no host).
    pub fn full_path(&self) -> String {
        format!("{}/{}", self.owner_path, self.repo)
    }
}

/// Parse a git remote URL of any common form.
///
/// Supports:
/// - HTTPS:    `https://cnb.cool/owner/repo[.git]`
/// - SSH-scp:  `git@cnb.cool:owner/repo[.git]`
/// - SSH-url:  `ssh://git@cnb.cool/owner/repo[.git]`
/// - Subgroups: any of the above with multi-segment owner path
pub fn parse_remote_url(input: &str) -> Result<RepoSlug, GitError> {
    let s = input.trim();
    if s.is_empty() {
        return Err(GitError::Parse {
            url: input.into(),
            reason: "empty url".into(),
        });
    }

    // scp-style: `user@host:path`
    if !s.contains("://") && s.contains(':') && !s.contains(' ') {
        if let Some((host_part, path_part)) = s.split_once(':') {
            let host = host_part
                .rsplit('@')
                .next()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| GitError::Parse {
                    url: input.into(),
                    reason: "missing host".into(),
                })?;
            return split_path(host, path_part).ok_or_else(|| GitError::Parse {
                url: input.into(),
                reason: "missing repo segment".into(),
            });
        }
    }

    // URL-style: ssh://git@host/path  or  https://host/path
    let parsed = url::Url::parse(s).map_err(|e| GitError::Parse {
        url: input.into(),
        reason: e.to_string(),
    })?;
    let host = parsed.host_str().ok_or_else(|| GitError::Parse {
        url: input.into(),
        reason: "missing host".into(),
    })?;
    split_path(host, parsed.path().trim_start_matches('/')).ok_or_else(|| GitError::Parse {
        url: input.into(),
        reason: "missing repo segment".into(),
    })
}

fn split_path(host: &str, path: &str) -> Option<RepoSlug> {
    let path = path.trim_start_matches('/').trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let repo = segs.pop()?;
    if segs.is_empty() {
        return None;
    }
    Some(RepoSlug {
        host: host.to_owned(),
        owner_path: segs.join("/"),
        repo: repo.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(host: &str, owner: &str, repo: &str) -> RepoSlug {
        RepoSlug {
            host: host.into(),
            owner_path: owner.into(),
            repo: repo.into(),
        }
    }

    #[test]
    fn https_with_dot_git() {
        let r = parse_remote_url("https://cnb.cool/cnb/feedback.git").unwrap();
        assert_eq!(r, s("cnb.cool", "cnb", "feedback"));
    }

    #[test]
    fn https_without_dot_git() {
        let r = parse_remote_url("https://cnb.cool/cnb/feedback").unwrap();
        assert_eq!(r, s("cnb.cool", "cnb", "feedback"));
    }

    #[test]
    fn ssh_scp_form() {
        let r = parse_remote_url("git@cnb.cool:cnb/feedback.git").unwrap();
        assert_eq!(r, s("cnb.cool", "cnb", "feedback"));
    }

    #[test]
    fn ssh_url_form() {
        let r = parse_remote_url("ssh://git@cnb.cool/cnb/feedback.git").unwrap();
        assert_eq!(r, s("cnb.cool", "cnb", "feedback"));
    }

    #[test]
    fn subgroup_path() {
        let r = parse_remote_url("https://cnb.cool/cnb/sub/repo.git").unwrap();
        assert_eq!(r, s("cnb.cool", "cnb/sub", "repo"));
        assert_eq!(r.full_path(), "cnb/sub/repo");
    }

    #[test]
    fn rejects_empty() {
        assert!(parse_remote_url("").is_err());
    }

    #[test]
    fn rejects_no_repo_segment() {
        assert!(parse_remote_url("https://cnb.cool/").is_err());
    }
}

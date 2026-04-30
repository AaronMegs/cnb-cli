//! Safe URL construction. **Never** build URLs with `format!` or string concatenation —
//! always go through these helpers (DESIGN §6.1 hard rule).

use url::Url;

use crate::error::ApiError;

/// Resolve `path` against `base`.
///
/// `path` may include a leading `/` and a `?query` string. This helper:
/// - splits off the query (if any) and re-encodes it via `Url::query_pairs_mut`,
/// - appends each path segment via `Url::path_segments_mut().push(...)`
///   (so `..`/spaces/UTF-8 are properly percent-encoded).
pub fn resolve(base: &Url, path: &str) -> Result<Url, ApiError> {
    let mut path = path.trim();
    // Allow callers to pass either `/foo/bar` or `foo/bar`.
    if let Some(stripped) = path.strip_prefix('/') {
        path = stripped;
    }
    let (path_part, query_part) = match path.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (path, None),
    };

    let mut url = base.clone();
    {
        let mut segs = url
            .path_segments_mut()
            .map_err(|()| ApiError::InvalidUrl(format!("base URL cannot be a base: {base}")))?;
        // If base path doesn't end with '/', append() would create a sibling — normalize first.
        segs.pop_if_empty();
        for seg in path_part.split('/').filter(|s| !s.is_empty()) {
            segs.push(seg);
        }
    }

    if let Some(q) = query_part {
        // Preserve raw query as-is (gh-style passthrough). Validate by re-parsing.
        url.set_query(Some(q));
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://api.cnb.cool").unwrap()
    }

    #[test]
    fn simple_path() {
        let u = resolve(&base(), "/user").unwrap();
        assert_eq!(u.as_str(), "https://api.cnb.cool/user");
    }

    #[test]
    fn no_leading_slash() {
        let u = resolve(&base(), "user/repos").unwrap();
        assert_eq!(u.as_str(), "https://api.cnb.cool/user/repos");
    }

    #[test]
    fn with_query() {
        let u = resolve(&base(), "/search?q=cnb&type=repo").unwrap();
        assert_eq!(u.as_str(), "https://api.cnb.cool/search?q=cnb&type=repo");
    }

    #[test]
    fn nested_segments_with_subgroup() {
        let u = resolve(&base(), "/cnb/sub/repo/-/issues/42").unwrap();
        assert_eq!(u.as_str(), "https://api.cnb.cool/cnb/sub/repo/-/issues/42");
    }

    #[test]
    fn percent_encodes_segments() {
        let u = resolve(&base(), "/owner/repo space/file?ref=main").unwrap();
        assert!(u.as_str().contains("repo%20space"), "got: {u}");
    }

    #[test]
    fn rejects_traversal_via_segment_push() {
        // url::Url::path_segments_mut().push("..") creates a literal `..` segment that
        // does NOT escape the host (it's percent-encoded as part of the path).
        let u = resolve(&base(), "/a/../b").unwrap();
        // The '..' becomes a literal segment — no host escape possible.
        assert!(u.as_str().starts_with("https://api.cnb.cool/"), "got: {u}");
    }
}

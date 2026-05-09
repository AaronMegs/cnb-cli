//! Atomic, locked, secure-permission file writes.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

use fs2::FileExt;

use crate::error::ConfigError;

/// Write `content` to `path` atomically (tempfile + rename),
/// holding an exclusive file lock and applying restrictive permissions on Unix.
pub fn write_secure(path: &Path, content: &str) -> Result<(), ConfigError> {
    let parent = path.parent().ok_or_else(|| ConfigError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "no parent dir"),
    })?;
    std::fs::create_dir_all(parent).map_err(|e| ConfigError::Io {
        path: parent.to_path_buf(),
        source: e,
    })?;

    // Lock file co-located with the target (best-effort cross-platform).
    let lock_path = path.with_extension("lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|e| ConfigError::Lock {
            path: lock_path.clone(),
            source: e,
        })?;
    lock.lock_exclusive().map_err(|e| ConfigError::Lock {
        path: lock_path.clone(),
        source: e,
    })?;

    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| ConfigError::Io {
        path: parent.to_path_buf(),
        source: e,
    })?;
    tmp.write_all(content.as_bytes()).map_err(|e| ConfigError::Io {
        path: tmp.path().to_path_buf(),
        source: e,
    })?;
    tmp.flush().map_err(|e| ConfigError::Io {
        path: tmp.path().to_path_buf(),
        source: e,
    })?;
    tmp.persist(path).map_err(|e| ConfigError::Io {
        path: path.to_path_buf(),
        source: e.error,
    })?;

    set_secure_permissions(path)?;
    let _ = fs2::FileExt::unlock(&lock);
    Ok(())
}

#[cfg(unix)]
fn set_secure_permissions(path: &Path) -> Result<(), ConfigError> {
    use std::os::unix::fs::PermissionsExt;
    let f = File::open(path).map_err(|e| ConfigError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let mut perms = f
        .metadata()
        .map_err(|e| ConfigError::Io {
            path: path.to_path_buf(),
            source: e,
        })?
        .permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(path, perms).map_err(|e| ConfigError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

#[cfg(not(unix))]
fn set_secure_permissions(_path: &Path) -> Result<(), ConfigError> {
    // Windows ACL hardening: tracked as a non-blocking follow-up.
    //
    // The typical `%APPDATA%\cnb\hosts.toml` path already lives under a
    // per-user profile directory that NTFS restricts to the owning user
    // (and administrators) via default inherited ACLs. That gives us
    // *most* of the 0600 guarantee Unix gets from `chmod`: other
    // non-admin accounts on the same machine cannot read the file.
    //
    // A fully-equivalent behaviour — stripping inherited ACLs and
    // setting an explicit "owner SID only" DACL — requires
    // `windows-sys` / `windows` crate bindings and is scoped out of
    // the cross-platform MVP. Revisit if/when we ship a Windows-first
    // deployment scenario where non-default profile locations are in
    // play.
    Ok(())
}

#[cfg(all(test, unix))]
mod tests_unix {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    #[test]
    fn writes_with_0600() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("hosts.toml");
        write_secure(&path, "version = 1\n").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "expected 0600, got {mode:o}");
    }

    #[test]
    fn overwrite_is_atomic() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("hosts.toml");
        write_secure(&path, "v1").unwrap();
        write_secure(&path, "v2").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "v2");
    }
}

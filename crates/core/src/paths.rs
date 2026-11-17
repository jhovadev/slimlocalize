use std::path::{Component, Path, PathBuf};

use crate::error::{DomainError, Result};

/// Canonicalize an existing import source and require it to be a regular file
/// or directory. Symlinks are resolved; dangling links are reported as I/O.
pub fn canonicalize_existing(path: &Path) -> Result<PathBuf> {
    let canonical = std::fs::canonicalize(path).map_err(|source| DomainError::Io {
        context: format!("cannot resolve {}", path.display()),
        source,
    })?;
    if !canonical.is_file() && !canonical.is_dir() {
        return Err(DomainError::validation(
            "path",
            "must point to a regular file or directory",
        ));
    }
    Ok(canonical)
}

/// Join a user-controlled relative name to a trusted root while preventing
/// absolute paths, `..` traversal and symlink escapes.
pub fn safe_join(base: &Path, relative: impl AsRef<Path>) -> Result<PathBuf> {
    let relative = relative.as_ref();
    if relative.is_absolute() {
        return Err(DomainError::PathTraversal {
            path: relative.to_path_buf(),
        });
    }
    for component in relative.components() {
        match component {
            Component::Normal(_) => {}
            _ => {
                return Err(DomainError::PathTraversal {
                    path: relative.to_path_buf(),
                });
            }
        }
    }

    let canonical_base = canonicalize_existing(base)?;
    if !canonical_base.is_dir() {
        return Err(DomainError::validation("base", "must be a directory"));
    }

    let candidate = canonical_base.join(relative);
    if candidate.exists() {
        let canonical_candidate = canonicalize_existing(&candidate)?;
        if !canonical_candidate.starts_with(&canonical_base) {
            return Err(DomainError::PathTraversal { path: candidate });
        }
        return Ok(canonical_candidate);
    }

    let parent = candidate
        .parent()
        .ok_or_else(|| DomainError::PathTraversal {
            path: candidate.clone(),
        })?;
    let canonical_parent = canonicalize_existing(parent)?;
    if !canonical_parent.starts_with(&canonical_base) {
        return Err(DomainError::PathTraversal { path: candidate });
    }
    Ok(canonical_parent.join(
        candidate
            .file_name()
            .ok_or_else(|| DomainError::PathTraversal {
                path: candidate.clone(),
            })?,
    ))
}

/// Verify that `candidate` (which may not exist yet) stays inside `base`.
pub fn ensure_within(base: &Path, candidate: &Path) -> Result<()> {
    let canonical_base = canonicalize_existing(base)?;
    let parent = candidate.parent().unwrap_or_else(|| Path::new("."));
    let canonical_parent = if parent.exists() {
        canonicalize_existing(parent)?
    } else {
        canonicalize_existing(base)?
    };
    if !canonical_parent.starts_with(&canonical_base) {
        return Err(DomainError::PathTraversal {
            path: candidate.to_path_buf(),
        });
    }
    Ok(())
}

/// Create an output directory and verify that it is a real directory.
pub fn ensure_directory(path: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(path).map_err(|source| DomainError::Io {
        context: format!("cannot create directory {}", path.display()),
        source,
    })?;
    canonicalize_existing(path)
}

/// Read a file while enforcing a hard size limit. This avoids accidental
/// multi-gigabyte imports when a directory is selected.
pub fn read_limited_file(path: &Path, max_bytes: u64) -> Result<Vec<u8>> {
    let metadata = std::fs::metadata(path).map_err(|source| DomainError::Io {
        context: format!("cannot inspect {}", path.display()),
        source,
    })?;
    if metadata.len() > max_bytes {
        return Err(DomainError::LimitExceeded {
            context: format!("file {}", path.display()),
            limit: max_bytes,
        });
    }
    std::fs::read(path).map_err(|source| DomainError::Io {
        context: format!("cannot read {}", path.display()),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_parent_traversal() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            safe_join(dir.path(), "../escape.json"),
            Err(DomainError::PathTraversal { .. })
        ));
        assert!(matches!(
            safe_join(dir.path(), "/etc/passwd"),
            Err(DomainError::PathTraversal { .. })
        ));
    }

    #[test]
    fn accepts_simple_relative_names() {
        let dir = tempfile::tempdir().unwrap();
        let joined = safe_join(dir.path(), "en-US.json").unwrap();
        assert!(joined.starts_with(dir.path().canonicalize().unwrap()));
    }
}

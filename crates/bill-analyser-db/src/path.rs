use std::path::{Path, PathBuf};

use crate::{DbError, DbResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqliteDbPath {
    path: PathBuf,
}

impl SqliteDbPath {
    pub fn temporary_file(path: impl AsRef<Path>) -> DbResult<Self> {
        let absolute = absolute_path(path.as_ref())?;
        deny_real_data_path(&absolute)?;
        ensure_under_temp_root(&absolute)?;
        ensure_existing_targets_stay_under_temp(&absolute)?;
        Ok(Self { path: absolute })
    }

    pub fn as_path(&self) -> &Path {
        &self.path
    }
}

fn absolute_path(path: &Path) -> DbResult<PathBuf> {
    if path.as_os_str().is_empty() {
        return Err(DbError::UnsafePath("empty database path".to_string()));
    }

    if path == Path::new(":memory:") || path == Path::new("file::memory:?cache=shared") {
        return Err(DbError::UnsafePath(
            "in-memory sqlite paths are not allowed for this runtime guard".to_string(),
        ));
    }

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };

    Ok(normalize_lexically(&absolute))
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn deny_real_data_path(path: &Path) -> DbResult<()> {
    let real_data_path =
        normalize_lexically(&std::env::current_dir()?.join("data").join("bills.db"));
    if path == real_data_path {
        return Err(DbError::UnsafePath(
            "refusing to open real data/bills.db from Rust DB foundation".to_string(),
        ));
    }
    Ok(())
}

fn ensure_under_temp_root(path: &Path) -> DbResult<()> {
    let temp_root = normalize_lexically(&std::env::temp_dir());
    if path.starts_with(&temp_root) {
        return Ok(());
    }

    Err(DbError::UnsafePath(format!(
        "database path must be a temp/copy DB under {}",
        temp_root.display()
    )))
}

fn ensure_existing_targets_stay_under_temp(path: &Path) -> DbResult<()> {
    let canonical_temp_root = std::fs::canonicalize(normalize_lexically(&std::env::temp_dir()))?;
    let canonical_real_data_path = canonical_real_data_path()?;

    if std::fs::symlink_metadata(path).is_ok() {
        let canonical_path = std::fs::canonicalize(path).map_err(|error| {
            DbError::UnsafePath(format!(
                "existing database path cannot be resolved before opening: {error}"
            ))
        })?;
        ensure_canonical_target_is_safe(
            &canonical_path,
            &canonical_temp_root,
            canonical_real_data_path.as_deref(),
        )?;
    }

    if let Some(existing_parent) = nearest_existing_parent(path) {
        let canonical_parent = std::fs::canonicalize(existing_parent)?;
        ensure_canonical_target_is_safe(
            &canonical_parent,
            &canonical_temp_root,
            canonical_real_data_path.as_deref(),
        )?;
    }

    Ok(())
}

fn canonical_real_data_path() -> DbResult<Option<PathBuf>> {
    let real_data_path =
        normalize_lexically(&std::env::current_dir()?.join("data").join("bills.db"));
    if std::fs::symlink_metadata(&real_data_path).is_ok() {
        return Ok(Some(std::fs::canonicalize(real_data_path)?));
    }
    Ok(None)
}

fn nearest_existing_parent(path: &Path) -> Option<&Path> {
    let mut parent = path.parent();
    while let Some(candidate) = parent {
        if std::fs::symlink_metadata(candidate).is_ok() {
            return Some(candidate);
        }
        parent = candidate.parent();
    }
    None
}

fn ensure_canonical_target_is_safe(
    target: &Path,
    temp_root: &Path,
    real_data_path: Option<&Path>,
) -> DbResult<()> {
    if real_data_path.is_some_and(|real_data_path| target == real_data_path) {
        return Err(DbError::UnsafePath(
            "refusing to open canonical target for real data/bills.db from Rust DB foundation"
                .to_string(),
        ));
    }

    if target.starts_with(temp_root) {
        return Ok(());
    }

    Err(DbError::UnsafePath(format!(
        "database path target must stay under temp root {}; resolved target was {}",
        temp_root.display(),
        target.display()
    )))
}

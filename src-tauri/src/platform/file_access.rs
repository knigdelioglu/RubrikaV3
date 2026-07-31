use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

pub fn atomic_write<P: AsRef<Path>>(path: P, content: &str) -> std::io::Result<()> {
    atomic_write_bytes(path, content.as_bytes())
}

pub fn atomic_write_bytes<P: AsRef<Path>>(path: P, content: &[u8]) -> std::io::Result<()> {
    let path = path.as_ref();
    let parent = path.parent().unwrap_or(Path::new(""));
    if !parent.exists() && parent != Path::new("") {
        fs::create_dir_all(parent)?;
    }

    let tmp_path = path.with_extension("tmp");
    if let Ok(metadata) = fs::symlink_metadata(&tmp_path) {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Atomic temporary target is not a regular file.",
            ));
        }
        fs::remove_file(&tmp_path)?;
    }
    let mut tmp_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp_path)?;
    if let Err(error) = tmp_file
        .write_all(content)
        .and_then(|_| tmp_file.sync_all())
    {
        let _ = fs::remove_file(&tmp_path);
        return Err(error);
    }

    if let Err(error) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(error);
    }
    // Persist the directory entry when the platform supports syncing a
    // directory. The rename is already atomic; this closes the durability
    // window after a successful replacement.
    if let Ok(parent_file) = OpenOptions::new().read(true).open(parent) {
        let _ = parent_file.sync_all();
    }
    Ok(())
}

pub fn remove_file_within(base_dir: &Path, candidate: &Path) -> std::io::Result<bool> {
    if !candidate.exists() {
        return Ok(false);
    }
    let metadata = std::fs::symlink_metadata(candidate)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Only regular non-symlink files may be removed.",
        ));
    }
    let canonical_base = std::fs::canonicalize(base_dir)?;
    let canonical_candidate = std::fs::canonicalize(candidate)?;
    if canonical_candidate.strip_prefix(&canonical_base).is_err() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Removal target is outside the allowed project directory.",
        ));
    }
    std::fs::remove_file(canonical_candidate)?;
    Ok(true)
}

pub fn remove_dir_within(base_dir: &Path, candidate: &Path) -> std::io::Result<bool> {
    if !candidate.exists() {
        return Ok(false);
    }
    let metadata = std::fs::symlink_metadata(candidate)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Only regular non-symlink directories may be removed.",
        ));
    }
    let canonical_base = std::fs::canonicalize(base_dir)?;
    let canonical_candidate = std::fs::canonicalize(candidate)?;
    if canonical_candidate == canonical_base
        || canonical_candidate.strip_prefix(&canonical_base).is_err()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Removal target is outside the allowed project directory.",
        ));
    }
    std::fs::remove_dir_all(canonical_candidate)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{remove_dir_within, remove_file_within};

    #[test]
    fn removal_is_limited_to_the_allowed_directory() {
        let root =
            std::env::temp_dir().join(format!("rubrika-safe-remove-{}", uuid::Uuid::new_v4()));
        let allowed = root.join("documents");
        let outside = root.join("outside.pdf");
        std::fs::create_dir_all(&allowed).expect("allowed directory");
        std::fs::write(&outside, b"outside").expect("outside file");

        let error = remove_file_within(&allowed, &outside)
            .expect_err("an outside target must never be deleted");
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        assert!(outside.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn regular_project_artifacts_can_be_removed() {
        let root =
            std::env::temp_dir().join(format!("rubrika-safe-remove-{}", uuid::Uuid::new_v4()));
        let allowed = root.join("cache");
        let file = allowed.join("preview.png");
        let directory = allowed.join("page-previews");
        std::fs::create_dir_all(&directory).expect("artifact directory");
        std::fs::write(&file, b"preview").expect("artifact file");

        assert!(remove_file_within(&allowed, &file).expect("safe file removal"));
        assert!(remove_dir_within(&allowed, &directory).expect("safe directory removal"));
        assert!(!file.exists());
        assert!(!directory.exists());
        let _ = std::fs::remove_dir_all(root);
    }
}

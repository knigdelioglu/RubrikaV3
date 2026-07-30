use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

pub fn atomic_write<P: AsRef<Path>>(path: P, content: &str) -> std::io::Result<()> {
    let path = path.as_ref();
    let parent = path.parent().unwrap_or(Path::new(""));
    if !parent.exists() && parent != Path::new("") {
        fs::create_dir_all(parent)?;
    }

    let tmp_path = path.with_extension("tmp");
    let mut tmp_file = File::create(&tmp_path)?;
    tmp_file.write_all(content.as_bytes())?;
    tmp_file.sync_all()?;

    fs::rename(tmp_path, path)?;
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
    if !canonical_candidate.starts_with(&canonical_base) {
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
    if canonical_candidate == canonical_base || !canonical_candidate.starts_with(&canonical_base) {
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

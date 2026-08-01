use std::path::PathBuf;

use crate::domain::errors::{AppError, AppErrorCode};
use crate::platform::project_paths::{ManagedProjectPath, TrustedProjectRoot};
use crate::services::project_store::ProjectStore;

/// Hard bound for a single managed asset read.
pub const MAX_MANAGED_ASSET_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ManagedAsset {
    pub bytes: Vec<u8>,
    pub mime: String,
    pub resolved_path: String,
}

/// Resolves an opaque managed-asset request.
///
/// The request path has the form `/<project_id>/<relative-managed-path>`.
/// Absolute paths, traversal, backslashes, symlink escapes and outside-root
/// targets are rejected. The read is bounded to `MAX_MANAGED_ASSET_BYTES`.
/// Resolution is based on the canonical project root held by the backend,
/// so a project moved to a new location keeps working without widening the
/// Tauri asset scope.
pub fn resolve_managed_asset(
    project_store: &ProjectStore,
    request_path: &str,
) -> Result<ManagedAsset, AppError> {
    let trimmed = request_path.trim_start_matches('/');
    if trimmed.is_empty() {
        return Err(asset_error("empty managed asset request"));
    }
    let mut segments = trimmed.splitn(2, '/');
    let project_id = segments
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| asset_error("missing project id"))?;
    let relative = segments
        .next()
        .ok_or_else(|| asset_error("missing asset path"))?;
    if project_id.contains('\\') || project_id.contains('\0') {
        return Err(asset_error("unsafe project id"));
    }

    let project = project_store
        .get_project_snapshot(project_id.to_string())
        .map_err(|_| asset_error("project is not open"))?;
    let trusted_root =
        TrustedProjectRoot::from_canonical_root(PathBuf::from(&project.root_path), false)?;
    let managed = ManagedProjectPath::parse(relative)?;
    let path = trusted_root.resolve_existing_file(&managed)?;
    let metadata = std::fs::metadata(&path)
        .map_err(|error| asset_error(&format!("asset metadata: {error}")))?;
    if metadata.len() > MAX_MANAGED_ASSET_BYTES {
        return Err(asset_error("asset exceeds size bound"));
    }
    let bytes =
        std::fs::read(&path).map_err(|error| asset_error(&format!("asset read: {error}")))?;
    let mime = mime_for_path(&path);
    Ok(ManagedAsset {
        bytes,
        mime: mime.to_string(),
        resolved_path: path.to_string_lossy().to_string(),
    })
}

fn mime_for_path(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("pdf") => "application/pdf",
        Some("wav") => "audio/wav",
        Some("mp3") => "audio/mpeg",
        Some("m4a") => "audio/mp4",
        Some("ogg") => "audio/ogg",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

fn asset_error(detail: &str) -> AppError {
    AppError {
        code: AppErrorCode::ManagedPathOutsideProject,
        message: "İçerik güvenli biçimde çözümlenemedi.".to_string(),
        recoverable: false,
        suggested_action: None,
        technical_details: Some(detail.to_string()),
        correlation_id: uuid::Uuid::new_v4().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_project() -> (ProjectStore, std::path::PathBuf, String) {
        let root =
            std::env::temp_dir().join(format!("rubrika-managed-asset-{}", uuid::Uuid::new_v4()));
        let store = ProjectStore::new();
        let project = store
            .create_project_with_setup(
                "Asset".to_string(),
                root.to_string_lossy().to_string(),
                None,
                None,
                None,
            )
            .expect("project");
        (
            store,
            std::path::PathBuf::from(project.root_path),
            project.id,
        )
    }

    #[test]
    fn relative_preview_asset_is_served() {
        let (store, root, project_id) = fixture_project();
        let document_dir = root.join("outputs/previews/doc-1/generations/g1");
        std::fs::create_dir_all(&document_dir).unwrap();
        std::fs::write(document_dir.join("page_1.png"), b"PNGDATA").unwrap();
        let request = format!(
            "/{}/outputs/previews/doc-1/generations/g1/page_1.png",
            project_id
        );
        let asset = resolve_managed_asset(&store, &request).expect("asset");
        assert_eq!(asset.bytes, b"PNGDATA");
        assert_eq!(asset.mime, "image/png");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn traversal_and_absolute_paths_are_rejected() {
        let (store, root, project_id) = fixture_project();
        for path in [
            format!("/{}/../outside.txt", project_id),
            format!("/{}/documents/../../escape", project_id),
            format!("/{}/%2e%2e/escape", project_id),
        ] {
            assert!(resolve_managed_asset(&store, &path).is_err());
        }
        assert!(resolve_managed_asset(&store, "/not-open-project/x.png").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn symlink_outside_project_is_rejected() {
        let (store, root, project_id) = fixture_project();
        let outside =
            std::env::temp_dir().join(format!("rubrika-asset-outside-{}", uuid::Uuid::new_v4()));
        std::fs::write(&outside, b"SECRET").unwrap();
        let symlink_target_dir = root.join("outputs/previews/doc-1/generations/g1");
        std::fs::create_dir_all(&symlink_target_dir).unwrap();
        std::os::unix::fs::symlink(&outside, symlink_target_dir.join("evil.png")).unwrap();
        let request = format!(
            "/{}/outputs/previews/doc-1/generations/g1/evil.png",
            project_id
        );
        let result = resolve_managed_asset(&store, &request);
        assert!(result.is_err(), "symlink escape must be rejected");
        let _ = std::fs::remove_file(&outside);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn moved_project_is_served_by_canonical_root() {
        let (_store, root, project_id) = fixture_project();
        let new_location =
            std::env::temp_dir().join(format!("rubrika-moved-assets-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&new_location).unwrap();
        copy_dir(&root, &new_location);
        // Re-open the project at its new location.
        let store2 = ProjectStore::new();
        let reopened = store2
            .open_project(new_location.to_string_lossy().to_string())
            .expect("reopen moved project");
        assert_eq!(reopened.id, project_id);
        let document_dir = new_location.join("outputs/previews/doc-1/generations/g1");
        std::fs::create_dir_all(&document_dir).unwrap();
        std::fs::write(document_dir.join("page_1.png"), b"MOVED").unwrap();
        let request = format!(
            "/{}/outputs/previews/doc-1/generations/g1/page_1.png",
            project_id
        );
        let asset = resolve_managed_asset(&store2, &request).expect("moved asset");
        assert_eq!(asset.bytes, b"MOVED");
        let _ = std::fs::remove_dir_all(&new_location);
        let _ = std::fs::remove_dir_all(&root);
    }

    fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
        fn copy_recursive(from: &std::path::Path, to: &std::path::Path) {
            for entry in std::fs::read_dir(from).unwrap() {
                let entry = entry.unwrap();
                let target = to.join(entry.file_name());
                if entry.file_type().unwrap().is_dir() {
                    std::fs::create_dir_all(&target).unwrap();
                    copy_recursive(&entry.path(), &target);
                } else {
                    std::fs::copy(entry.path(), target).unwrap();
                }
            }
        }
        std::fs::create_dir_all(to).unwrap();
        copy_recursive(from, to);
    }
}

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::sync::{Mutex, OnceLock};

/// Error messages returned after the rename must be treated as an uncertain
/// commit. The rename may already be durable even when the directory sync
/// fails, so callers must never roll the target back blindly.
pub fn is_durability_uncertain(error: &std::io::Error) -> bool {
    error
        .to_string()
        .contains("parent directory fsync failed after rename")
}

pub fn is_durability_unsupported(error: &std::io::Error) -> bool {
    error
        .to_string()
        .contains("parent directory fsync unsupported")
}

#[cfg(test)]
thread_local! {
    static FAILPOINT: RefCell<Option<String>> = const { RefCell::new(None) };
}
#[cfg(test)]
static FAILPOINT_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(test)]
pub(crate) fn set_test_failpoint(name: Option<&str>) {
    FAILPOINT.with(|slot| {
        *slot.borrow_mut() = name.map(str::to_string);
    });
}

#[cfg(test)]
pub(crate) fn test_failpoint_guard() -> std::sync::MutexGuard<'static, ()> {
    FAILPOINT_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("failpoint test lock")
}

fn failpoint(name: &str) -> std::io::Result<()> {
    #[cfg(test)]
    {
        let active = FAILPOINT.with(|slot| slot.borrow().clone());
        let sleep_before_rename = name == "atomic_before_rename"
            && active.as_deref() == Some("atomic_before_rename_sleep");
        if active.as_deref() == Some(name) || sleep_before_rename {
            if sleep_before_rename {
                std::thread::sleep(std::time::Duration::from_secs(5));
                return Ok(());
            }
            let message = if matches!(
                name,
                "atomic_after_rename" | "atomic_before_parent_sync" | "atomic_after_parent_sync"
            ) {
                format!("parent directory fsync failed after rename: test failpoint: {name}")
            } else {
                format!("test failpoint: {name}")
            };
            return Err(std::io::Error::other(message));
        }
    }
    let _ = name;
    Ok(())
}

fn failpoint_enabled(name: &str) -> bool {
    #[cfg(test)]
    {
        FAILPOINT.with(|slot| slot.borrow().as_deref() == Some(name))
    }
    #[cfg(not(test))]
    {
        let _ = name;
        false
    }
}

pub fn atomic_write<P: AsRef<Path>>(path: P, content: &str) -> std::io::Result<()> {
    atomic_write_bytes(path, content.as_bytes())
}

/// Publishes a same-filesystem staged file and then syncs its parent
/// directory. A post-rename sync error is returned as an explicit uncertain
/// commit; callers must not delete or restore the target blindly.
pub fn durable_rename<P: AsRef<Path>, Q: AsRef<Path>>(
    staging: P,
    target: Q,
) -> std::io::Result<()> {
    let staging = staging.as_ref();
    let target = target.as_ref();
    let parent = target.parent().unwrap_or(Path::new(""));
    if staging.parent() != target.parent() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Durable rename requires a same-directory staging file.",
        ));
    }
    let metadata = fs::symlink_metadata(staging)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Durable rename source is not a regular file.",
        ));
    }
    if fs::symlink_metadata(target)
        .as_ref()
        .is_ok_and(|value| value.file_type().is_symlink() || value.is_dir())
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Durable rename target is not a regular file.",
        ));
    }
    fs::rename(staging, target)?;
    let parent_file = OpenOptions::new()
        .read(true)
        .open(parent)
        .map_err(|error| {
            std::io::Error::other(format!(
                "parent directory fsync failed after rename: open: {error}"
            ))
        })?;
    match parent_file.sync_all() {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::Unsupported => Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            format!("parent directory fsync unsupported: {error}"),
        )),
        Err(error) => Err(std::io::Error::other(format!(
            "parent directory fsync failed after rename: {error}"
        ))),
    }
}

/// Publishes a staged directory and then syncs its parent directory. The
/// target must not already exist; generation directories are immutable once
/// published and are never replaced in place.
pub fn durable_rename_directory<P: AsRef<Path>, Q: AsRef<Path>>(
    staging: P,
    target: Q,
) -> std::io::Result<()> {
    let staging = staging.as_ref();
    let target = target.as_ref();
    let parent = target.parent().unwrap_or(Path::new(""));
    if staging.parent() != target.parent() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Durable directory rename requires a same-directory staging path.",
        ));
    }
    let staging_metadata = fs::symlink_metadata(staging)?;
    if staging_metadata.file_type().is_symlink() || !staging_metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Durable directory rename source is not a regular directory.",
        ));
    }
    if fs::symlink_metadata(target).is_ok() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "Durable directory rename target already exists.",
        ));
    }
    fs::rename(staging, target)?;
    let parent_file = OpenOptions::new()
        .read(true)
        .open(parent)
        .map_err(|error| {
            std::io::Error::other(format!(
                "parent directory fsync failed after rename: open: {error}"
            ))
        })?;
    match parent_file.sync_all() {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::Unsupported => Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            format!("parent directory fsync unsupported: {error}"),
        )),
        Err(error) => Err(std::io::Error::other(format!(
            "parent directory fsync failed after rename: {error}"
        ))),
    }
}

pub fn atomic_write_bytes<P: AsRef<Path>>(path: P, content: &[u8]) -> std::io::Result<()> {
    let path = path.as_ref();
    let parent = path.parent().unwrap_or(Path::new(""));
    if !parent.exists() && parent != Path::new("") {
        fs::create_dir_all(parent)?;
    }

    let parent_metadata = fs::symlink_metadata(parent)?;
    if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Atomic write parent is not a regular directory.",
        ));
    }

    let tmp_path = path.with_extension("tmp");
    if let Ok(metadata) = fs::symlink_metadata(&tmp_path) {
        let _ = metadata;
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "Atomic temporary target already exists; recovery must classify it first.",
        ));
    }
    let mut tmp_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp_path)?;
    if let Err(error) = failpoint("atomic_after_temp_create") {
        let _ = fs::remove_file(&tmp_path);
        return Err(error);
    }
    let write_result = (|| -> std::io::Result<()> {
        if content.len() > 1 && failpoint_enabled("atomic_after_partial_write") {
            tmp_file.write_all(&content[..content.len() / 2])?;
            failpoint("atomic_after_partial_write")?;
        }
        tmp_file.write_all(content)?;
        tmp_file.flush()?;
        failpoint("atomic_after_file_sync")?;
        tmp_file.sync_all()?;
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&tmp_path);
        return Err(error);
    }

    let target_metadata = fs::symlink_metadata(path);
    if target_metadata
        .as_ref()
        .is_ok_and(|metadata| metadata.file_type().is_symlink() || metadata.is_dir())
    {
        let _ = fs::remove_file(&tmp_path);
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Atomic write target changed to a non-regular file.",
        ));
    }
    if let Err(error) = failpoint("atomic_before_rename") {
        let _ = fs::remove_file(&tmp_path);
        return Err(error);
    }
    if let Err(error) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(error);
    }
    failpoint("atomic_after_rename")?;

    // Persist the directory entry. A failure here is not a normal write
    // failure: the rename may already have happened and the canonical target
    // must be reloaded by the caller before it can classify the outcome.
    failpoint("atomic_before_parent_sync")?;
    let parent_file = OpenOptions::new()
        .read(true)
        .open(parent)
        .map_err(|error| {
            std::io::Error::other(format!(
                "parent directory fsync failed after rename: open: {error}"
            ))
        })?;
    match parent_file.sync_all() {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::Unsupported => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                format!("parent directory fsync unsupported: {error}"),
            ));
        }
        Err(error) => {
            return Err(std::io::Error::other(format!(
                "parent directory fsync failed after rename: {error}"
            )));
        }
    }
    failpoint("atomic_after_parent_sync")?;
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
    use std::path::Path;
    use std::process::{Command, Stdio};

    use super::{
        atomic_write_bytes, is_durability_uncertain, remove_dir_within, remove_file_within,
        set_test_failpoint, test_failpoint_guard,
    };

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

    #[test]
    fn failed_atomic_write_preserves_a_complete_previous_or_new_state() {
        let _guard = test_failpoint_guard();
        let root =
            std::env::temp_dir().join(format!("rubrika-atomic-proof-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("root");
        let target = root.join("project.json");
        std::fs::write(&target, b"old-complete").expect("old state");

        for point in [
            "atomic_after_temp_create",
            "atomic_after_partial_write",
            "atomic_after_file_sync",
            "atomic_before_rename",
        ] {
            set_test_failpoint(Some(point));
            let result = atomic_write_bytes(&target, b"new-complete");
            set_test_failpoint(None);
            assert!(result.is_err(), "failpoint {point} must fail");
            let bytes = std::fs::read(&target).expect("canonical target");
            assert!(bytes == b"old-complete" || bytes == b"new-complete");
            assert!(!target.with_extension("tmp").exists());
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn proof_33_failed_atomic_write_preserves_previous_project() {
        failed_atomic_write_preserves_a_complete_previous_or_new_state();
    }

    #[test]
    fn rename_then_parent_sync_failure_is_explicitly_uncertain() {
        let _guard = test_failpoint_guard();
        let root =
            std::env::temp_dir().join(format!("rubrika-atomic-uncertain-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("root");
        let target = root.join("project.json");
        std::fs::write(&target, b"old").expect("old state");
        set_test_failpoint(Some("atomic_before_parent_sync"));
        let result = atomic_write_bytes(&target, b"new");
        set_test_failpoint(None);
        let error = result.expect_err("durability must be uncertain");
        assert!(is_durability_uncertain(&error));
        let bytes = std::fs::read(&target).expect("canonical target");
        assert_eq!(bytes, b"new");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn proof_51_parent_sync_failure_never_reports_saved() {
        rename_then_parent_sync_failure_is_explicitly_uncertain();
    }

    #[test]
    fn proof_43_disk_full_never_reports_success() {
        let _guard = test_failpoint_guard();
        let root =
            std::env::temp_dir().join(format!("rubrika-disk-fault-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("root");
        let target = root.join("project.json");
        std::fs::write(&target, b"old-complete-json").expect("old state");
        set_test_failpoint(Some("atomic_after_file_sync"));
        let result = atomic_write_bytes(&target, b"new-complete-json");
        set_test_failpoint(None);
        assert!(result.is_err(), "simulated disk fault must fail the write");
        assert_eq!(
            std::fs::read(&target).expect("canonical target"),
            b"old-complete-json"
        );
        let temp_target = target.with_extension("tmp");
        if temp_target.exists() {
            assert_eq!(
                std::fs::read(&temp_target).expect("temporary target"),
                b"new-complete-json"
            );
            std::fs::remove_file(temp_target).expect("temporary target cleanup");
        }
        let _ = std::fs::remove_dir_all(root);
    }

    /// proof_34: a real child process killed while the atomic writer is
    /// between temp-file sync and rename leaves the old complete JSON.
    #[test]
    fn proof_34_process_kill_cannot_leave_partial_canonical_json() {
        let root =
            std::env::temp_dir().join(format!("rubrika-atomic-kill-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("root");
        let target = root.join("project.json");
        std::fs::write(&target, b"old-complete-json").expect("old state");
        let mut child = Command::new(std::env::current_exe().expect("test binary"))
            .args([
                "--exact",
                "platform::file_access::tests::proof_34_atomic_write_child",
                "--ignored",
                "--nocapture",
            ])
            .env("RUBRIKA_PROOF_ATOMIC_PATH", &target)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn atomic child");
        std::thread::sleep(std::time::Duration::from_millis(150));
        unsafe {
            libc::kill(child.id() as i32, libc::SIGKILL);
        }
        let _ = child.wait().expect("wait atomic child");
        assert_eq!(
            std::fs::read(&target).expect("canonical target"),
            b"old-complete-json"
        );
        let temp_target = target.with_extension("tmp");
        if temp_target.exists() {
            assert_eq!(
                std::fs::read(&temp_target).expect("temporary target"),
                b"new-complete-json"
            );
            std::fs::remove_file(temp_target).expect("temporary target cleanup");
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    #[ignore = "child fixture invoked by proof_34_process_kill_cannot_leave_partial_canonical_json"]
    fn proof_34_atomic_write_child() {
        let Some(path) = std::env::var_os("RUBRIKA_PROOF_ATOMIC_PATH") else {
            return;
        };
        set_test_failpoint(Some("atomic_before_rename_sleep"));
        let _ = atomic_write_bytes(Path::new(&path), b"new-complete-json");
        set_test_failpoint(None);
    }
}

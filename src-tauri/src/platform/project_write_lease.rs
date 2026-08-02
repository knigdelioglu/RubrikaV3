use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use crate::domain::errors::{AppError, AppErrorCode};

/// Name of the OS-level advisory lock file inside a project root.
const LOCK_FILE_NAME: &str = ".rubrika.lock";

/// Process-wide registry of held leases keyed by canonical project root.
///
/// Multiple ProjectStore instances inside one process share the same OS
/// lease for a root (the in-process project mutex already serializes
/// writers). A second OS process still fails `flock` and receives
/// `ProjectAlreadyOpen`.
static PROCESS_LEASES: OnceLock<Mutex<HashMap<PathBuf, Weak<ProjectWriteLease>>>> = OnceLock::new();

/// An OS-owned advisory write lease for a canonical project root.
///
/// The lease uses a real `flock(LOCK_EX | LOCK_NB)` on a file inside the
/// project root. The OS releases the lock automatically when the process
/// exits or crashes; stale metadata PID files are never trusted.
#[derive(Debug)]
pub struct ProjectWriteLease {
    file: File,
    lock_path: PathBuf,
}

impl ProjectWriteLease {
    /// Acquires an exclusive non-blocking lease for `root`.
    ///
    /// `root` must exist and be canonical. If another process (or another
    /// open file description) holds the lease, `ProjectAlreadyOpen` is
    /// returned.
    pub fn acquire(root: &Path) -> Result<Self, AppError> {
        Self::acquire_internal(root, true)
    }

    /// Acquires an existing lock file without ever creating or truncating it.
    /// Read-only backup/preflight paths use this variant so a missing lock is
    /// reported as a consistency problem instead of becoming a source write.
    pub fn acquire_existing(root: &Path) -> Result<Self, AppError> {
        Self::acquire_internal(root, false)
    }

    fn acquire_internal(root: &Path, create: bool) -> Result<Self, AppError> {
        let root = root.canonicalize().map_err(|error| AppError {
            code: AppErrorCode::ProjectLoadFailed,
            message: "Proje klasörü çözümlenemedi.".to_string(),
            recoverable: true,
            suggested_action: Some("Klasörün erişilebilir olduğunu doğrulayın.".to_string()),
            technical_details: Some(format!("canonicalize failed: {error}")),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        })?;
        let lock_path = root.join(LOCK_FILE_NAME);
        let mut options = OpenOptions::new();
        if create {
            options.read(true).write(true).create(true).truncate(false);
        } else {
            options.read(true);
        }
        let file = options.open(&lock_path).map_err(|error| AppError {
            code: AppErrorCode::ProjectLoadFailed,
            message: "Proje yazma kilidi açılamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("Klasör izinlerini kontrol edin.".to_string()),
            technical_details: Some(format!("lock file open failed: {error}")),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        })?;

        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result != 0 {
            let io_error = std::io::Error::last_os_error();
            if io_error.kind() == std::io::ErrorKind::WouldBlock {
                return Err(AppError {
                    code: AppErrorCode::ProjectAlreadyOpen,
                    message: "Bu proje başka bir Rubrika penceresinde açık.".to_string(),
                    recoverable: true,
                    suggested_action: Some(
                        "Diğer Rubrika penceresinde projeyi kapatıp tekrar deneyin.".to_string(),
                    ),
                    technical_details: None,
                    correlation_id: uuid::Uuid::new_v4().to_string(),
                });
            }
            return Err(AppError {
                code: AppErrorCode::ProjectLoadFailed,
                message: "Proje yazma kilidi alınamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Tekrar deneyin.".to_string()),
                technical_details: Some(format!("flock failed: {io_error}")),
                correlation_id: uuid::Uuid::new_v4().to_string(),
            });
        }

        Ok(Self { file, lock_path })
    }

    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }
}

/// Acquires the lease for `root`, sharing the process-wide lease when this
/// process already holds one for the same canonical root.
pub fn acquire_or_share(root: &Path) -> Result<Arc<ProjectWriteLease>, AppError> {
    let canonical = root.canonicalize().map_err(|error| AppError {
        code: AppErrorCode::ProjectLoadFailed,
        message: "Proje klasörü çözümlenemedi.".to_string(),
        recoverable: true,
        suggested_action: Some("Klasörün erişilebilir olduğunu doğrulayın.".to_string()),
        technical_details: Some(format!("canonicalize failed: {error}")),
        correlation_id: uuid::Uuid::new_v4().to_string(),
    })?;
    let registry = PROCESS_LEASES.get_or_init(|| Mutex::new(HashMap::new()));
    {
        let map = registry
            .lock()
            .map_err(|_| lease_registry_error("process lease registry lock failed"))?;
        if let Some(lease) = map.get(&canonical).and_then(Weak::upgrade) {
            return Ok(lease);
        }
    }
    let lease = Arc::new(ProjectWriteLease::acquire(&canonical)?);
    {
        let mut map = registry
            .lock()
            .map_err(|_| lease_registry_error("process lease registry lock failed"))?;
        map.retain(|_, weak| weak.strong_count() > 0);
        map.insert(canonical, Arc::downgrade(&lease));
    }
    Ok(lease)
}

/// Shares an already-held process lease or obtains an exclusive lease from an
/// existing lock file without creating any file in the project. This is the
/// only lease helper permitted to be used by source-preserving backup paths.
pub fn acquire_or_share_existing(root: &Path) -> Result<Arc<ProjectWriteLease>, AppError> {
    let canonical = root.canonicalize().map_err(|error| AppError {
        code: AppErrorCode::ProjectLoadFailed,
        message: "Proje klasörü çözümlenemedi.".to_string(),
        recoverable: true,
        suggested_action: Some("Klasörün erişilebilir olduğunu doğrulayın.".to_string()),
        technical_details: Some(format!("canonicalize failed: {error}")),
        correlation_id: uuid::Uuid::new_v4().to_string(),
    })?;
    let registry = PROCESS_LEASES.get_or_init(|| Mutex::new(HashMap::new()));
    {
        let map = registry
            .lock()
            .map_err(|_| lease_registry_error("process lease registry lock failed"))?;
        if let Some(lease) = map.get(&canonical).and_then(Weak::upgrade) {
            return Ok(lease);
        }
    }
    let lease = Arc::new(ProjectWriteLease::acquire_existing(&canonical)?);
    let mut map = registry
        .lock()
        .map_err(|_| lease_registry_error("process lease registry lock failed"))?;
    map.retain(|_, weak| weak.strong_count() > 0);
    map.insert(canonical, Arc::downgrade(&lease));
    Ok(lease)
}

fn lease_registry_error(detail: &str) -> AppError {
    AppError {
        code: AppErrorCode::ProjectLoadFailed,
        message: "Proje yazma kilidi yönetilemedi.".to_string(),
        recoverable: true,
        suggested_action: None,
        technical_details: Some(detail.to_string()),
        correlation_id: uuid::Uuid::new_v4().to_string(),
    }
}

impl Drop for ProjectWriteLease {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn lease_excludes_second_writer_and_releases_after_drop() {
        let raw_root =
            std::env::temp_dir().join(format!("rubrika-lease-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&raw_root).unwrap();
        let root = raw_root.canonicalize().unwrap_or(raw_root.clone());

        let first = Arc::new(ProjectWriteLease::acquire(&root).expect("first lease"));
        let second = ProjectWriteLease::acquire(&root);
        assert!(second.is_err());
        assert_eq!(second.unwrap_err().code, AppErrorCode::ProjectAlreadyOpen);

        drop(first);
        let retry = ProjectWriteLease::acquire(&root).expect("lease after release");
        drop(retry);
        let _ = std::fs::remove_dir_all(&raw_root);
    }

    #[test]
    fn lock_file_lives_inside_project_root() {
        let raw_root =
            std::env::temp_dir().join(format!("rubrika-lease-path-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&raw_root).unwrap();
        let root = raw_root.canonicalize().unwrap_or(raw_root.clone());
        let lease = ProjectWriteLease::acquire(&root).expect("lease");
        assert!(lease.lock_path().starts_with(&root));
        assert!(lease.lock_path().ends_with(".rubrika.lock"));
        drop(lease);
        let _ = std::fs::remove_dir_all(&raw_root);
    }

    #[test]
    fn same_process_shares_lease_but_cross_process_does_not() {
        let raw_root =
            std::env::temp_dir().join(format!("rubrika-lease-share-{}", uuid::Uuid::new_v4()));
        let root = raw_root.canonicalize().unwrap_or(raw_root.clone());
        std::fs::create_dir_all(&root).unwrap();

        let first = acquire_or_share(&root).expect("first shared lease");
        let second = acquire_or_share(&root).expect("same-process share");
        assert!(Arc::ptr_eq(&first, &second));
        // A direct flock on a fresh file description still fails while the
        // process holds the shared lease (cross-process semantics).
        assert_eq!(
            ProjectWriteLease::acquire(&root).unwrap_err().code,
            AppErrorCode::ProjectAlreadyOpen
        );
        drop(first);
        drop(second);
        let _ = std::fs::remove_dir_all(&raw_root);
    }
}

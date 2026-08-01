use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;

use crate::domain::errors::{AppError, AppErrorCode};

const INSTANCE_LOCK_RELATIVE: [&str; 3] = ["Library", "Application Support", "RubrikaV3"];
const INSTANCE_LOCK_FILE: &str = "rubrika.instance.lock";

/// OS-backed single-instance lease.
///
/// The first Rubrika process holds an exclusive `flock` on a well-known
/// path in the per-user application-support directory. A second process
/// cannot start a writer; the OS releases the lock automatically if the
/// first process crashes. The official Tauri single-instance plugin was
/// intentionally not used because its dependency graph could not be
/// resolved in this environment's offline registry.
#[derive(Debug)]
pub struct AppInstanceLease {
    file: File,
    path: PathBuf,
}

impl AppInstanceLease {
    pub fn acquire() -> Result<Self, AppError> {
        let dir = instance_lock_dir();
        std::fs::create_dir_all(&dir).map_err(|error| AppError {
            code: AppErrorCode::PermissionDenied,
            message: "Uygulama kilidi klasörü oluşturulamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("Uygulama destek klasörüne erişimi kontrol edin.".to_string()),
            technical_details: Some(format!("create_dir_all failed: {error}")),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        })?;
        let path = dir.join(INSTANCE_LOCK_FILE);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|error| AppError {
                code: AppErrorCode::PermissionDenied,
                message: "Uygulama kilidi açılamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Tekrar deneyin.".to_string()),
                technical_details: Some(format!("instance lock open failed: {error}")),
                correlation_id: uuid::Uuid::new_v4().to_string(),
            })?;

        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result != 0 {
            let io_error = std::io::Error::last_os_error();
            if io_error.kind() == std::io::ErrorKind::WouldBlock {
                return Err(AppError {
                    code: AppErrorCode::AppAlreadyRunning,
                    message: "Rubrika zaten çalışıyor.".to_string(),
                    recoverable: false,
                    suggested_action: Some("Açık Rubrika penceresine geçin.".to_string()),
                    technical_details: None,
                    correlation_id: uuid::Uuid::new_v4().to_string(),
                });
            }
            return Err(AppError {
                code: AppErrorCode::PermissionDenied,
                message: "Uygulama kilidi alınamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Tekrar deneyin.".to_string()),
                technical_details: Some(format!("flock failed: {io_error}")),
                correlation_id: uuid::Uuid::new_v4().to_string(),
            });
        }
        Ok(Self { file, path })
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for AppInstanceLease {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

fn instance_lock_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("RUBRIKA_V3_APP_LOCK_DIR") {
        return PathBuf::from(path);
    }
    let mut base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    for component in INSTANCE_LOCK_RELATIVE {
        base.push(component);
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_instance_cannot_start_while_first_runs() {
        let dir =
            std::env::temp_dir().join(format!("rubrika-instance-test-{}", uuid::Uuid::new_v4()));
        std::env::set_var("RUBRIKA_V3_APP_LOCK_DIR", &dir);

        let first = AppInstanceLease::acquire().expect("first instance");
        let second = AppInstanceLease::acquire();
        assert!(second.is_err());
        assert_eq!(second.unwrap_err().code, AppErrorCode::AppAlreadyRunning);
        drop(first);

        let retry = AppInstanceLease::acquire().expect("lease after first instance exits");
        drop(retry);
        let _ = std::fs::remove_dir_all(dir);
    }
}

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use crate::domain::errors::{AppError, AppErrorCode};
use crate::platform::file_access::{atomic_write, atomic_write_bytes as write_bytes};

const PROJECT_FILE_NAME: &str = "project.json";

/// The only filesystem root that may be used for a loaded project session.
///
/// The value is created from the path explicitly selected by the user and is
/// canonicalized before it is stored. `Project.root_path` is never used to
/// construct this value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedProjectRoot {
    root: PathBuf,
    project_file: PathBuf,
}

/// Canonical path representation for files managed by a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedProjectPath {
    relative: PathBuf,
    serialized: String,
}

impl ManagedProjectPath {
    pub fn parse(raw: &str) -> Result<Self, AppError> {
        if raw.trim().is_empty() || raw.contains('\0') || raw.contains('\\') {
            return Err(unsafe_path_error(
                AppErrorCode::UnsafeManagedPath,
                "Bu dosya yolu proje içindeki güvenli biçime uymuyor.",
                raw,
            ));
        }

        let bytes = raw.as_bytes();
        if raw.starts_with('/')
            || raw.starts_with("//")
            || (bytes.len() >= 2 && bytes[1] == b':')
            || Path::new(raw).is_absolute()
        {
            return Err(unsafe_path_error(
                AppErrorCode::UnsafeManagedPath,
                "Mutlak dosya yolları proje içindeki yönetilen dosyalar için kullanılamaz.",
                raw,
            ));
        }

        let mut relative = PathBuf::new();
        let mut serialized_components = Vec::new();
        for component in Path::new(raw).components() {
            match component {
                Component::Normal(value) => {
                    let value = value.to_str().ok_or_else(|| {
                        unsafe_path_error(
                            AppErrorCode::UnsafeManagedPath,
                            "Dosya yolu geçerli bir metin içermiyor.",
                            raw,
                        )
                    })?;
                    if value.is_empty() || value.contains('\0') {
                        return Err(unsafe_path_error(
                            AppErrorCode::UnsafeManagedPath,
                            "Dosya yolu geçersiz bir bileşen içeriyor.",
                            raw,
                        ));
                    }
                    relative.push(value);
                    serialized_components.push(value.to_string());
                }
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(unsafe_path_error(
                        AppErrorCode::UnsafeManagedPath,
                        "Dosya yolunda üst klasör veya kök bileşeni kullanılamaz.",
                        raw,
                    ));
                }
            }
        }

        if serialized_components.is_empty() {
            return Err(unsafe_path_error(
                AppErrorCode::UnsafeManagedPath,
                "Boş veya yalnızca nokta olan dosya yolu geçersiz.",
                raw,
            ));
        }

        Ok(Self {
            relative,
            serialized: serialized_components.join("/"),
        })
    }

    pub fn as_path(&self) -> &Path {
        &self.relative
    }

    pub fn as_str(&self) -> &str {
        &self.serialized
    }

    pub fn starts_with_component(&self, component: &str) -> bool {
        self.relative
            .components()
            .next()
            .is_some_and(|value| value == Component::Normal(component.as_ref()))
    }
}

impl TrustedProjectRoot {
    pub fn open_selected(selected: &Path) -> Result<Self, AppError> {
        let selected = canonicalize_absolute(selected, AppErrorCode::ProjectLoadFailed)?;
        let root = if selected.is_file() {
            if selected.file_name().and_then(|name| name.to_str()) != Some(PROJECT_FILE_NAME) {
                return Err(project_path_error(
                    AppErrorCode::ProjectLoadFailed,
                    "Seçilen dosya geçerli bir Rubrika proje dosyası değil.",
                    selected,
                ));
            }
            selected.parent().map(Path::to_path_buf).ok_or_else(|| {
                project_path_error(
                    AppErrorCode::ProjectLoadFailed,
                    "Proje klasörü belirlenemedi.",
                    selected.clone(),
                )
            })?
        } else if selected.is_dir() {
            selected
        } else {
            return Err(project_path_error(
                AppErrorCode::ProjectNotFound,
                "Seçilen proje klasörü bulunamadı.",
                selected,
            ));
        };

        Self::from_canonical_root(root, true)
    }

    pub fn for_create(target: &Path) -> Result<Self, AppError> {
        let target = absolute_path(target)?;
        let canonical_target = canonicalize_with_missing_tail(&target)?;
        if canonical_target.exists() {
            Self::from_canonical_root(canonical_target, false)
        } else {
            Ok(Self {
                project_file: canonical_target.join(PROJECT_FILE_NAME),
                root: canonical_target,
            })
        }
    }

    pub fn from_canonical_root(
        root: PathBuf,
        require_project_file: bool,
    ) -> Result<Self, AppError> {
        let root = fs::canonicalize(&root).map_err(|error| {
            project_io_error(
                if require_project_file {
                    AppErrorCode::ProjectLoadFailed
                } else {
                    AppErrorCode::ProjectSaveFailed
                },
                "Proje klasörü doğrulanamadı.",
                &root,
                error,
            )
        })?;
        if !root.is_dir() {
            return Err(project_path_error(
                AppErrorCode::ProjectLoadFailed,
                "Proje kökü bir klasör olmalı.",
                root,
            ));
        }

        let project_file = root.join(PROJECT_FILE_NAME);
        if require_project_file {
            let metadata = fs::symlink_metadata(&project_file).map_err(|error| {
                project_io_error(
                    AppErrorCode::ProjectNotFound,
                    "Proje klasöründe project.json bulunamadı.",
                    &project_file,
                    error,
                )
            })?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(project_path_error(
                    AppErrorCode::ProjectLoadFailed,
                    "Proje dosyası güvenli bir normal dosya değil.",
                    project_file,
                ));
            }
            let canonical_file = fs::canonicalize(&project_file).map_err(|error| {
                project_io_error(
                    AppErrorCode::ProjectLoadFailed,
                    "Proje dosyası doğrulanamadı.",
                    &project_file,
                    error,
                )
            })?;
            ensure_contained(
                &root,
                &canonical_file,
                AppErrorCode::ManagedPathOutsideProject,
            )?;
            if canonical_file != project_file {
                return Err(project_path_error(
                    AppErrorCode::ManagedPathSymlinkEscape,
                    "Proje dosyası symlink üzerinden açılamaz.",
                    project_file,
                ));
            }
        }

        Ok(Self { root, project_file })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn project_file(&self) -> &Path {
        &self.project_file
    }

    pub fn root_string(&self) -> String {
        self.root.to_string_lossy().to_string()
    }

    pub fn managed(&self, raw: &str) -> Result<ManagedProjectPath, AppError> {
        ManagedProjectPath::parse(raw)
    }

    pub fn adapt_legacy_document_path(&self, raw: &str) -> Result<ManagedProjectPath, AppError> {
        if looks_like_absolute_path(raw) {
            let candidate = Path::new(raw);
            let canonical = fs::canonicalize(candidate).map_err(|error| {
                unsafe_path_io_error(
                    AppErrorCode::LegacyDocumentPathUnresolved,
                    "Eski belge yolu güvenli biçimde çözülemedi.",
                    raw,
                    error,
                )
            })?;
            ensure_contained(
                &self.root,
                &canonical,
                AppErrorCode::ManagedPathOutsideProject,
            )?;
            reject_symlink_components(&self.root, &canonical)?;
            return self.relative_for_existing(&canonical);
        }

        let parsed = ManagedProjectPath::parse(raw)?;
        let direct = self.root.join(parsed.as_path());
        if direct.exists() {
            return Ok(parsed);
        }

        if !parsed.starts_with_component("documents") {
            let legacy_documents = self.root.join("documents").join(parsed.as_path());
            if legacy_documents.exists() {
                return self.relative_for_existing(&fs::canonicalize(&legacy_documents).map_err(
                    |error| {
                        unsafe_path_io_error(
                            AppErrorCode::LegacyDocumentPathUnresolved,
                            "Eski belge yolu güvenli biçimde çözülemedi.",
                            raw,
                            error,
                        )
                    },
                )?);
            }
        }

        Ok(parsed)
    }

    pub fn relative_for_existing(&self, existing: &Path) -> Result<ManagedProjectPath, AppError> {
        let canonical = fs::canonicalize(existing).map_err(|error| {
            unsafe_path_io_error(
                AppErrorCode::LegacyDocumentPathUnresolved,
                "Belge yolu güvenli biçimde çözülemedi.",
                &existing.to_string_lossy(),
                error,
            )
        })?;
        ensure_contained(
            &self.root,
            &canonical,
            AppErrorCode::ManagedPathOutsideProject,
        )?;
        reject_symlink_components(&self.root, &canonical)?;
        let relative = canonical.strip_prefix(&self.root).map_err(|_| {
            unsafe_path_error(
                AppErrorCode::ManagedPathOutsideProject,
                "Bu dosya proje klasörünün dışında olduğu için açılamadı.",
                &canonical.to_string_lossy(),
            )
        })?;
        ManagedProjectPath::parse(&relative.to_string_lossy())
    }

    pub fn resolve_existing_file(&self, managed: &ManagedProjectPath) -> Result<PathBuf, AppError> {
        let candidate = self.root.join(managed.as_path());
        let metadata = fs::symlink_metadata(&candidate).map_err(|error| {
            unsafe_path_io_error(
                AppErrorCode::ManagedPathOutsideProject,
                "Bu dosya proje klasörünün dışında olduğu için açılamadı.",
                managed.as_str(),
                error,
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(unsafe_path_error(
                AppErrorCode::ManagedPathSymlinkEscape,
                "Bu dosya symlink üzerinden güvenli proje alanının dışına çıkıyor.",
                managed.as_str(),
            ));
        }
        if !metadata.is_file() {
            return Err(unsafe_path_error(
                AppErrorCode::ManagedPathOutsideProject,
                "Yönetilen belge normal bir dosya değil.",
                managed.as_str(),
            ));
        }
        let canonical = fs::canonicalize(&candidate).map_err(|error| {
            unsafe_path_io_error(
                AppErrorCode::ManagedPathOutsideProject,
                "Bu dosya proje klasörünün dışında olduğu için açılamadı.",
                managed.as_str(),
                error,
            )
        })?;
        ensure_contained(
            &self.root,
            &canonical,
            AppErrorCode::ManagedPathOutsideProject,
        )?;
        reject_symlink_components(&self.root, &candidate)?;
        Ok(canonical)
    }

    pub fn resolve_existing_directory(
        &self,
        managed: &ManagedProjectPath,
    ) -> Result<PathBuf, AppError> {
        let candidate = self.root.join(managed.as_path());
        let metadata = fs::symlink_metadata(&candidate).map_err(|error| {
            unsafe_path_io_error(
                AppErrorCode::ManagedPathOutsideProject,
                "Proje içi klasör güvenli biçimde açılamadı.",
                managed.as_str(),
                error,
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(unsafe_path_error(
                AppErrorCode::ManagedPathSymlinkEscape,
                "Proje içi klasör symlink veya normal klasör olmayan bir hedef içeriyor.",
                managed.as_str(),
            ));
        }
        let canonical = fs::canonicalize(&candidate).map_err(|error| {
            unsafe_path_io_error(
                AppErrorCode::ManagedPathOutsideProject,
                "Proje içi klasör güvenli biçimde doğrulanamadı.",
                managed.as_str(),
                error,
            )
        })?;
        ensure_contained(
            &self.root,
            &canonical,
            AppErrorCode::ManagedPathOutsideProject,
        )?;
        reject_symlink_components(&self.root, &candidate)?;
        Ok(canonical)
    }

    pub fn managed_for_path(&self, path: &Path) -> Result<ManagedProjectPath, AppError> {
        let relative = path.strip_prefix(&self.root).map_err(|_| {
            unsafe_path_error(
                AppErrorCode::ManagedPathOutsideProject,
                "Yönetilen dosya yolu proje klasörünün dışında.",
                &path.to_string_lossy(),
            )
        })?;
        ManagedProjectPath::parse(&relative.to_string_lossy())
    }

    pub fn prepare_write_target(&self, managed: &ManagedProjectPath) -> Result<PathBuf, AppError> {
        let target = self.root.join(managed.as_path());
        let parent = target.parent().ok_or_else(|| {
            unsafe_path_error(
                AppErrorCode::ManagedPathOutsideProject,
                "Yönetilen dosyanın parent klasörü belirlenemedi.",
                managed.as_str(),
            )
        })?;
        self.ensure_managed_directory(parent)?;
        if let Ok(metadata) = fs::symlink_metadata(&target) {
            if metadata.file_type().is_symlink() {
                return Err(unsafe_path_error(
                    AppErrorCode::ManagedPathSymlinkEscape,
                    "Yazma hedefi symlink olamaz.",
                    managed.as_str(),
                ));
            }
            if metadata.is_dir() {
                return Err(unsafe_path_error(
                    AppErrorCode::UnsafeManagedPath,
                    "Yazma hedefi klasör olamaz.",
                    managed.as_str(),
                ));
            }
        }
        let canonical_parent = fs::canonicalize(parent).map_err(|error| {
            unsafe_path_io_error(
                AppErrorCode::ManagedPathOutsideProject,
                "Yazma klasörü güvenli biçimde doğrulanamadı.",
                managed.as_str(),
                error,
            )
        })?;
        ensure_contained(
            &self.root,
            &canonical_parent,
            AppErrorCode::ManagedPathOutsideProject,
        )?;
        reject_symlink_components(&self.root, parent)?;
        Ok(target)
    }

    pub fn ensure_managed_directory(&self, directory: &Path) -> Result<(), AppError> {
        ensure_contained(
            &self.root,
            directory,
            AppErrorCode::ManagedPathOutsideProject,
        )?;
        let relative = directory.strip_prefix(&self.root).map_err(|_| {
            unsafe_path_error(
                AppErrorCode::ManagedPathOutsideProject,
                "Yazma klasörü proje kökü içinde değil.",
                &directory.to_string_lossy(),
            )
        })?;
        let mut current = self.root.clone();
        for component in relative.components() {
            let Component::Normal(component) = component else {
                return Err(unsafe_path_error(
                    AppErrorCode::UnsafeManagedPath,
                    "Yazma klasörü geçersiz bir bileşen içeriyor.",
                    &directory.to_string_lossy(),
                ));
            };
            current.push(component);
            match fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(unsafe_path_error(
                        AppErrorCode::ManagedPathSymlinkEscape,
                        "Yazma klasörü symlink olamaz.",
                        &current.to_string_lossy(),
                    ));
                }
                Ok(metadata) if metadata.is_dir() => {}
                Ok(_) => {
                    return Err(unsafe_path_error(
                        AppErrorCode::ManagedPathOutsideProject,
                        "Yazma yolu bir dosyanın içinden geçiyor.",
                        &current.to_string_lossy(),
                    ));
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    fs::create_dir(&current).map_err(|create_error| {
                        project_io_error(
                            AppErrorCode::FileWriteFailed,
                            "Proje içi yazma klasörü oluşturulamadı.",
                            &current,
                            create_error,
                        )
                    })?;
                }
                Err(error) => {
                    return Err(project_io_error(
                        AppErrorCode::FileWriteFailed,
                        "Proje içi yazma klasörü doğrulanamadı.",
                        &current,
                        error,
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn atomic_write(
        &self,
        managed: &ManagedProjectPath,
        content: &str,
    ) -> Result<(), AppError> {
        let target = self.prepare_write_target(managed)?;
        let temp = target.with_extension("tmp");
        self.ensure_write_candidate(&temp)?;
        atomic_write(&target, content).map_err(|error| {
            project_io_error(
                AppErrorCode::ProjectSaveFailed,
                "Proje dosyası güvenli biçimde yazılamadı.",
                &target,
                error,
            )
        })?;
        self.ensure_write_candidate(&target)?;
        Ok(())
    }

    pub fn atomic_write_bytes(
        &self,
        managed: &ManagedProjectPath,
        content: &[u8],
    ) -> Result<(), AppError> {
        let target = self.prepare_write_target(managed)?;
        write_bytes(&target, content).map_err(|error| {
            unsafe_path_io_error(
                AppErrorCode::ProjectSaveFailed,
                "Proje içi dosya güvenli biçimde yazılamadı.",
                managed.as_str(),
                error,
            )
        })
    }

    pub fn create_new_file(
        &self,
        managed: &ManagedProjectPath,
        content: &str,
    ) -> Result<(), AppError> {
        let target = self.prepare_write_target(managed)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|error| {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    project_io_error(
                        AppErrorCode::ProjectAlreadyExists,
                        "Bu klasörde zaten bir Rubrika projesi bulunuyor.",
                        &target,
                        error,
                    )
                } else {
                    project_io_error(
                        AppErrorCode::ProjectSaveFailed,
                        "Yeni proje dosyası oluşturulamadı.",
                        &target,
                        error,
                    )
                }
            })?;
        if let Err(error) = file
            .write_all(content.as_bytes())
            .and_then(|_| file.sync_all())
        {
            drop(file);
            let _ = fs::remove_file(&target);
            return Err(project_io_error(
                AppErrorCode::ProjectSaveFailed,
                "Yeni proje dosyası tamamlanamadı.",
                &target,
                error,
            ));
        }
        Ok(())
    }

    fn ensure_write_candidate(&self, candidate: &Path) -> Result<(), AppError> {
        let parent = candidate.parent().ok_or_else(|| {
            unsafe_path_error(
                AppErrorCode::ManagedPathOutsideProject,
                "Geçici yazma dosyasının parent klasörü belirlenemedi.",
                &candidate.to_string_lossy(),
            )
        })?;
        let canonical_parent = fs::canonicalize(parent).map_err(|error| {
            unsafe_path_io_error(
                AppErrorCode::ManagedPathOutsideProject,
                "Geçici yazma klasörü doğrulanamadı.",
                &candidate.to_string_lossy(),
                error,
            )
        })?;
        ensure_contained(
            &self.root,
            &canonical_parent,
            AppErrorCode::ManagedPathOutsideProject,
        )?;
        reject_symlink_components(&self.root, parent)?;
        Ok(())
    }
}

fn looks_like_absolute_path(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    raw.starts_with('/')
        || raw.starts_with("\\\\")
        || (bytes.len() >= 2 && bytes[1] == b':')
        || Path::new(raw).is_absolute()
}

pub fn is_absolute_like_path(raw: &str) -> bool {
    looks_like_absolute_path(raw)
}

fn canonicalize_absolute(path: &Path, code: AppErrorCode) -> Result<PathBuf, AppError> {
    let absolute = absolute_path(path)?;
    fs::canonicalize(&absolute)
        .map_err(|error| project_io_error(code, "Proje yolu okunamadı.", &absolute, error))
}

fn absolute_path(path: &Path) -> Result<PathBuf, AppError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    std::env::current_dir()
        .map(|current| current.join(path))
        .map_err(|error| {
            project_io_error(
                AppErrorCode::ProjectLoadFailed,
                "Geçerli çalışma klasörü belirlenemedi.",
                path,
                error,
            )
        })
}

fn canonicalize_with_missing_tail(path: &Path) -> Result<PathBuf, AppError> {
    let absolute = absolute_path(path)?;
    let mut missing = Vec::new();
    let mut cursor = absolute.clone();
    while !cursor.exists() {
        let component = cursor.file_name().ok_or_else(|| {
            project_path_error(
                AppErrorCode::ProjectSaveFailed,
                "Yeni proje yolu belirlenemedi.",
                cursor.clone(),
            )
        })?;
        missing.push(component.to_os_string());
        cursor.pop();
    }
    let mut result = fs::canonicalize(&cursor).map_err(|error| {
        project_io_error(
            AppErrorCode::ProjectSaveFailed,
            "Yeni proje parent klasörü doğrulanamadı.",
            &cursor,
            error,
        )
    })?;
    for component in missing.iter().rev() {
        result.push(component);
    }
    Ok(result)
}

fn reject_symlink_components(root: &Path, candidate: &Path) -> Result<(), AppError> {
    let relative = candidate.strip_prefix(root).map_err(|_| {
        unsafe_path_error(
            AppErrorCode::ManagedPathOutsideProject,
            "Bu dosya proje klasörünün dışında olduğu için açılamadı.",
            &candidate.to_string_lossy(),
        )
    })?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            continue;
        };
        current.push(component);
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            if metadata.file_type().is_symlink() {
                return Err(unsafe_path_error(
                    AppErrorCode::ManagedPathSymlinkEscape,
                    "Bu dosya symlink üzerinden güvenli proje alanının dışına çıkıyor.",
                    &current.to_string_lossy(),
                ));
            }
        }
    }
    Ok(())
}

fn ensure_contained(root: &Path, candidate: &Path, code: AppErrorCode) -> Result<(), AppError> {
    if candidate == root || candidate.strip_prefix(root).is_ok() {
        Ok(())
    } else {
        Err(unsafe_path_error(
            code,
            "Bu dosya proje klasörünün dışında olduğu için açılamadı.",
            &candidate.to_string_lossy(),
        ))
    }
}

fn unsafe_path_error(code: AppErrorCode, message: &str, details: &str) -> AppError {
    AppError {
        code,
        message: message.to_string(),
        recoverable: true,
        suggested_action: Some(
            "Proje içindeki güvenli bir dosyayı seçip tekrar deneyin.".to_string(),
        ),
        technical_details: Some(details.to_string()),
        correlation_id: uuid::Uuid::new_v4().to_string(),
    }
}

fn unsafe_path_io_error(
    code: AppErrorCode,
    message: &str,
    details: &str,
    error: io::Error,
) -> AppError {
    let mut result = unsafe_path_error(code, message, details);
    result.technical_details = Some(format!(
        "{}; error={error}",
        result.technical_details.unwrap_or_default()
    ));
    result
}

fn project_path_error(code: AppErrorCode, message: &str, path: PathBuf) -> AppError {
    unsafe_path_error(code, message, &path.to_string_lossy())
}

fn project_io_error(code: AppErrorCode, message: &str, path: &Path, error: io::Error) -> AppError {
    let mut result = unsafe_path_error(code, message, &path.to_string_lossy());
    result.technical_details = Some(format!(
        "{}; error={error}",
        result.technical_details.unwrap_or_default()
    ));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn temp_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("rubrika-paths-{name}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).expect("temp root");
        root
    }

    #[test]
    fn managed_paths_reject_absolute_parent_and_empty_values() {
        for value in [
            "/tmp/x",
            "C:\\Users\\x",
            "C:/Users/x",
            "../x",
            "documents/../../x",
            ".",
            "",
        ] {
            assert!(ManagedProjectPath::parse(value).is_err(), "{value}");
        }
        assert_eq!(
            ManagedProjectPath::parse("documents/exam.pdf")
                .unwrap()
                .as_str(),
            "documents/exam.pdf"
        );
    }

    #[test]
    fn read_rejects_symlink_escape() {
        let root = temp_root("read-symlink");
        let outside = temp_root("read-outside");
        fs::write(outside.join("secret.pdf"), b"secret").expect("outside file");
        fs::create_dir_all(root.join("documents")).expect("documents");
        symlink(outside.join("secret.pdf"), root.join("documents/link.pdf")).expect("symlink");
        let trusted = TrustedProjectRoot::from_canonical_root(root.clone(), false).unwrap();
        let path = trusted.managed("documents/link.pdf").unwrap();
        assert_eq!(
            trusted.resolve_existing_file(&path).unwrap_err().code,
            AppErrorCode::ManagedPathSymlinkEscape
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }

    #[test]
    fn write_rejects_symlink_parent_even_when_target_is_missing() {
        let root = temp_root("write-symlink");
        let outside = temp_root("write-outside");
        symlink(&outside, root.join("outputs")).expect("symlink");
        let trusted = TrustedProjectRoot::from_canonical_root(root.clone(), false).unwrap();
        let path = trusted.managed("outputs/result.json").unwrap();
        assert_eq!(
            trusted.prepare_write_target(&path).unwrap_err().code,
            AppErrorCode::ManagedPathSymlinkEscape
        );
        assert!(!outside.join("result.json").exists());
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }

    #[test]
    fn create_new_file_does_not_overwrite_existing_project_file() {
        let root = temp_root("create-new");
        fs::write(root.join(PROJECT_FILE_NAME), b"original").expect("project file");
        let trusted = TrustedProjectRoot::from_canonical_root(root.clone(), false).unwrap();
        let path = trusted.managed(PROJECT_FILE_NAME).unwrap();
        assert_eq!(
            trusted
                .create_new_file(&path, "replacement")
                .unwrap_err()
                .code,
            AppErrorCode::ProjectAlreadyExists
        );
        assert_eq!(fs::read(root.join(PROJECT_FILE_NAME)).unwrap(), b"original");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_absolute_path_inside_root_becomes_relative_and_outside_stays_unresolved() {
        let root = temp_root("legacy-adapter");
        let outside = temp_root("legacy-outside");
        fs::create_dir_all(root.join("documents")).unwrap();
        let inside = root.join("documents").join("exam.pdf");
        fs::write(&inside, b"exam").unwrap();
        fs::write(outside.join("secret.pdf"), b"secret").unwrap();
        let trusted = TrustedProjectRoot::from_canonical_root(root.clone(), false).unwrap();

        let adapted = trusted
            .adapt_legacy_document_path(&inside.to_string_lossy())
            .unwrap();
        assert_eq!(adapted.as_str(), "documents/exam.pdf");
        assert_eq!(
            trusted
                .adapt_legacy_document_path(&outside.join("secret.pdf").to_string_lossy())
                .unwrap_err()
                .code,
            AppErrorCode::ManagedPathOutsideProject
        );

        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }

    #[test]
    fn atomic_temp_symlink_cannot_redirect_bytes_outside_root() {
        let root = temp_root("atomic-temp");
        let outside = temp_root("atomic-outside");
        let outside_target = outside.join("secret.bin");
        fs::write(&outside_target, b"unchanged").unwrap();
        let trusted = TrustedProjectRoot::from_canonical_root(root.clone(), false).unwrap();
        let target = trusted.managed("cache/output.bin").unwrap();
        let target_path = root.join(target.as_path());
        fs::create_dir_all(target_path.parent().unwrap()).unwrap();
        symlink(&outside_target, target_path.with_extension("tmp")).unwrap();

        assert!(trusted.atomic_write_bytes(&target, b"replacement").is_err());
        assert_eq!(fs::read(outside_target).unwrap(), b"unchanged");

        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }

    #[test]
    fn tauri_asset_scope_is_limited_to_managed_project_area() {
        let config = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tauri.conf.json"));
        assert!(config.contains("$HOME/Documents/RubrikaV3/Projects/**/*"));
        assert!(!config.contains("$HOME/Desktop/RubriKa/**/*"));
        assert!(!config.contains("$HOME/**/**/*"));
    }
}

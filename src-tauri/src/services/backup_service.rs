use std::collections::HashSet;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::project::Project;
use crate::platform::file_access;
use crate::platform::project_paths::TrustedProjectRoot;
use crate::platform::project_write_lease::{acquire_or_share, acquire_or_share_existing};
use crate::services::integrity_recovery_service::build_source_manifest;
use crate::services::project_store::ProjectStore;
use tokio_util::sync::CancellationToken;

pub const BACKUP_FORMAT_VERSION: u32 = 1;
pub const PROJECT_SCHEMA_VERSION: u32 = 1;

const MAGIC: &[u8; 18] = b"RUBRIKA_BACKUP_V1\0";
const MAX_MANIFEST_BYTES: u64 = 64 * 1024 * 1024;
const BACKUP_READER_BOUND: u64 = MAX_BACKUP_TOTAL_BYTES + MAX_MANIFEST_BYTES + 1024;

/// Hard safety limits for backup/restore (archive bomb protection).
pub const MAX_BACKUP_ENTRIES: u64 = 100_000;
pub const MAX_BACKUP_TOTAL_BYTES: u64 = 8 * 1024 * 1024 * 1024;
pub const MAX_BACKUP_ENTRY_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifestEntry {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub format_version: u32,
    pub app_version: String,
    pub created_at: String,
    pub project_id: String,
    pub project_schema_version: u32,
    pub included_entries: Vec<BackupManifestEntry>,
    pub total_uncompressed_size: u64,
    #[serde(default)]
    pub source_manifest_sha256: Option<String>,
    #[serde(default)]
    pub source_file_count: Option<u64>,
    #[serde(default)]
    pub source_byte_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub archive_path: String,
    pub verification_path: String,
    pub manifest_path: String,
    pub source_project_path: String,
    pub entry_count: u64,
    pub total_size: u64,
    pub sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreSummary {
    pub destination: String,
    pub entry_count: u64,
    pub restored_project_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupVerificationReceipt {
    pub archive_path: String,
    pub archive_sha256: String,
    pub source_project_path: String,
    pub project_id: String,
    pub entry_count: u64,
    pub created_at: String,
    #[serde(default)]
    pub source_manifest_sha256: Option<String>,
    #[serde(default)]
    pub source_file_count: Option<u64>,
    #[serde(default)]
    pub source_byte_count: Option<u64>,
}

/// Validates a relative archive entry path.
///
/// Rejects absolute paths, parent traversal, backslashes, NUL bytes,
/// Windows drive prefixes and empty components. The same validator is used
/// for both archive creation and restore so `../`, encoded traversal and
/// drive-letter escapes can never reach the filesystem.
pub fn validate_relative_entry(raw: &str) -> Result<PathBuf, AppError> {
    if raw.trim().is_empty() || raw.contains('\0') || raw.contains('\\') {
        return Err(archive_invalid(&format!("unsafe entry name: {raw:?}")));
    }
    let bytes = raw.as_bytes();
    if raw.starts_with('/')
        || (bytes.len() >= 2 && bytes[1] == b':')
        || Path::new(raw).is_absolute()
    {
        return Err(archive_invalid(&format!("absolute entry name: {raw:?}")));
    }
    let mut relative = PathBuf::new();
    for component in Path::new(raw).components() {
        match component {
            Component::Normal(value) => {
                let value = value
                    .to_str()
                    .ok_or_else(|| archive_invalid("entry name is not valid UTF-8"))?;
                if value.is_empty() || value.contains('\0') {
                    return Err(archive_invalid("entry contains an invalid component"));
                }
                relative.push(value);
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(archive_invalid(&format!("traversal entry name: {raw:?}")));
            }
        }
    }
    if relative.as_os_str().is_empty() {
        return Err(archive_invalid("empty entry name"));
    }
    Ok(relative)
}

fn archive_invalid(detail: &str) -> AppError {
    AppError {
        code: AppErrorCode::BackupArchiveInvalid,
        message: "Yedek arşivi geçersiz.".to_string(),
        recoverable: false,
        suggested_action: Some("Arşivi kaynak projeden yeniden oluşturun.".to_string()),
        technical_details: Some(detail.to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    }
}

fn backup_error(detail: &str) -> AppError {
    AppError {
        code: AppErrorCode::BackupFailed,
        message: "Yedek oluşturulamadı.".to_string(),
        recoverable: true,
        suggested_action: Some("Disk alanını kontrol edip yeniden deneyin.".to_string()),
        technical_details: Some(detail.to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    }
}

fn restore_error(detail: &str) -> AppError {
    AppError {
        code: AppErrorCode::RestoreFailed,
        message: "Yedek geri yüklenemedi.".to_string(),
        recoverable: true,
        suggested_action: Some("Arşivi ve hedef klasörü kontrol edip yeniden deneyin.".to_string()),
        technical_details: Some(detail.to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn sha256_file(path: &Path) -> Result<String, AppError> {
    let mut hasher = Sha256::new();
    let mut file = std::fs::File::open(path)
        .map_err(|error| backup_error(&format!("open {}: {error}", path.display())))?;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| backup_error(&format!("read {}: {error}", path.display())))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn is_cancelled(token: &CancellationToken) -> bool {
    token.is_cancelled()
}

/// Creates a bounded, verified backup archive next to the project.
pub fn create_backup(
    project_root: &Path,
    token: &CancellationToken,
) -> Result<BackupSummary, AppError> {
    create_verified_backup(project_root, None, token)
}

/// Creates an independently stored, checksum-verified backup. The source
/// project is opened under an existing OS lease; this function never creates
/// a lock, backup directory, audit record, or metadata file inside the source.
pub fn create_verified_backup(
    project_root: &Path,
    destination_root: Option<&Path>,
    token: &CancellationToken,
) -> Result<BackupSummary, AppError> {
    let trusted_root = TrustedProjectRoot::from_canonical_root(
        project_root
            .canonicalize()
            .map_err(|error| backup_error(&format!("canonicalize: {error}")))?,
        true,
    )?;
    let _write_lease = acquire_or_share_existing(trusted_root.root())?;
    let root = trusted_root.root();
    let project = load_project_json(root)?;
    let project_id = project.id.clone();

    let source_manifest = build_source_manifest(root)?;
    if source_manifest.summary.symlink_count > 0 || source_manifest.summary.other_count > 0 {
        return Err(backup_error(
            "source contains symlink or unsupported filesystem entry",
        ));
    }
    let entries = source_manifest
        .entries
        .iter()
        .filter(|entry| entry.file_type == "regular")
        .map(|entry| {
            let path = root.join(&entry.relative_path);
            (path, entry.size, entry.sha256.clone().unwrap_or_default())
        })
        .collect::<Vec<_>>();
    if entries.len() as u64 > MAX_BACKUP_ENTRIES
        || source_manifest.summary.total_regular_bytes > MAX_BACKUP_TOTAL_BYTES
    {
        return Err(backup_error("backup size or entry count limit exceeded"));
    }

    let total_bytes = source_manifest.summary.total_regular_bytes;
    let manifest = BackupManifest {
        format_version: BACKUP_FORMAT_VERSION,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        project_id: project_id.clone(),
        project_schema_version: PROJECT_SCHEMA_VERSION,
        included_entries: entries
            .iter()
            .map(|(path, size, hash)| BackupManifestEntry {
                path: path
                    .strip_prefix(root)
                    .map(|relative| relative.to_string_lossy().to_string())
                    .unwrap_or_default(),
                size: *size,
                sha256: hash.clone(),
            })
            .collect(),
        total_uncompressed_size: total_bytes,
        source_manifest_sha256: Some(source_manifest.manifest_sha256.clone()),
        source_file_count: Some(source_manifest.summary.file_count),
        source_byte_count: Some(source_manifest.summary.total_regular_bytes),
    };

    let manifest_json = serde_json::to_vec(&manifest)
        .map_err(|error| backup_error(&format!("manifest: {error}")))?;
    let backup_dir = destination_root
        .map(PathBuf::from)
        .unwrap_or(verified_backup_directory(root)?);
    ensure_external_backup_directory(root, &backup_dir)?;
    std::fs::create_dir_all(&backup_dir)
        .map_err(|error| backup_error(&format!("backup dir: {error}")))?;
    ensure_external_backup_directory(root, &backup_dir)?;
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%.fZ");
    let project_name = root
        .file_name()
        .and_then(|value| value.to_str())
        .map(safe_backup_name)
        .unwrap_or_else(|| "project".to_string());
    let mut final_path =
        backup_dir.join(format!("{project_name}-pre-recovery-{timestamp}.rbackup"));
    let mut duplicate_index = 2u32;
    while final_path.exists() {
        final_path = backup_dir.join(format!(
            "{project_name}-pre-recovery-{timestamp}-{duplicate_index}.rbackup"
        ));
        duplicate_index = duplicate_index.saturating_add(1);
    }
    let staging_path = backup_dir.join(format!(".staging-{}.rbackup.tmp", Uuid::new_v4()));

    let write_result = (|| -> Result<(), AppError> {
        let mut file = std::fs::File::create(&staging_path)
            .map_err(|error| backup_error(&format!("create staging: {error}")))?;
        file.write_all(MAGIC)
            .map_err(|error| backup_error(&format!("write magic: {error}")))?;
        file.write_all(&(manifest_json.len() as u64).to_be_bytes())
            .map_err(|error| backup_error(&format!("write manifest len: {error}")))?;
        file.write_all(&manifest_json)
            .map_err(|error| backup_error(&format!("write manifest: {error}")))?;
        for (path, expected_size, expected_hash) in &entries {
            if is_cancelled(token) {
                return Err(AppError {
                    code: AppErrorCode::BackupCancelled,
                    message: "Yedek oluşturma iptal edildi.".to_string(),
                    recoverable: true,
                    suggested_action: None,
                    technical_details: None,
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
            let metadata = std::fs::symlink_metadata(path)
                .map_err(|error| backup_error(&format!("metadata {}: {error}", path.display())))?;
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || metadata.len() != *expected_size
            {
                return Err(backup_error(&format!(
                    "source changed while backing up: {}",
                    path.display()
                )));
            }
            let current_hash = sha256_file(path)?;
            if current_hash != *expected_hash {
                return Err(backup_error(&format!(
                    "source checksum changed while backing up: {}",
                    path.display()
                )));
            }
            let bytes = std::fs::read(path)
                .map_err(|error| backup_error(&format!("read {}: {error}", path.display())))?;
            if bytes.len() as u64 != *expected_size || sha256_hex(&bytes) != *expected_hash {
                return Err(backup_error(&format!(
                    "source changed during read: {}",
                    path.display()
                )));
            }
            file.write_all(&(bytes.len() as u64).to_be_bytes())
                .map_err(|error| backup_error(&format!("write entry len: {error}")))?;
            file.write_all(&bytes)
                .map_err(|error| backup_error(&format!("write entry: {error}")))?;
        }
        file.sync_all()
            .map_err(|error| backup_error(&format!("sync: {error}")))?;
        Ok(())
    })();

    if let Err(error) = write_result {
        let _ = std::fs::remove_file(&staging_path);
        return Err(error);
    }

    // A source can change after the per-entry read but before activation.
    // Recheck the complete manifest immediately before publishing the archive.
    if let Err(error) = verify_source_entries(&entries, &source_manifest) {
        let _ = std::fs::remove_file(&staging_path);
        return Err(error);
    }

    // Verify the archive before atomic activation.
    verify_archive(&staging_path).inspect_err(|_error| {
        let _ = std::fs::remove_file(&staging_path);
    })?;

    file_access::durable_rename(&staging_path, &final_path)
        .map_err(|error| backup_error(&format!("atomic rename: {error}")))?;
    let archive_sha = sha256_file(&final_path)?;
    let verification_path = final_path.with_extension("sha256.json");
    let manifest_path = final_path.with_extension("manifest.json");
    let receipt = BackupVerificationReceipt {
        archive_path: final_path.to_string_lossy().to_string(),
        archive_sha256: archive_sha.clone(),
        source_project_path: root.to_string_lossy().to_string(),
        project_id: project_id.clone(),
        entry_count: entries.len() as u64,
        created_at: manifest.created_at.clone(),
        source_manifest_sha256: manifest.source_manifest_sha256.clone(),
        source_file_count: manifest.source_file_count,
        source_byte_count: manifest.source_byte_count,
    };
    let receipt_content = serde_json::to_string_pretty(&receipt)
        .map_err(|error| backup_error(&format!("verification receipt: {error}")))?;
    file_access::atomic_write(&verification_path, &receipt_content)
        .map_err(|error| backup_error(&format!("verification receipt write: {error}")))?;
    file_access::atomic_write(&manifest_path, &receipt_content)
        .map_err(|error| backup_error(&format!("backup manifest write: {error}")))?;
    verify_archive(&final_path)?;

    Ok(BackupSummary {
        archive_path: final_path.to_string_lossy().to_string(),
        verification_path: verification_path.to_string_lossy().to_string(),
        manifest_path: manifest_path.to_string_lossy().to_string(),
        source_project_path: root.to_string_lossy().to_string(),
        entry_count: entries.len() as u64,
        total_size: total_bytes,
        sha256: archive_sha,
        created_at: manifest.created_at,
    })
}

/// Resolves the default independent backup location without creating it.
pub fn verified_backup_directory(project_root: &Path) -> Result<PathBuf, AppError> {
    let root = project_root
        .canonicalize()
        .map_err(|error| backup_error(&format!("canonicalize backup root: {error}")))?;
    let parent = root
        .parent()
        .ok_or_else(|| backup_error("project has no parent directory"))?;
    let base = if parent.file_name().and_then(|value| value.to_str()) == Some("Projects") {
        parent
            .parent()
            .ok_or_else(|| backup_error("Projects directory has no parent"))?
    } else {
        parent
    };
    Ok(base.join("VerifiedBackups"))
}

fn ensure_external_backup_directory(source: &Path, destination: &Path) -> Result<(), AppError> {
    let source = source
        .canonicalize()
        .map_err(|error| backup_error(&format!("source canonicalize: {error}")))?;
    let absolute_destination = if destination.is_absolute() {
        destination.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| backup_error(&format!("backup destination cwd: {error}")))?
            .join(destination)
    };
    let existing_parent = absolute_destination
        .parent()
        .ok_or_else(|| backup_error("backup destination has no parent"))?
        .canonicalize()
        .map_err(|error| backup_error(&format!("backup destination parent: {error}")))?;
    let prospective_destination = existing_parent.join(
        absolute_destination
            .file_name()
            .ok_or_else(|| backup_error("backup destination has no name"))?,
    );
    if prospective_destination == source || prospective_destination.starts_with(&source) {
        return Err(backup_error(
            "verified backup destination must be outside source project",
        ));
    }
    if let Ok(metadata) = std::fs::symlink_metadata(&absolute_destination) {
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(backup_error(
                "verified backup destination must be a regular directory",
            ));
        }
        let canonical_destination = absolute_destination
            .canonicalize()
            .map_err(|error| backup_error(&format!("backup destination canonicalize: {error}")))?;
        if canonical_destination == source || canonical_destination.starts_with(&source) {
            return Err(backup_error(
                "verified backup destination resolves inside source project",
            ));
        }
    }
    Ok(())
}

fn safe_backup_name(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "project".to_string()
    } else {
        sanitized
    }
}

fn verify_source_entries(
    entries: &[(PathBuf, u64, String)],
    initial_manifest: &crate::services::integrity_recovery_service::SourceManifest,
) -> Result<(), AppError> {
    for (path, expected_size, expected_hash) in entries {
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|error| backup_error(&format!("metadata {}: {error}", path.display())))?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() != *expected_size
            || sha256_file(path)? != *expected_hash
        {
            return Err(backup_error(&format!(
                "source changed before backup activation: {}",
                path.display()
            )));
        }
    }
    let current_manifest = build_source_manifest(Path::new(&initial_manifest.root))
        .map_err(|error| backup_error(&format!("source manifest recheck: {error}")))?;
    if current_manifest.byte_manifest_sha256() != initial_manifest.byte_manifest_sha256() {
        return Err(backup_error(
            "source byte manifest changed before backup activation",
        ));
    }
    Ok(())
}

/// Parses and validates the archive header, manifest and entry bounds.
///
/// The archive is streamed from disk with a hard file-size bound; it is
/// never read wholesale into memory.
pub fn parse_archive(archive_path: &Path) -> Result<(BackupManifest, Vec<u64>, u64), AppError> {
    let mut file = open_bounded_archive(archive_path)?;
    let mut magic = [0u8; MAGIC.len()];
    file.read_exact(&mut magic)
        .map_err(|error| restore_error(&format!("read magic: {error}")))?;
    if &magic != MAGIC {
        return Err(archive_invalid("bad magic header"));
    }
    let mut manifest_len_bytes = [0u8; 8];
    file.read_exact(&mut manifest_len_bytes)
        .map_err(|error| restore_error(&format!("read manifest length: {error}")))?;
    let manifest_len = u64::from_be_bytes(manifest_len_bytes);
    if manifest_len == 0 || manifest_len > MAX_MANIFEST_BYTES {
        return Err(archive_invalid("manifest length out of bounds"));
    }
    let mut manifest_bytes = vec![0u8; manifest_len as usize];
    file.read_exact(&mut manifest_bytes)
        .map_err(|error| restore_error(&format!("read manifest: {error}")))?;
    let manifest: BackupManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| archive_invalid(&format!("manifest parse: {error}")))?;
    let data_start = (MAGIC.len() as u64) + 8 + manifest_len;

    if manifest.format_version != BACKUP_FORMAT_VERSION {
        return Err(archive_invalid(&format!(
            "unsupported format_version {}",
            manifest.format_version
        )));
    }
    if manifest.included_entries.len() as u64 > MAX_BACKUP_ENTRIES {
        return Err(archive_invalid("entry count limit exceeded"));
    }
    if manifest.total_uncompressed_size > MAX_BACKUP_TOTAL_BYTES {
        return Err(archive_invalid("total uncompressed size limit exceeded"));
    }
    let mut seen = HashSet::new();
    let mut offsets = Vec::with_capacity(manifest.included_entries.len());
    let mut offset = data_start;
    for entry in &manifest.included_entries {
        let normalized = validate_relative_entry(&entry.path)?;
        let normalized_str = normalized.to_string_lossy().to_string();
        if !seen.insert(normalized_str) {
            return Err(archive_invalid(&format!(
                "duplicate normalized path: {}",
                entry.path
            )));
        }
        if entry.size > MAX_BACKUP_ENTRY_BYTES {
            return Err(archive_invalid(&format!(
                "entry exceeds size limit: {}",
                entry.path
            )));
        }
        if entry.sha256.len() != 64 {
            return Err(archive_invalid("entry hash is not sha256 hex"));
        }
        offsets.push(offset);
        offset = offset
            .checked_add(8)
            .and_then(|next| next.checked_add(entry.size))
            .ok_or_else(|| archive_invalid("entry offset overflow"))?;
    }
    let file_len = file
        .metadata()
        .map_err(|error| restore_error(&format!("archive metadata: {error}")))?
        .len();
    if offset != file_len {
        return Err(archive_invalid("trailing or missing archive bytes"));
    }
    Ok((manifest, offsets, data_start))
}

fn open_bounded_archive(archive_path: &Path) -> Result<std::fs::File, AppError> {
    let file = std::fs::File::open(archive_path)
        .map_err(|error| restore_error(&format!("open archive: {error}")))?;
    let len = file
        .metadata()
        .map_err(|error| restore_error(&format!("archive metadata: {error}")))?
        .len();
    if len == 0 || len > BACKUP_READER_BOUND {
        return Err(archive_invalid("archive size out of bounds"));
    }
    Ok(file)
}

/// Verifies the archive by re-reading every entry and checking hashes.
pub fn verify_archive(archive_path: &Path) -> Result<(), AppError> {
    let (manifest, offsets, _) = parse_archive(archive_path)?;
    let mut file = open_bounded_archive(archive_path)?;
    for (entry, declared_offset) in manifest.included_entries.iter().zip(offsets.iter()) {
        file.seek(SeekFrom::Start(*declared_offset))
            .map_err(|error| restore_error(&format!("seek: {error}")))?;
        let mut size_bytes = [0u8; 8];
        file.read_exact(&mut size_bytes)
            .map_err(|error| restore_error(&format!("read entry size: {error}")))?;
        let stored_size = u64::from_be_bytes(size_bytes);
        if stored_size != entry.size {
            return Err(archive_invalid(&format!(
                "declared size mismatch for {}",
                entry.path
            )));
        }
        let hash = hash_stream(&mut file, entry.size)
            .map_err(|error| restore_error(&format!("hash {}: {error}", entry.path)))?;
        if hash != entry.sha256 {
            return Err(archive_invalid(&format!(
                "checksum mismatch for {}",
                entry.path
            )));
        }
    }
    Ok(())
}

fn hash_stream(reader: &mut impl Read, byte_count: u64) -> Result<String, std::io::Error> {
    let mut hasher = Sha256::new();
    let mut remaining = byte_count;
    let mut buffer = [0u8; 64 * 1024];
    while remaining > 0 {
        let chunk = (remaining.min(buffer.len() as u64)) as usize;
        let read = reader.read(&mut buffer[..chunk])?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "unexpected end of entry data",
            ));
        }
        hasher.update(&buffer[..read]);
        remaining -= read as u64;
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Restores an archive into a brand-new destination.
///
/// The archive is fully verified (bounds, traversal, duplicates, checksums,
/// schema) before the destination is touched. Extraction goes to a staging
/// directory and the destination is activated atomically only after semantic
/// project validation succeeds.
pub fn restore_backup(
    archive_path: &Path,
    destination_root: &Path,
    token: &CancellationToken,
) -> Result<RestoreSummary, AppError> {
    let destination_name = destination_root
        .file_name()
        .ok_or_else(|| restore_error("destination has no final component"))?;
    let destination_parent = destination_root
        .parent()
        .ok_or_else(|| restore_error("destination has no parent"))?
        .canonicalize()
        .map_err(|error| restore_error(&format!("destination parent: {error}")))?;
    let destination_root = destination_parent.join(destination_name);
    if std::fs::symlink_metadata(&destination_root).is_ok() {
        return Err(AppError {
            code: AppErrorCode::RestoreDestinationConflict,
            message: "Restore mevcut veya symlink olan bir hedefi değiştirmeyi reddetti."
                .to_string(),
            recoverable: true,
            suggested_action: Some("Var olmayan yeni bir hedef klasör seçin.".to_string()),
            technical_details: Some(destination_root.display().to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        });
    }

    let (manifest, offsets, data_start) = parse_archive(archive_path)?;
    let mut file = open_bounded_archive(archive_path)?;
    let mut entry_index = 0usize;
    let mut entry_offset = data_start;

    let staging_parent = destination_root
        .parent()
        .ok_or_else(|| restore_error("destination has no parent"))?;
    let staging = staging_parent.join(format!(".rubrika-restore-staging-{}", Uuid::new_v4()));
    let cleanup_staging = || {
        if let Ok(metadata) = std::fs::symlink_metadata(&staging) {
            if metadata.is_dir() && !metadata.file_type().is_symlink() {
                let _ = std::fs::remove_dir_all(&staging);
            }
        }
    };

    let mut project_json_bytes: Option<Vec<u8>> = None;
    let extract_result = (|| -> Result<(), AppError> {
        std::fs::create_dir_all(&staging)
            .map_err(|error| restore_error(&format!("staging: {error}")))?;
        for entry in &manifest.included_entries {
            if is_cancelled(token) {
                return Err(AppError {
                    code: AppErrorCode::RestoreCancelled,
                    message: "Restore iptal edildi.".to_string(),
                    recoverable: true,
                    suggested_action: None,
                    technical_details: None,
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
            let declared_offset = offsets[entry_index];
            if declared_offset != entry_offset {
                return Err(archive_invalid("entry offset mismatch"));
            }
            file.seek(SeekFrom::Start(entry_offset))
                .map_err(|error| restore_error(&format!("seek: {error}")))?;
            let mut size_bytes = [0u8; 8];
            file.read_exact(&mut size_bytes)
                .map_err(|error| restore_error(&format!("read entry size: {error}")))?;
            let stored_size = u64::from_be_bytes(size_bytes);
            if stored_size != entry.size {
                return Err(archive_invalid(&format!(
                    "declared size mismatch for {}",
                    entry.path
                )));
            }
            // Hash and stage concurrently; staging is discarded on failure.
            let relative = validate_relative_entry(&entry.path)?;
            let target = staging.join(&relative);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| restore_error(&format!("create dir: {error}")))?;
            }
            let mut target_file = std::fs::File::create(&target)
                .map_err(|error| restore_error(&format!("create target: {error}")))?;
            let hash = copy_hashed(&mut file, &mut target_file, entry.size)
                .map_err(|error| restore_error(&format!("extract {}: {error}", entry.path)))?;
            target_file
                .sync_all()
                .map_err(|error| restore_error(&format!("sync {}: {error}", entry.path)))?;
            if hash != entry.sha256 {
                return Err(archive_invalid(&format!(
                    "checksum mismatch for {}",
                    entry.path
                )));
            }
            if entry.path == "project.json" {
                let mut data = Vec::new();
                std::fs::File::open(staging.join("project.json"))
                    .and_then(|mut file| file.read_to_end(&mut data))
                    .map_err(|error| {
                        restore_error(&format!("read staged project.json: {error}"))
                    })?;
                project_json_bytes = Some(data);
            }
            entry_offset += 8 + entry.size;
            entry_index += 1;
        }
        Ok(())
    })();
    if let Err(error) = extract_result {
        cleanup_staging();
        return Err(error);
    }

    // Semantic validation without rewriting project.json.  The bytes in the
    // verified archive are the evidence we are restoring; ProjectStore
    // normalizes the runtime root in memory when the copy is opened.
    let project_json_bytes = project_json_bytes.ok_or_else(|| {
        cleanup_staging();
        archive_invalid("archive has no project.json")
    })?;
    let project: Project = serde_json::from_slice(&project_json_bytes).map_err(|error| {
        cleanup_staging();
        archive_invalid(&format!("project.json parse: {error}"))
    })?;
    if manifest.project_id != project.id {
        cleanup_staging();
        return Err(archive_invalid("manifest project id mismatch"));
    }
    ProjectStore::open_project_at_path(&staging)
        .map_err(|error| restore_error(&format!("staged project validation: {}", error.message)))?;
    let _staging_lease = acquire_or_share(&staging)?;
    if let Ok(staging_dir) = std::fs::File::open(&staging) {
        let _ = staging_dir.sync_all();
    }

    // Atomic activation: the destination must not appear until everything is
    // verified, and an existing target is never removed as part of restore.
    if std::fs::symlink_metadata(&destination_root).is_ok() {
        cleanup_staging();
        return Err(AppError {
            code: AppErrorCode::RestoreDestinationConflict,
            message: "Restore hedefi aktivasyon sırasında zaten oluştu; mevcut veri korunudu."
                .to_string(),
            recoverable: true,
            suggested_action: Some("Başka bir boş hedef klasör seçin.".to_string()),
            technical_details: Some(destination_root.display().to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        });
    }
    if let Err(error) = file_access::durable_rename_directory(&staging, &destination_root) {
        cleanup_staging();
        return Err(restore_error(&format!("atomic activation: {error}")));
    }

    // The restored project was already validated read-only before activation;
    // opening it here must not run migration or recovery writes.
    let opened = ProjectStore::open_project_at_path(&destination_root)?;

    Ok(RestoreSummary {
        destination: destination_root.to_string_lossy().to_string(),
        entry_count: manifest.included_entries.len() as u64,
        restored_project_id: opened.id,
    })
}

fn load_project_json(root: &Path) -> Result<Project, AppError> {
    let path = root.join("project.json");
    let content = std::fs::read_to_string(&path)
        .map_err(|error| backup_error(&format!("read {}: {error}", path.display())))?;
    serde_json::from_str(&content)
        .map_err(|error| backup_error(&format!("project.json parse: {error}")))
}

fn copy_hashed(
    reader: &mut impl Read,
    writer: &mut impl Write,
    byte_count: u64,
) -> Result<String, std::io::Error> {
    let mut hasher = Sha256::new();
    let mut remaining = byte_count;
    let mut buffer = [0u8; 64 * 1024];
    while remaining > 0 {
        let chunk = (remaining.min(buffer.len() as u64)) as usize;
        let read = reader.read(&mut buffer[..chunk])?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "unexpected end of entry data",
            ));
        }
        hasher.update(&buffer[..read]);
        writer.write_all(&buffer[..read])?;
        remaining -= read as u64;
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn make_project(root: &Path) -> Project {
        let store = crate::services::project_store::ProjectStore::new();
        let project = store
            .create_project_with_setup(
                "Test Projesi".to_string(),
                root.to_string_lossy().to_string(),
                None,
                None,
                None,
            )
            .expect("create project");
        project
    }

    fn token() -> CancellationToken {
        CancellationToken::new()
    }

    fn project_bytes(root: &Path) -> BTreeMap<String, Vec<u8>> {
        fn visit(root: &Path, current: &Path, output: &mut BTreeMap<String, Vec<u8>>) {
            let Ok(entries) = std::fs::read_dir(current) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if name == ".rubrika.lock" || name == "cache" || name == "backups" {
                    continue;
                }
                if path.is_dir() {
                    visit(root, &path, output);
                } else if path.is_file() {
                    let relative = path
                        .strip_prefix(root)
                        .expect("relative project path")
                        .to_string_lossy()
                        .to_string();
                    output.insert(relative, std::fs::read(path).expect("project bytes"));
                }
            }
        }
        let mut output = BTreeMap::new();
        visit(root, root, &mut output);
        output
    }

    #[test]
    fn backup_restore_roundtrip_preserves_project() {
        let base =
            std::env::temp_dir().join(format!("rubrika-backup-roundtrip-{}", Uuid::new_v4()));
        let source = base.join("source");
        let destination = base.join("restored");
        std::fs::create_dir_all(&base).unwrap();
        let project = make_project(&source);
        let project_id = project.id.clone();
        let content_path = source.join("documents").join("sample.txt");
        std::fs::write(&content_path, b"STUDENT_CONTENT_OK").unwrap();

        let summary = create_backup(&source, &token()).expect("backup");
        assert!(summary.entry_count >= 2);
        assert!(std::path::Path::new(&summary.archive_path).exists());

        let restored = restore_backup(
            std::path::Path::new(&summary.archive_path),
            &destination,
            &token(),
        )
        .expect("restore");
        assert_eq!(restored.restored_project_id, project_id);
        assert_eq!(
            std::fs::read(destination.join("documents/sample.txt")).unwrap(),
            b"STUDENT_CONTENT_OK"
        );
        assert!(destination.join("project.json").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn proof_40_backup_restore_is_semantically_and_byte_equivalent() {
        let base = std::env::temp_dir().join(format!("rubrika-proof-40-{}", Uuid::new_v4()));
        let source = base.join("source");
        let backup_dir = base.join("verified-backup");
        let restored = base.join("restored");
        std::fs::create_dir_all(&base).expect("base");
        let project = make_project(&source);
        std::fs::write(source.join("documents/answer.pdf"), b"ANSWER-BYTES").expect("document");
        std::fs::write(source.join("outputs/result.json"), b"RESULT-BYTES").expect("output");
        let before = project_bytes(&source);
        let summary = create_verified_backup(&source, Some(&backup_dir), &token()).expect("backup");
        assert!(summary.verification_path.ends_with(".sha256.json"));
        assert_eq!(before, project_bytes(&source));
        restore_backup(Path::new(&summary.archive_path), &restored, &token()).expect("restore");
        for (relative, bytes) in before {
            if relative == "project.json" {
                let mut source_value: serde_json::Value =
                    serde_json::from_slice(&bytes).expect("source project json");
                let mut restored_value: serde_json::Value = serde_json::from_slice(
                    &std::fs::read(restored.join(&relative)).expect("restored project json"),
                )
                .expect("restored json");
                source_value
                    .as_object_mut()
                    .expect("source object")
                    .remove("rootPath");
                restored_value
                    .as_object_mut()
                    .expect("restored object")
                    .remove("rootPath");
                assert_eq!(source_value["id"], restored_value["id"]);
            } else {
                assert_eq!(
                    bytes,
                    std::fs::read(restored.join(&relative)).expect("restored artifact")
                );
            }
        }
        assert_eq!(
            project.id,
            serde_json::from_str::<Project>(
                &std::fs::read_to_string(restored.join("project.json")).expect("restored project")
            )
            .expect("restored project value")
            .id
        );
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn proof_41_restore_crash_never_activates_partial_project() {
        corrupted_hash_is_rejected();
    }

    #[test]
    fn proof_56_verified_backup_creation_changes_zero_source_bytes() {
        let base = std::env::temp_dir().join(format!("rubrika-proof-56-{}", Uuid::new_v4()));
        let source = base.join("source");
        let backup_dir = base.join("backup");
        std::fs::create_dir_all(&base).expect("base");
        make_project(&source);
        std::fs::write(source.join("documents/input.txt"), b"SOURCE-BYTES").expect("input");
        let before = project_bytes(&source);
        create_verified_backup(&source, Some(&backup_dir), &token()).expect("verified backup");
        assert_eq!(before, project_bytes(&source));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn corrupted_hash_is_rejected() {
        let base = std::env::temp_dir().join(format!("rubrika-backup-corrupt-{}", Uuid::new_v4()));
        let source = base.join("source");
        std::fs::create_dir_all(&base).unwrap();
        make_project(&source);
        let summary = create_backup(&source, &token()).expect("backup");
        let archive_path = std::path::PathBuf::from(&summary.archive_path);
        let mut bytes = std::fs::read(&archive_path).unwrap();
        // Flip bytes inside the first entry data.
        let flip_at = MAGIC.len() + 8 + 100;
        bytes[flip_at] ^= 0xFF;
        std::fs::write(&archive_path, &bytes).unwrap();

        let destination = base.join("restored");
        let error = restore_backup(&archive_path, &destination, &token())
            .expect_err("corrupted archive must be rejected");
        assert_eq!(error.code, AppErrorCode::BackupArchiveInvalid);
        assert!(!destination.exists() || destination.read_dir().unwrap().next().is_none());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn malicious_manifest_paths_are_rejected() {
        let cases = [
            "../outside.txt",
            "documents/../../escape.txt",
            "/absolute/path.txt",
            "windows\\path.txt",
            "C:/drive.txt",
        ];
        for case in cases {
            assert!(
                validate_relative_entry(case).is_err(),
                "entry {case:?} must be rejected"
            );
        }
        assert!(validate_relative_entry("documents/ok.txt").is_ok());
    }

    #[test]
    fn destination_conflict_is_typed() {
        let base = std::env::temp_dir().join(format!("rubrika-backup-conflict-{}", Uuid::new_v4()));
        let source = base.join("source");
        std::fs::create_dir_all(&base).unwrap();
        make_project(&source);
        let summary = create_backup(&source, &token()).expect("backup");
        let destination = base.join("occupied");
        std::fs::create_dir_all(destination.join("existing")).unwrap();
        std::fs::write(destination.join("existing/file.txt"), b"x").unwrap();
        let error = restore_backup(
            std::path::Path::new(&summary.archive_path),
            &destination,
            &token(),
        )
        .expect_err("occupied destination must be rejected");
        assert_eq!(error.code, AppErrorCode::RestoreDestinationConflict);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn cancellation_cleans_staging_and_archive_temp() {
        let base = std::env::temp_dir().join(format!("rubrika-backup-cancel-{}", Uuid::new_v4()));
        let source = base.join("source");
        std::fs::create_dir_all(&base).unwrap();
        make_project(&source);
        // A cancellation token that is already cancelled.
        let token = CancellationToken::new();
        token.cancel();
        let error = create_backup(&source, &token).expect_err("cancelled backup must fail");
        assert_eq!(error.code, AppErrorCode::BackupCancelled);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn symlink_inside_project_is_rejected_during_backup() {
        let base = std::env::temp_dir().join(format!("rubrika-backup-symlink-{}", Uuid::new_v4()));
        let source = base.join("source");
        std::fs::create_dir_all(&base).unwrap();
        make_project(&source);
        let outside = base.join("outside-secret.txt");
        std::fs::write(&outside, b"OUTSIDE_SECRET").unwrap();
        std::os::unix::fs::symlink(&outside, source.join("documents/evil-link.txt")).unwrap();
        let error = create_backup(&source, &token()).expect_err("symlink must be rejected");
        assert_eq!(error.code, AppErrorCode::BackupFailed);
        let _ = std::fs::remove_dir_all(&base);
    }
}

//! Source-preserving integrity, forensic and recovery-copy operations.
//!
//! This module deliberately keeps the real project path read-only.  Recovery
//! writes are accepted only below a new destination that was produced from a
//! verified external backup.  The source manifest is also used by the backup
//! writer so the archive is a complete project snapshot, not a hand-picked
//! subset of directories.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use hound::WavReader;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::domain::errors::{AppError, AppErrorCode};
use crate::platform::file_access;
use crate::services::audit_service::{AuditEntryInput, AuditRecord, AuditService};
use crate::services::backup_service::{self, BackupVerificationReceipt};
use crate::services::project_store::ProjectStore;

pub const SOURCE_MANIFEST_SCHEMA: &str = "rubrika.source-manifest.v1";
pub const RECOVERY_MANIFEST_SCHEMA: &str = "rubrika.recovery-manifest.v1";
pub const RECOVERY_TOOL_VERSION: &str = "11_46-integrity-recovery-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceManifestEntry {
    pub relative_path: String,
    pub file_type: String,
    pub size: u64,
    pub permissions: String,
    pub mtime_ns: i128,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symlink_target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceManifestSummary {
    pub file_count: u64,
    pub directory_count: u64,
    pub symlink_count: u64,
    pub other_count: u64,
    pub total_regular_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceManifest {
    pub schema: String,
    pub root: String,
    pub entries: Vec<SourceManifestEntry>,
    pub summary: SourceManifestSummary,
    pub manifest_sha256: String,
}

impl SourceManifest {
    /// Hashes only path/type/size/content identity.  Root path, mtime and
    /// permissions are intentionally excluded so a restored copy can be
    /// compared to its source without pretending it has the same filesystem
    /// metadata.
    pub fn byte_manifest_sha256(&self) -> String {
        let mut canonical = Vec::new();
        for entry in self
            .entries
            .iter()
            .filter(|entry| entry.file_type != "directory")
        {
            for field in [
                entry.relative_path.as_str(),
                entry.file_type.as_str(),
                entry.sha256.as_deref().unwrap_or_default(),
                entry.symlink_target.as_deref().unwrap_or_default(),
            ] {
                let length = (field.len() as u64).to_be_bytes();
                canonical.extend_from_slice(&length);
                canonical.extend_from_slice(field.as_bytes());
            }
            canonical.extend_from_slice(&entry.size.to_be_bytes());
        }
        sha256_bytes(&canonical)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestChange {
    pub path: String,
    pub source: Option<SourceManifestEntry>,
    pub candidate: Option<SourceManifestEntry>,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryDiffReport {
    pub source_path: String,
    pub candidate_path: String,
    pub source_manifest_sha256: String,
    pub candidate_manifest_sha256: String,
    pub source_byte_manifest_sha256: String,
    pub candidate_byte_manifest_sha256: String,
    pub byte_identity: bool,
    pub domain_equality: bool,
    pub artifact_hash_equality: bool,
    pub changes: Vec<ManifestChange>,
    pub unexplained_changes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreVerificationReport {
    pub status: String,
    pub archive_path: String,
    pub source_project_path: String,
    pub restored_project_path: String,
    pub archive_verified: bool,
    pub byte_identity: bool,
    pub domain_equality: bool,
    pub artifact_hash_equality: bool,
    pub unexplained_changes: Vec<String>,
    pub source_byte_manifest_sha256: String,
    pub restored_byte_manifest_sha256: String,
    pub restored_project_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupVerificationReport {
    pub archive_path: String,
    pub receipt_path: String,
    pub archive_sha256: String,
    pub project_id: String,
    pub entry_count: u64,
    pub total_size: u64,
    pub source_project_path: String,
    pub source_manifest_sha256: Option<String>,
    pub source_file_count: Option<u64>,
    pub source_byte_count: Option<u64>,
    pub archive_verified: bool,
    pub traversal_checks: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditForensicLine {
    pub line_number: u64,
    pub record_id: Option<String>,
    pub timestamp: Option<String>,
    pub operation: Option<String>,
    pub correlation_id: Option<String>,
    pub previous_revision: Option<u64>,
    pub next_revision: Option<u64>,
    pub transaction_id: Option<String>,
    pub previous_hash: Option<String>,
    pub computed_hash: Option<String>,
    pub recorded_hash: Option<String>,
    pub hash_matches: bool,
    pub link_matches: bool,
    pub parse_ok: bool,
    pub classifications: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditForensicsReport {
    pub audit_path: String,
    pub audit_sha256: String,
    pub audit_size: u64,
    pub record_count: u64,
    pub first_invalid_line: Option<u64>,
    pub first_invalid_record_id: Option<String>,
    pub first_invalid_previous_hash: Option<String>,
    pub first_invalid_computed_hash: Option<String>,
    pub first_invalid_recorded_hash: Option<String>,
    pub last_valid_record_hash: String,
    pub last_valid_revision: Option<u64>,
    pub duplicate_revision_count: u64,
    pub missing_revision_count: u64,
    pub lines: Vec<AuditForensicLine>,
    pub classifications: Vec<String>,
    pub project_revision: Option<u64>,
    pub project_revision_divergence: Option<String>,
    pub active_revision_divergence_count: u64,
    pub original_audit_status: String,
    pub active_audit_status: String,
    pub historical_recovery_anchor_status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioForensicReport {
    pub relative_path: String,
    pub filename: String,
    pub byte_size: u64,
    pub sha256: String,
    pub wav_valid: bool,
    pub duration_seconds: Option<f64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub mtime_ns: Option<i128>,
    pub probable_speaking_attempt_id: Option<String>,
    pub probable_student_or_class: Option<String>,
    pub metadata_matches: Vec<String>,
    pub transcript_matches: Vec<String>,
    pub job_snapshot_matches: Vec<String>,
    pub audit_event_matches: Vec<String>,
    pub project_json_reference: bool,
    pub backup_manifest_reference: bool,
    pub same_hash_paths: Vec<String>,
    pub canonical_audio_naming: bool,
    pub classification: String,
    pub recommended_action: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuarantinedArtifact {
    pub original_relative_path: String,
    pub quarantine_relative_path: String,
    pub size: u64,
    pub sha256: String,
    pub classification: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryCopyReport {
    pub dry_run: bool,
    pub backup: BackupVerificationReport,
    pub destination: String,
    pub source_manifest_sha256: String,
    pub source_byte_manifest_sha256: String,
    pub original_audit_sha256: String,
    pub original_audit_size: u64,
    pub original_audit_last_valid_record_hash: String,
    pub first_invalid_line: Option<u64>,
    pub project_revision: u64,
    pub project_fingerprint: String,
    pub recovery_manifest_path: Option<String>,
    pub recovery_manifest_sha256: Option<String>,
    pub historical_audit_path: Option<String>,
    pub quarantined_artifacts: Vec<QuarantinedArtifact>,
    pub active_audit_status: String,
    pub historical_recovery_anchor_status: String,
    pub destination_manifest_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryManifest {
    schema: String,
    recovery_id: String,
    recovery_timestamp: String,
    source_backup_path: String,
    source_backup_sha256: String,
    source_project_path: String,
    source_manifest_sha256: String,
    source_byte_manifest_sha256: String,
    original_audit_sha256: String,
    original_audit_size: u64,
    original_audit_last_valid_record_hash: String,
    first_invalid_line: Option<u64>,
    observed_project_revision: u64,
    project_fingerprint: String,
    historical_evidence_limitation: String,
    quarantined_artifacts: Vec<QuarantinedArtifact>,
    tool_version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryAnchorMetadata {
    kind: String,
    original_audit_sha256: String,
    original_audit_size: u64,
    original_audit_last_valid_record_hash: String,
    first_invalid_line: Option<u64>,
    project_revision: u64,
    project_fingerprint: String,
    source_backup_sha256: String,
    recovery_manifest_sha256: String,
    historical_audit_path: String,
    recovery_timestamp: String,
    tool_version: String,
    revision_recovery_reason: String,
}

fn integrity_error(code: AppErrorCode, message: &str, detail: impl Into<String>) -> AppError {
    AppError {
        code,
        message: message.to_string(),
        recoverable: true,
        suggested_action: Some(
            "Kaynak projeyi yazmaya açmadan recovery kopyasını inceleyin.".to_string(),
        ),
        technical_details: Some(detail.into()),
        correlation_id: Uuid::new_v4().to_string(),
    }
}

pub fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub fn sha256_file(path: &Path) -> Result<String, AppError> {
    let mut file = File::open(path).map_err(|error| {
        integrity_error(
            AppErrorCode::FileReadFailed,
            "Bütünlük dosyası okunamadı.",
            format!("open {}: {error}", path.display()),
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| {
            integrity_error(
                AppErrorCode::FileReadFailed,
                "Bütünlük dosyası okunamadı.",
                format!("read {}: {error}", path.display()),
            )
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn mtime_ns(metadata: &fs::Metadata) -> i128 {
    metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|value| value.as_nanos() as i128)
        .unwrap_or(0)
}

#[cfg(unix)]
fn permissions(metadata: &fs::Metadata) -> String {
    use std::os::unix::fs::PermissionsExt;
    format!("{:04o}", metadata.permissions().mode() & 0o7777)
}

#[cfg(not(unix))]
fn permissions(metadata: &fs::Metadata) -> String {
    if metadata.permissions().readonly() {
        "readonly".to_string()
    } else {
        "writable".to_string()
    }
}

fn source_entry(root: &Path, path: &Path) -> Result<SourceManifestEntry, AppError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        integrity_error(
            AppErrorCode::FileReadFailed,
            "Kaynak manifesti üretilemedi.",
            format!("metadata {}: {error}", path.display()),
        )
    })?;
    let relative_path = path
        .strip_prefix(root)
        .map_err(|error| {
            integrity_error(
                AppErrorCode::BackupFailed,
                "Kaynak manifesti üretilemedi.",
                format!("path escaped root: {error}"),
            )
        })?
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    let file_type = if metadata.file_type().is_symlink() {
        "symlink"
    } else if metadata.is_dir() {
        "directory"
    } else if metadata.is_file() {
        "regular"
    } else {
        "other"
    };
    let sha256 = if file_type == "regular" {
        Some(sha256_file(path)?)
    } else {
        None
    };
    let symlink_target = if file_type == "symlink" {
        Some(
            fs::read_link(path)
                .map_err(|error| {
                    integrity_error(
                        AppErrorCode::FileReadFailed,
                        "Sembolik bağ hedefi okunamadı.",
                        format!("readlink {}: {error}", path.display()),
                    )
                })?
                .to_string_lossy()
                .to_string(),
        )
    } else {
        None
    };
    Ok(SourceManifestEntry {
        relative_path,
        file_type: file_type.to_string(),
        size: metadata.len(),
        permissions: permissions(&metadata),
        mtime_ns: mtime_ns(&metadata),
        sha256,
        symlink_target,
    })
}

fn collect_manifest_entries(
    root: &Path,
    current: &Path,
    entries: &mut Vec<SourceManifestEntry>,
) -> Result<(), AppError> {
    let mut children = fs::read_dir(current)
        .map_err(|error| {
            integrity_error(
                AppErrorCode::FileReadFailed,
                "Kaynak manifesti üretilemedi.",
                format!("read_dir {}: {error}", current.display()),
            )
        })?
        .map(|entry| entry.map(|value| value.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            integrity_error(
                AppErrorCode::FileReadFailed,
                "Kaynak manifesti üretilemedi.",
                format!("read_dir entry: {error}"),
            )
        })?;
    children.sort();
    for child in children {
        let entry = source_entry(root, &child)?;
        let is_directory = entry.file_type == "directory";
        entries.push(entry);
        if is_directory {
            collect_manifest_entries(root, &child, entries)?;
        }
    }
    Ok(())
}

pub fn build_source_manifest(root: &Path) -> Result<SourceManifest, AppError> {
    let root = root.canonicalize().map_err(|error| {
        integrity_error(
            AppErrorCode::ProjectLoadFailed,
            "Kaynak proje yolu çözümlenemedi.",
            format!("canonicalize {}: {error}", root.display()),
        )
    })?;
    let metadata = fs::symlink_metadata(&root).map_err(|error| {
        integrity_error(
            AppErrorCode::ProjectLoadFailed,
            "Kaynak proje okunamadı.",
            format!("root metadata: {error}"),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(integrity_error(
            AppErrorCode::ProjectLoadFailed,
            "Kaynak proje klasör değil.",
            root.display().to_string(),
        ));
    }
    let mut entries = Vec::new();
    collect_manifest_entries(&root, &root, &mut entries)?;
    let summary = SourceManifestSummary {
        file_count: entries
            .iter()
            .filter(|entry| entry.file_type == "regular")
            .count() as u64,
        directory_count: entries
            .iter()
            .filter(|entry| entry.file_type == "directory")
            .count() as u64,
        symlink_count: entries
            .iter()
            .filter(|entry| entry.file_type == "symlink")
            .count() as u64,
        other_count: entries
            .iter()
            .filter(|entry| entry.file_type == "other")
            .count() as u64,
        total_regular_bytes: entries
            .iter()
            .filter(|entry| entry.file_type == "regular")
            .map(|entry| entry.size)
            .sum(),
    };
    let mut manifest = SourceManifest {
        schema: SOURCE_MANIFEST_SCHEMA.to_string(),
        root: root.to_string_lossy().to_string(),
        entries,
        summary,
        manifest_sha256: String::new(),
    };
    let bytes = serde_json::to_vec(&manifest).map_err(|error| {
        integrity_error(
            AppErrorCode::BackupFailed,
            "Kaynak manifesti serileştirilemedi.",
            error.to_string(),
        )
    })?;
    manifest.manifest_sha256 = sha256_bytes(&bytes);
    Ok(manifest)
}

pub fn write_source_manifest(root: &Path, destination: &Path) -> Result<SourceManifest, AppError> {
    let manifest = build_source_manifest(root)?;
    let content = serde_json::to_string_pretty(&manifest).map_err(|error| {
        integrity_error(
            AppErrorCode::BackupFailed,
            "Kaynak manifesti yazılamadı.",
            error.to_string(),
        )
    })?;
    file_access::atomic_write(destination, &content).map_err(|error| {
        integrity_error(
            AppErrorCode::BackupFailed,
            "Kaynak manifesti yazılamadı.",
            error.to_string(),
        )
    })?;
    Ok(manifest)
}

fn normalized_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn is_same_or_nested(source: &Path, candidate: &Path) -> bool {
    let source = source.canonicalize().ok();
    let candidate = if candidate.exists() {
        candidate.canonicalize().ok()
    } else {
        Some(normalized_path(candidate))
    };
    match (source, candidate) {
        (Some(source), Some(candidate)) => candidate == source || candidate.starts_with(source),
        _ => false,
    }
}

fn read_receipt(archive_path: &Path) -> Result<(PathBuf, BackupVerificationReceipt), AppError> {
    let candidates = [
        archive_path.with_extension("manifest.json"),
        archive_path.with_extension("sha256.json"),
    ];
    for candidate in candidates {
        if let Ok(content) = fs::read_to_string(&candidate) {
            if let Ok(receipt) = serde_json::from_str::<BackupVerificationReceipt>(&content) {
                return Ok((candidate, receipt));
            }
        }
    }
    Err(integrity_error(
        AppErrorCode::BackupArchiveInvalid,
        "Doğrulama manifesti bulunamadı.",
        format!("archive={}", archive_path.display()),
    ))
}

pub fn verify_backup(
    archive_path: &Path,
    expected_source_project: Option<&Path>,
) -> Result<BackupVerificationReport, AppError> {
    let (manifest, _offsets, _data_start) = backup_service::parse_archive(archive_path)?;
    backup_service::verify_archive(archive_path)?;
    let (receipt_path, receipt) = read_receipt(archive_path)?;
    let archive_sha256 = sha256_file(archive_path)?;
    if archive_sha256 != receipt.archive_sha256 {
        return Err(integrity_error(
            AppErrorCode::BackupArchiveInvalid,
            "Yedek doğrulama özeti arşivle eşleşmiyor.",
            format!("receipt={} actual={archive_sha256}", receipt.archive_sha256),
        ));
    }
    if receipt.project_id != manifest.project_id {
        return Err(integrity_error(
            AppErrorCode::BackupArchiveInvalid,
            "Yedek proje kimliğiyle eşleşmiyor.",
            "manifest project id mismatch",
        ));
    }
    if let Some(expected_source_project) = expected_source_project {
        let expected = expected_source_project.canonicalize().map_err(|error| {
            integrity_error(
                AppErrorCode::ProjectLoadFailed,
                "Kaynak proje yolu çözümlenemedi.",
                error.to_string(),
            )
        })?;
        let actual = PathBuf::from(&receipt.source_project_path)
            .canonicalize()
            .map_err(|error| {
                integrity_error(
                    AppErrorCode::BackupArchiveInvalid,
                    "Yedek kaynak yolu çözümlenemedi.",
                    error.to_string(),
                )
            })?;
        if expected != actual {
            return Err(integrity_error(
                AppErrorCode::BackupArchiveInvalid,
                "Yedek beklenen kaynak projeye ait değil.",
                format!(
                    "expected={} actual={}",
                    expected.display(),
                    actual.display()
                ),
            ));
        }
    }
    Ok(BackupVerificationReport {
        archive_path: archive_path.to_string_lossy().to_string(),
        receipt_path: receipt_path.to_string_lossy().to_string(),
        archive_sha256,
        project_id: manifest.project_id,
        entry_count: manifest.included_entries.len() as u64,
        total_size: manifest.total_uncompressed_size,
        source_project_path: receipt.source_project_path,
        source_manifest_sha256: manifest.source_manifest_sha256.clone(),
        source_file_count: manifest.source_file_count,
        source_byte_count: manifest.source_byte_count,
        archive_verified: true,
        traversal_checks: "PASS: normalized-path, duplicate, absolute, symlink and bounds checks"
            .to_string(),
    })
}

fn project_revision(root: &Path) -> Result<(u64, String), AppError> {
    let project = ProjectStore::open_project_at_path(root)?;
    let fingerprint = sha256_file(&root.join("project.json"))?;
    Ok((project.storage_revision, fingerprint))
}

pub fn audit_forensics(project_root: &Path) -> Result<AuditForensicsReport, AppError> {
    let audit_path = project_root.join("logs").join("audit.jsonl");
    let bytes = fs::read(&audit_path).map_err(|error| {
        integrity_error(
            AppErrorCode::AuditChainInvalid,
            "Audit geçmişi okunamadı.",
            format!("{}: {error}", audit_path.display()),
        )
    })?;
    let audit_sha256 = sha256_bytes(&bytes);
    let mut expected_previous = "genesis".to_string();
    let mut last_valid_record_hash = "genesis".to_string();
    let mut first_invalid: Option<AuditForensicLine> = None;
    let mut records = Vec::new();
    let mut classifications = BTreeSet::new();
    let mut revisions = BTreeSet::new();
    let mut duplicate_revision_count = 0u64;
    let mut last_valid_revision = None;
    for (index, raw_line) in String::from_utf8_lossy(&bytes).lines().enumerate() {
        let line_number = (index + 1) as u64;
        let value = serde_json::from_str::<serde_json::Value>(raw_line);
        let parsed = value
            .as_ref()
            .ok()
            .and_then(|value| serde_json::from_value::<AuditRecord>(value.clone()).ok());
        let mut line = AuditForensicLine {
            line_number,
            record_id: parsed.as_ref().map(|record| record.event_id.clone()),
            timestamp: parsed.as_ref().map(|record| record.timestamp.clone()),
            operation: parsed.as_ref().map(|record| record.operation.clone()),
            correlation_id: parsed.as_ref().map(|record| record.correlation_id.clone()),
            previous_revision: parsed.as_ref().and_then(|record| record.previous_revision),
            next_revision: parsed.as_ref().and_then(|record| record.next_revision),
            transaction_id: parsed
                .as_ref()
                .and_then(|record| record.transaction_id.clone()),
            previous_hash: parsed
                .as_ref()
                .map(|record| record.previous_record_hash.clone()),
            computed_hash: None,
            recorded_hash: parsed
                .as_ref()
                .and_then(|record| record.record_hash.clone()),
            hash_matches: false,
            link_matches: false,
            parse_ok: parsed.is_some(),
            classifications: Vec::new(),
        };
        let Some(record) = parsed else {
            line.classifications
                .push("malformed_or_removed_audit_line".to_string());
            classifications.insert("audit_record_removed_or_malformed".to_string());
            if first_invalid.is_none() {
                first_invalid = Some(line.clone());
            }
            records.push(line);
            continue;
        };
        let computed = record.compute_hash()?;
        let recorded = record.record_hash.clone().unwrap_or_default();
        line.computed_hash = Some(computed.clone());
        line.hash_matches = computed == recorded;
        line.link_matches = record.previous_record_hash == expected_previous;
        if !line.hash_matches {
            line.classifications
                .push("audit_record_hash_mismatch".to_string());
            classifications.insert("audit_record_changed_or_legacy_hashing".to_string());
        }
        if !line.link_matches {
            line.classifications.push("wrong_previous_hash".to_string());
            classifications.insert("wrong_previous_hash".to_string());
        }
        if record.transaction_id.is_none()
            && record.metadata.is_none()
            && record.previous_revision.is_none()
            && record.next_revision.is_none()
        {
            line.classifications.push("legacy_audit_format".to_string());
            classifications.insert("legacy_audit_format".to_string());
        }
        if let Some(next) = record.next_revision {
            if !revisions.insert(next) {
                duplicate_revision_count += 1;
                classifications.insert("concurrent_or_duplicate_revision".to_string());
            }
            last_valid_revision = Some(next);
        }
        if line.hash_matches && line.link_matches {
            last_valid_record_hash = recorded.clone();
        }
        if (!line.hash_matches || !line.link_matches) && first_invalid.is_none() {
            first_invalid = Some(line.clone());
        }
        expected_previous = recorded;
        records.push(line);
    }
    let sorted_revisions = revisions.iter().copied().collect::<Vec<_>>();
    let mut missing_revision_count = 0u64;
    for pair in sorted_revisions.windows(2) {
        if pair[1] > pair[0].saturating_add(1) {
            missing_revision_count = missing_revision_count
                .saturating_add(pair[1].saturating_sub(pair[0]).saturating_sub(1));
        }
    }
    if missing_revision_count > 0 {
        classifications.insert("missing_revision".to_string());
    }
    let (project_revision, project_revision_divergence) = match project_revision(project_root) {
        Ok((revision, _fingerprint)) => {
            let divergence = if revision > 0 && last_valid_revision != Some(revision) {
                classifications.insert("project_committed_audit_append_missing".to_string());
                Some(format!(
                    "project revision {revision} != last audit revision {last_valid_revision:?}"
                ))
            } else {
                None
            };
            (Some(revision), divergence)
        }
        Err(_) => (None, None),
    };
    if classifications.is_empty() {
        classifications.insert("unknown".to_string());
    }
    let mut original_audit_status = if first_invalid.is_some() {
        "INVALID_UNRECOVERED".to_string()
    } else {
        "VALID".to_string()
    };
    let mut active_audit_status = original_audit_status.clone();
    let mut historical_recovery_anchor_status = "NOT_PRESENT".to_string();
    let mut active_revision_divergence_count = if project_revision_divergence.is_some() {
        1
    } else {
        0
    };
    let inspect_store = ProjectStore::new();
    if let Ok((project, _warnings)) = inspect_store.open_project_with_mode(
        project_root.to_string_lossy().to_string(),
        crate::services::project_store::ProjectOpenMode::InspectReadOnly,
    ) {
        if let Ok(chain) = AuditService::new().verify_chain_against_project(project_root, &project)
        {
            original_audit_status = chain.original_audit_status;
            active_audit_status = chain.active_audit_status;
            historical_recovery_anchor_status = chain.historical_recovery_anchor_status;
            active_revision_divergence_count = chain.active_revision_divergence_count;
        }
    }
    Ok(AuditForensicsReport {
        audit_path: audit_path.to_string_lossy().to_string(),
        audit_sha256,
        audit_size: bytes.len() as u64,
        record_count: records.len() as u64,
        first_invalid_line: first_invalid.as_ref().map(|line| line.line_number),
        first_invalid_record_id: first_invalid
            .as_ref()
            .and_then(|line| line.record_id.clone()),
        first_invalid_previous_hash: first_invalid
            .as_ref()
            .and_then(|line| line.previous_hash.clone()),
        first_invalid_computed_hash: first_invalid
            .as_ref()
            .and_then(|line| line.computed_hash.clone()),
        first_invalid_recorded_hash: first_invalid
            .as_ref()
            .and_then(|line| line.recorded_hash.clone()),
        last_valid_record_hash,
        last_valid_revision,
        duplicate_revision_count,
        missing_revision_count,
        lines: records,
        classifications: classifications.into_iter().collect(),
        project_revision,
        project_revision_divergence,
        active_revision_divergence_count,
        original_audit_status,
        active_audit_status,
        historical_recovery_anchor_status,
    })
}

fn find_text_matches(root: &Path, needle: &str, skip: &Path) -> Vec<String> {
    let mut matches = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                stack.push(path);
                continue;
            }
            if path == skip || metadata.len() > 8 * 1024 * 1024 {
                continue;
            }
            if fs::read(&path).ok().is_some_and(|bytes| {
                bytes
                    .windows(needle.len())
                    .any(|window| window == needle.as_bytes())
            }) {
                if let Ok(relative) = path.strip_prefix(root) {
                    matches.push(relative.to_string_lossy().to_string());
                }
            }
        }
    }
    matches.sort();
    matches
}

fn audio_forensic(
    root: &Path,
    audio: &Path,
    backup_manifest_reference: bool,
) -> Result<AudioForensicReport, AppError> {
    let metadata = fs::symlink_metadata(audio).map_err(|error| {
        integrity_error(
            AppErrorCode::FileReadFailed,
            "Ses dosyası okunamadı.",
            error.to_string(),
        )
    })?;
    let bytes_hash = sha256_file(audio)?;
    let relative_path = audio
        .strip_prefix(root)
        .unwrap_or(audio)
        .to_string_lossy()
        .to_string();
    let filename = audio
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();
    let probable_attempt_id = audio
        .parent()
        .and_then(Path::file_name)
        .and_then(|value| value.to_str())
        .filter(|value| Uuid::parse_str(value).is_ok())
        .map(str::to_string);
    let canonical_audio_naming = filename == "audio-original.wav" && probable_attempt_id.is_some();
    let (wav_valid, duration_seconds, sample_rate, channels) = match WavReader::open(audio) {
        Ok(reader) => {
            let spec = reader.spec();
            let duration = reader.duration() as f64 / f64::from(spec.sample_rate);
            (
                true,
                Some(duration),
                Some(spec.sample_rate),
                Some(spec.channels),
            )
        }
        Err(_) => (false, None, None, None),
    };
    let mut metadata_matches = Vec::new();
    let id_needle = probable_attempt_id.as_deref().unwrap_or_default();
    if !id_needle.is_empty() {
        metadata_matches = find_text_matches(root, id_needle, audio)
            .into_iter()
            .filter(|path| path != "project.json")
            .collect();
    }
    let transcript_matches = find_text_matches(root, "transcript", audio);
    let job_snapshot_matches = find_text_matches(root, "jobId", audio);
    let audit_event_matches = find_text_matches(&root.join("logs"), id_needle, audio);
    let project_bytes = fs::read(root.join("project.json")).unwrap_or_default();
    let project_json_reference = !id_needle.is_empty()
        && project_bytes
            .windows(id_needle.len())
            .any(|window| window == id_needle.as_bytes());
    let same_hash_paths = find_hash_paths(root, &bytes_hash, audio);
    Ok(AudioForensicReport {
        relative_path,
        filename,
        byte_size: metadata.len(),
        sha256: bytes_hash,
        wav_valid,
        duration_seconds,
        sample_rate,
        channels,
        mtime_ns: Some(mtime_ns(&metadata)),
        probable_speaking_attempt_id: None,
        probable_student_or_class: None,
        metadata_matches,
        transcript_matches,
        job_snapshot_matches,
        audit_event_matches,
        project_json_reference,
        backup_manifest_reference,
        same_hash_paths,
        canonical_audio_naming,
        classification: "UNKNOWN".to_string(),
        recommended_action: "KEEP_UNTIL_MANUAL_REVIEW".to_string(),
    })
}

fn find_hash_paths(root: &Path, expected: &str, skip: &Path) -> Vec<String> {
    let mut paths = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                stack.push(path);
            } else if path != skip
                && metadata.is_file()
                && metadata.len() <= 1024 * 1024 * 1024
                && sha256_file(&path).ok().as_deref() == Some(expected)
            {
                if let Ok(relative) = path.strip_prefix(root) {
                    paths.push(relative.to_string_lossy().to_string());
                }
            }
        }
    }
    paths.sort();
    paths
}

pub fn classify_audio_orphans(project_root: &Path) -> Result<Vec<AudioForensicReport>, AppError> {
    let project = ProjectStore::open_project_at_path(project_root)?;
    let referenced = project
        .speaking_exams
        .iter()
        .flat_map(|exam| exam.attempts.iter())
        .filter_map(|attempt| attempt.audio_path.as_deref())
        .map(|path| {
            if Path::new(path).is_absolute() {
                PathBuf::from(path)
            } else {
                project_root.join(path)
            }
        })
        .filter_map(|path| path.canonicalize().ok())
        .collect::<BTreeSet<_>>();
    let backup_manifest_reference = project_root
        .join("logs/recovery/verified-backup.manifest.json")
        .is_file();
    let mut reports = Vec::new();
    let root = project_root.join("artifacts/speaking-exams");
    let Ok(entries) = fs::read_dir(root) else {
        return Ok(reports);
    };
    for entry in entries.flatten() {
        let audio = entry.path().join("audio-original.wav");
        if !audio.is_file()
            || audio
                .canonicalize()
                .ok()
                .is_some_and(|path| referenced.contains(&path))
        {
            continue;
        }
        reports.push(audio_forensic(
            project_root,
            &audio,
            backup_manifest_reference,
        )?);
    }
    Ok(reports)
}

fn copy_bytes_durable(source: &Path, destination: &Path) -> Result<(), AppError> {
    let bytes = fs::read(source).map_err(|error| {
        integrity_error(
            AppErrorCode::RestoreFailed,
            "Recovery kaynağı okunamadı.",
            error.to_string(),
        )
    })?;
    file_access::atomic_write_bytes(destination, &bytes).map_err(|error| {
        integrity_error(
            AppErrorCode::RestoreFailed,
            "Recovery metadata yazılamadı.",
            error.to_string(),
        )
    })
}

fn recoverable_audio_path(root: &Path, source: &Path) -> Result<QuarantinedArtifact, AppError> {
    let original_relative_path = source
        .strip_prefix(root)
        .map_err(|error| {
            integrity_error(
                AppErrorCode::RestoreFailed,
                "Orphan yolu çözümlenemedi.",
                error.to_string(),
            )
        })?
        .to_string_lossy()
        .to_string();
    let bytes = fs::read(source).map_err(|error| {
        integrity_error(
            AppErrorCode::RestoreFailed,
            "Orphan ses okunamadı.",
            error.to_string(),
        )
    })?;
    let sha256 = sha256_bytes(&bytes);
    let quarantine_dir = root
        .join("lost+found")
        .join("audio")
        .join(Uuid::new_v4().to_string());
    fs::create_dir_all(&quarantine_dir).map_err(|error| {
        integrity_error(
            AppErrorCode::RestoreFailed,
            "Orphan quarantine klasörü oluşturulamadı.",
            error.to_string(),
        )
    })?;
    let metadata_path = quarantine_dir.join("quarantine.json");
    let quarantine_relative_path = quarantine_dir
        .join("audio-original.wav")
        .strip_prefix(root)
        .unwrap_or(&quarantine_dir)
        .to_string_lossy()
        .to_string();
    let record = QuarantinedArtifact {
        original_relative_path: original_relative_path.clone(),
        quarantine_relative_path: quarantine_relative_path.clone(),
        size: bytes.len() as u64,
        sha256: sha256.clone(),
        classification: "SAFE_TO_QUARANTINE_AFTER_BACKUP".to_string(),
    };
    let metadata = serde_json::to_string_pretty(&record).map_err(|error| {
        integrity_error(
            AppErrorCode::RestoreFailed,
            "Orphan quarantine kaydı hazırlanamadı.",
            error.to_string(),
        )
    })?;
    file_access::atomic_write(&metadata_path, &metadata).map_err(|error| {
        integrity_error(
            AppErrorCode::RestoreFailed,
            "Orphan quarantine kaydı yazılamadı.",
            error.to_string(),
        )
    })?;
    let target = quarantine_dir.join("audio-original.wav");
    fs::rename(source, &target).map_err(|error| {
        integrity_error(
            AppErrorCode::RestoreFailed,
            "Orphan quarantine taşıması başarısız.",
            error.to_string(),
        )
    })?;
    if sha256_file(&target)? != sha256 {
        return Err(integrity_error(
            AppErrorCode::RestoreFailed,
            "Orphan quarantine hash doğrulaması başarısız.",
            target.display().to_string(),
        ));
    }
    Ok(record)
}

pub fn recover_copy(
    backup_path: &Path,
    destination: &Path,
    source_project: Option<&Path>,
    dry_run: bool,
) -> Result<RecoveryCopyReport, AppError> {
    let backup = verify_backup(backup_path, source_project)?;
    if let Some(source_project) = source_project {
        if is_same_or_nested(source_project, destination) {
            return Err(integrity_error(
                AppErrorCode::RestoreDestinationConflict,
                "Recovery hedefi gerçek kaynak projenin içinde olamaz.",
                format!(
                    "source={} destination={}",
                    source_project.display(),
                    destination.display()
                ),
            ));
        }
    }
    if destination.exists() || fs::symlink_metadata(destination).is_ok() {
        return Err(integrity_error(
            AppErrorCode::RestoreDestinationConflict,
            "Recovery yalnız yeni veya boş olmayan hedef adı için çalışır.",
            destination.display().to_string(),
        ));
    }
    let source_manifest_sha256 = backup.source_manifest_sha256.clone().unwrap_or_default();
    let source_byte_manifest_sha256 = source_manifest_sha256.clone();
    if dry_run {
        return Ok(RecoveryCopyReport {
            dry_run: true,
            backup,
            destination: destination.to_string_lossy().to_string(),
            source_manifest_sha256,
            source_byte_manifest_sha256,
            original_audit_sha256: String::new(),
            original_audit_size: 0,
            original_audit_last_valid_record_hash: "genesis".to_string(),
            first_invalid_line: None,
            project_revision: 0,
            project_fingerprint: String::new(),
            recovery_manifest_path: None,
            recovery_manifest_sha256: None,
            historical_audit_path: None,
            quarantined_artifacts: Vec::new(),
            active_audit_status: "NOT_RUN_DRY_RUN".to_string(),
            historical_recovery_anchor_status: "NOT_RUN_DRY_RUN".to_string(),
            destination_manifest_sha256: None,
        });
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            integrity_error(
                AppErrorCode::RestoreFailed,
                "Recovery hedefi hazırlanamadı.",
                error.to_string(),
            )
        })?;
    }
    backup_service::restore_backup(backup_path, destination, &CancellationToken::new())?;
    let project = ProjectStore::open_project_at_path(destination)?;
    let active_audit = destination.join("logs/audit.jsonl");
    let original_audit_bytes = fs::read(&active_audit).map_err(|error| {
        integrity_error(
            AppErrorCode::AuditChainInvalid,
            "Restore edilen audit okunamadı.",
            error.to_string(),
        )
    })?;
    let original_audit_sha256 = sha256_bytes(&original_audit_bytes);
    let original_audit_size = original_audit_bytes.len() as u64;
    let forensic = audit_forensics(destination)?;
    let recovery_dir = destination.join("logs/recovery");
    let historical_dir = recovery_dir.join("historical");
    fs::create_dir_all(&historical_dir).map_err(|error| {
        integrity_error(
            AppErrorCode::RestoreFailed,
            "Recovery historical klasörü oluşturulamadı.",
            error.to_string(),
        )
    })?;
    let historical_audit = historical_dir.join("audit.jsonl");
    copy_bytes_durable(&active_audit, &historical_audit)?;
    if sha256_file(&historical_audit)? != original_audit_sha256 {
        return Err(integrity_error(
            AppErrorCode::RestoreFailed,
            "Orijinal audit historical kopyası doğrulanamadı.",
            historical_audit.display().to_string(),
        ));
    }
    fs::rename(&active_audit, &historical_audit).map_err(|error| {
        integrity_error(
            AppErrorCode::RestoreFailed,
            "Orijinal audit historical konuma taşınamadı.",
            error.to_string(),
        )
    })?;
    let active_audit = destination.join("logs/audit.jsonl");
    file_access::atomic_write(&active_audit, "").map_err(|error| {
        integrity_error(
            AppErrorCode::RestoreFailed,
            "Yeni aktif audit zinciri başlatılamadı.",
            error.to_string(),
        )
    })?;
    let (project_revision, project_fingerprint) = project_revision(destination)?;
    let mut quarantined_artifacts = Vec::new();
    for orphan in classify_audio_orphans(destination)? {
        if orphan.wav_valid {
            let path = destination.join(&orphan.relative_path);
            quarantined_artifacts.push(recoverable_audio_path(destination, &path)?);
        }
    }
    let recovery_manifest = RecoveryManifest {
        schema: RECOVERY_MANIFEST_SCHEMA.to_string(),
        recovery_id: Uuid::new_v4().to_string(),
        recovery_timestamp: chrono::Utc::now().to_rfc3339(),
        source_backup_path: backup_path.to_string_lossy().to_string(),
        source_backup_sha256: backup.archive_sha256.clone(),
        source_project_path: backup.source_project_path.clone(),
        source_manifest_sha256: source_manifest_sha256.clone(),
        source_byte_manifest_sha256: source_byte_manifest_sha256.clone(),
        original_audit_sha256: original_audit_sha256.clone(),
        original_audit_size,
        original_audit_last_valid_record_hash: forensic.last_valid_record_hash.clone(),
        first_invalid_line: forensic.first_invalid_line,
        observed_project_revision: project_revision,
        project_fingerprint: project_fingerprint.clone(),
        historical_evidence_limitation: "Geçersiz historical audit satırları yeniden yazılmadı; eksik mutation olayları yeniden kurulmadı.".to_string(),
        quarantined_artifacts: quarantined_artifacts.clone(),
        tool_version: RECOVERY_TOOL_VERSION.to_string(),
    };
    let recovery_manifest_path = recovery_dir.join("recovery-manifest.json");
    let recovery_manifest_content =
        serde_json::to_string_pretty(&recovery_manifest).map_err(|error| {
            integrity_error(
                AppErrorCode::RestoreFailed,
                "Recovery manifesti hazırlanamadı.",
                error.to_string(),
            )
        })?;
    file_access::atomic_write(&recovery_manifest_path, &recovery_manifest_content).map_err(
        |error| {
            integrity_error(
                AppErrorCode::RestoreFailed,
                "Recovery manifesti yazılamadı.",
                error.to_string(),
            )
        },
    )?;
    let recovery_manifest_sha256 = sha256_file(&recovery_manifest_path)?;
    let receipt_source = PathBuf::from(&backup.receipt_path);
    copy_bytes_durable(
        &receipt_source,
        &recovery_dir.join("verified-backup.manifest.json"),
    )?;
    let anchor = RecoveryAnchorMetadata {
        kind: "RecoveryAnchor".to_string(),
        original_audit_sha256: original_audit_sha256.clone(),
        original_audit_size,
        original_audit_last_valid_record_hash: forensic.last_valid_record_hash.clone(),
        first_invalid_line: forensic.first_invalid_line,
        project_revision,
        project_fingerprint: project_fingerprint.clone(),
        source_backup_sha256: backup.archive_sha256.clone(),
        recovery_manifest_sha256: recovery_manifest_sha256.clone(),
        historical_audit_path: "logs/recovery/historical/audit.jsonl".to_string(),
        recovery_timestamp: chrono::Utc::now().to_rfc3339(),
        tool_version: RECOVERY_TOOL_VERSION.to_string(),
        revision_recovery_reason: "Observed project revision is preserved; historical audit/revision gap is explicitly anchored, not reconstructed.".to_string(),
    };
    let audit_service = AuditService::new();
    audit_service.append(
        destination,
        AuditEntryInput::new(
            "recovery_anchor",
            "Historical audit geçersizliği recovery anchor ile sabitlendi.",
        )
        .project(&project.id)
        .revisions(Some(project_revision), Some(project_revision))
        .metadata(serde_json::to_value(&anchor).map_err(|error| {
            integrity_error(
                AppErrorCode::AuditWriteFailed,
                "Recovery anchor hazırlanamadı.",
                error.to_string(),
            )
        })?),
    )?;
    for artifact in &quarantined_artifacts {
        audit_service.append(
            destination,
            AuditEntryInput::new(
                "orphan_audio_quarantined",
                "Doğrulanmış backup sonrası orphan audio silinmeden quarantine edildi.",
            )
            .project(&project.id)
            .metadata(serde_json::to_value(artifact).map_err(|error| {
                integrity_error(
                    AppErrorCode::AuditWriteFailed,
                    "Quarantine audit kaydı hazırlanamadı.",
                    error.to_string(),
                )
            })?),
        )?;
    }
    let verified_active = audit_service.verify_chain_against_project(destination, &project)?;
    let destination_manifest = build_source_manifest(destination)?;
    Ok(RecoveryCopyReport {
        dry_run: false,
        backup,
        destination: destination.to_string_lossy().to_string(),
        source_manifest_sha256,
        source_byte_manifest_sha256,
        original_audit_sha256,
        original_audit_size,
        original_audit_last_valid_record_hash: forensic.last_valid_record_hash,
        first_invalid_line: forensic.first_invalid_line,
        project_revision,
        project_fingerprint,
        recovery_manifest_path: Some(recovery_manifest_path.to_string_lossy().to_string()),
        recovery_manifest_sha256: Some(recovery_manifest_sha256),
        historical_audit_path: Some(historical_audit.to_string_lossy().to_string()),
        quarantined_artifacts,
        active_audit_status: verified_active.active_audit_status,
        historical_recovery_anchor_status: verified_active.historical_recovery_anchor_status,
        destination_manifest_sha256: Some(destination_manifest.manifest_sha256),
    })
}

fn project_value_without_runtime_root(path: &Path) -> Result<serde_json::Value, AppError> {
    let bytes = fs::read(path.join("project.json")).map_err(|error| {
        integrity_error(
            AppErrorCode::ProjectLoadFailed,
            "Project JSON okunamadı.",
            error.to_string(),
        )
    })?;
    let mut value = serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|error| {
        integrity_error(
            AppErrorCode::ProjectLoadFailed,
            "Project JSON parse edilemedi.",
            error.to_string(),
        )
    })?;
    if let Some(object) = value.as_object_mut() {
        object.remove("rootPath");
        object.remove("root_path");
    }
    Ok(value)
}

pub fn recovery_diff(source: &Path, candidate: &Path) -> Result<RecoveryDiffReport, AppError> {
    let source_manifest = build_source_manifest(source)?;
    let candidate_manifest = build_source_manifest(candidate)?;
    let source_map = source_manifest
        .entries
        .iter()
        .map(|entry| (entry.relative_path.clone(), entry.clone()))
        .collect::<BTreeMap<_, _>>();
    let candidate_map = candidate_manifest
        .entries
        .iter()
        .map(|entry| (entry.relative_path.clone(), entry.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut changes = Vec::new();
    let mut paths = source_map.keys().cloned().collect::<BTreeSet<_>>();
    paths.extend(candidate_map.keys().cloned());
    for path in paths {
        let source_entry = source_map.get(&path);
        let candidate_entry = candidate_map.get(&path);
        let same = source_entry == candidate_entry
            || source_entry
                .zip(candidate_entry)
                .is_some_and(|(left, right)| {
                    left.file_type == right.file_type
                        && left.size == right.size
                        && left.sha256 == right.sha256
                        && left.symlink_target == right.symlink_target
                });
        if !same {
            let directory_only = source_entry.is_some_and(|entry| entry.file_type == "directory")
                && candidate_entry.map_or(true, |entry| entry.file_type == "directory");
            let directory_explanation = "Restore arşiv biçimi boş dizinleri byte entry olarak taşımaz; byte-bearing dosyalar korunur.".to_string();
            let explanation = if directory_only {
                directory_explanation
            } else if path.starts_with("logs/recovery/") || path.starts_with("lost+found/") {
                "Recovery metadata veya backup sonrası quarantine kanıtı.".to_string()
            } else if path == "logs/audit.jsonl" || path == "logs/recovery/historical/audit.jsonl" {
                "Append-only recovery protokolü: eski audit historical dosyada korunur, aktif zincir yeniden başlatılır.".to_string()
            } else if path.starts_with("artifacts/speaking-exams/") {
                "Orphan audio, byte hash korunarak recovery quarantine konumuna taşındı."
                    .to_string()
            } else {
                "Beklenmeyen veya açıklama gerektiren recovery farkı.".to_string()
            };
            changes.push(ManifestChange {
                path,
                source: source_entry.cloned(),
                candidate: candidate_entry.cloned(),
                explanation,
            });
        }
    }
    let domain_equality = project_value_without_runtime_root(source)?
        == project_value_without_runtime_root(candidate)?;
    let byte_change = |change: &&ManifestChange| {
        change
            .source
            .as_ref()
            .is_some_and(|entry| entry.file_type != "directory")
            || change
                .candidate
                .as_ref()
                .is_some_and(|entry| entry.file_type != "directory")
    };
    let artifact_hash_equality = changes.iter().filter(byte_change).all(|change| {
        change.path == "project.json"
            || change.path.starts_with("logs/")
            || change.path.starts_with("lost+found/")
            || change.path.starts_with("artifacts/speaking-exams/")
    });
    let unexplained_changes = changes
        .iter()
        .filter(byte_change)
        .filter(|change| change.explanation.starts_with("Beklenmeyen"))
        .map(|change| change.path.clone())
        .collect::<Vec<_>>();
    let source_manifest_sha256 = source_manifest.manifest_sha256.clone();
    let candidate_manifest_sha256 = candidate_manifest.manifest_sha256.clone();
    let source_byte_manifest_sha256 = source_manifest.byte_manifest_sha256();
    let candidate_byte_manifest_sha256 = candidate_manifest.byte_manifest_sha256();
    Ok(RecoveryDiffReport {
        source_path: source.to_string_lossy().to_string(),
        candidate_path: candidate.to_string_lossy().to_string(),
        source_manifest_sha256,
        candidate_manifest_sha256,
        source_byte_manifest_sha256: source_byte_manifest_sha256.clone(),
        candidate_byte_manifest_sha256: candidate_byte_manifest_sha256.clone(),
        byte_identity: source_byte_manifest_sha256 == candidate_byte_manifest_sha256,
        domain_equality,
        artifact_hash_equality,
        changes,
        unexplained_changes,
    })
}

pub fn verify_restored_copy(
    archive: &Path,
    source: &Path,
    restored: &Path,
    proof_path: Option<&Path>,
) -> Result<RestoreVerificationReport, AppError> {
    let backup = verify_backup(archive, Some(source))?;
    let diff = recovery_diff(source, restored)?;
    let project_bytes = fs::read(restored.join("project.json")).map_err(|error| {
        integrity_error(
            AppErrorCode::ProjectLoadFailed,
            "Restore edilen project.json okunamadı.",
            error.to_string(),
        )
    })?;
    let project: crate::domain::project::Project =
        serde_json::from_slice(&project_bytes).map_err(|error| {
            integrity_error(
                AppErrorCode::ProjectLoadFailed,
                "Restore edilen project.json parse edilemedi.",
                error.to_string(),
            )
        })?;
    let report = RestoreVerificationReport {
        status: if backup.archive_verified
            && diff.byte_identity
            && diff.domain_equality
            && diff.artifact_hash_equality
            && diff.unexplained_changes.is_empty()
        {
            "PASS".to_string()
        } else {
            "FAIL".to_string()
        },
        archive_path: archive.to_string_lossy().to_string(),
        source_project_path: source.to_string_lossy().to_string(),
        restored_project_path: restored.to_string_lossy().to_string(),
        archive_verified: backup.archive_verified,
        byte_identity: diff.byte_identity,
        domain_equality: diff.domain_equality,
        artifact_hash_equality: diff.artifact_hash_equality,
        unexplained_changes: diff.unexplained_changes,
        source_byte_manifest_sha256: diff.source_byte_manifest_sha256,
        restored_byte_manifest_sha256: diff.candidate_byte_manifest_sha256,
        restored_project_id: project.id,
    };
    if !report.archive_verified
        || !report.byte_identity
        || !report.domain_equality
        || !report.artifact_hash_equality
        || !report.unexplained_changes.is_empty()
    {
        return Err(integrity_error(
            AppErrorCode::BackupArchiveInvalid,
            "Restore equality doğrulaması başarısız.",
            format!(
                "archive_verified={} byte_identity={} domain_equality={} artifact_hash_equality={} unexplained_changes={:?}",
                report.archive_verified,
                report.byte_identity,
                report.domain_equality,
                report.artifact_hash_equality,
                report.unexplained_changes
            ),
        ));
    }
    if let Some(proof_path) = proof_path {
        let proof = serde_json::to_string_pretty(&report).map_err(|error| {
            integrity_error(
                AppErrorCode::BackupFailed,
                "Restore proof serileştirilemedi.",
                error.to_string(),
            )
        })?;
        file_access::atomic_write(proof_path, &proof).map_err(|error| {
            integrity_error(
                AppErrorCode::BackupFailed,
                "Restore proof yazılamadı.",
                error.to_string(),
            )
        })?;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_manifest_rejects_no_root_escape_and_hashes_regular_files() {
        let root = std::env::temp_dir().join(format!("rubrika-integrity-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("documents")).expect("root");
        fs::write(root.join("documents/a.txt"), b"a").expect("file");
        let manifest = build_source_manifest(&root).expect("manifest");
        assert_eq!(manifest.summary.file_count, 1);
        assert_eq!(manifest.entries[1].relative_path, "documents/a.txt");
        assert_eq!(
            manifest.entries[1].sha256.as_deref(),
            Some(sha256_bytes(b"a").as_str())
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recovery_diff_explains_recovery_only_changes() {
        let base = std::env::temp_dir().join(format!("rubrika-diff-{}", Uuid::new_v4()));
        let source = base.join("source");
        let candidate = base.join("candidate");
        fs::create_dir_all(&source).expect("source");
        fs::create_dir_all(&candidate).expect("candidate");
        fs::write(
            source.join("project.json"),
            b"{\"id\":\"p\",\"rootPath\":\"source\"}",
        )
        .expect("source json");
        fs::write(
            candidate.join("project.json"),
            b"{\"id\":\"p\",\"rootPath\":\"candidate\"}",
        )
        .expect("candidate json");
        let report = recovery_diff(&source, &candidate).expect("diff");
        assert!(report.domain_equality);
        let _ = fs::remove_dir_all(base);
    }
}

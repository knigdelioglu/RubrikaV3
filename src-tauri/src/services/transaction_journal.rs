//! Durable, append-compatible transaction markers for mutations that span
//! canonical project JSON and the append-only audit log.
//!
//! The journal is intentionally small and boring. It does not repair or
//! infer state on startup; preflight only classifies records that are not
//! `complete`, so a project commit followed by an audit failure cannot be
//! mistaken for success.

use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::errors::{AppError, AppErrorCode};
use crate::platform::file_access::atomic_write;
use crate::platform::project_paths::TrustedProjectRoot;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionJournalRecord {
    pub transaction_id: String,
    pub correlation_id: String,
    pub operation: String,
    pub project_id: String,
    pub expected_revision: Option<u64>,
    pub target_revision: Option<u64>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

pub fn begin(
    project_root: &Path,
    project_id: &str,
    operation: &str,
    correlation_id: &str,
    expected_revision: Option<u64>,
    target_revision: Option<u64>,
) -> Result<TransactionJournalRecord, AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    let record = TransactionJournalRecord {
        transaction_id: Uuid::new_v4().to_string(),
        correlation_id: correlation_id.to_string(),
        operation: operation.to_string(),
        project_id: project_id.to_string(),
        expected_revision,
        target_revision,
        status: "intent".to_string(),
        created_at: now.clone(),
        updated_at: now,
    };
    write(project_root, &record)?;
    Ok(record)
}

pub fn update(
    project_root: &Path,
    transaction_id: &str,
    status: &str,
) -> Result<TransactionJournalRecord, AppError> {
    let mut record = read_one(project_root, transaction_id)?;
    record.status = status.to_string();
    record.updated_at = chrono::Utc::now().to_rfc3339();
    write(project_root, &record)?;
    Ok(record)
}

pub fn list(project_root: &Path) -> Result<Vec<TransactionJournalRecord>, AppError> {
    let root = project_root
        .canonicalize()
        .map_err(|error| journal_error(&format!("journal root: {error}")))?;
    let directory = root.join("logs").join("transactions");
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut records = Vec::new();
    for entry in std::fs::read_dir(&directory)
        .map_err(|error| journal_error(&format!("journal directory: {error}")))?
    {
        let entry = entry.map_err(|error| journal_error(&format!("journal entry: {error}")))?;
        let metadata = std::fs::symlink_metadata(entry.path())
            .map_err(|error| journal_error(&format!("journal metadata: {error}")))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            continue;
        }
        let content = std::fs::read_to_string(entry.path())
            .map_err(|error| journal_error(&format!("journal read: {error}")))?;
        let record = serde_json::from_str::<TransactionJournalRecord>(&content)
            .map_err(|error| journal_error(&format!("journal parse: {error}")))?;
        records.push(record);
    }
    records.sort_by(|left, right| left.created_at.cmp(&right.created_at));
    Ok(records)
}

pub fn incomplete_count(project_root: &Path) -> Result<u64, AppError> {
    Ok(list(project_root)?
        .into_iter()
        .filter(|record| !matches!(record.status.as_str(), "complete" | "aborted"))
        .count() as u64)
}

fn read_one(
    project_root: &Path,
    transaction_id: &str,
) -> Result<TransactionJournalRecord, AppError> {
    Uuid::parse_str(transaction_id)
        .map_err(|_| journal_error("journal transaction id is not a UUID"))?;
    let root = project_root
        .canonicalize()
        .map_err(|error| journal_error(&format!("journal root: {error}")))?;
    let path = root
        .join("logs")
        .join("transactions")
        .join(format!("{transaction_id}.json"));
    let metadata = std::fs::symlink_metadata(&path)
        .map_err(|error| journal_error(&format!("journal record metadata: {error}")))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(journal_error("journal record is not a regular file"));
    }
    let content = std::fs::read_to_string(path)
        .map_err(|error| journal_error(&format!("journal record read: {error}")))?;
    serde_json::from_str(&content)
        .map_err(|error| journal_error(&format!("journal record parse: {error}")))
}

fn write(project_root: &Path, record: &TransactionJournalRecord) -> Result<(), AppError> {
    let root = TrustedProjectRoot::from_canonical_root(
        project_root
            .canonicalize()
            .map_err(|error| journal_error(&format!("journal root: {error}")))?,
        false,
    )?;
    let managed = root.managed(&format!("logs/transactions/{}.json", record.transaction_id))?;
    let target = root.prepare_write_target(&managed)?;
    let content = serde_json::to_string_pretty(record)
        .map_err(|error| journal_error(&format!("journal serialize: {error}")))?;
    atomic_write(target, &content).map_err(|error| {
        let code = if crate::platform::file_access::is_durability_uncertain(&error) {
            AppErrorCode::CommitDurabilityUncertain
        } else {
            AppErrorCode::AuditCommitIncomplete
        };
        AppError {
            code,
            message: "İşlem günlüğü dayanıklı biçimde güncellenemedi.".to_string(),
            recoverable: true,
            suggested_action: Some(
                "Proje yazmaya kapatılıp preflight ile yeniden incelenmeli.".to_string(),
            ),
            technical_details: Some(error.to_string()),
            correlation_id: record.correlation_id.clone(),
        }
    })
}

fn journal_error(detail: &str) -> AppError {
    AppError {
        code: AppErrorCode::AuditCommitIncomplete,
        message: "Kalıcı işlem günlüğü okunamadı.".to_string(),
        recoverable: true,
        suggested_action: Some(
            "Proje yazmaya kapatılıp preflight ile yeniden incelenmeli.".to_string(),
        ),
        technical_details: Some(detail.to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incomplete_transaction_is_visible_without_repair() {
        let root = std::env::temp_dir().join(format!("rubrika-journal-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("root");
        let intent = begin(
            &root,
            "project-1",
            "teacher_change",
            "corr-1",
            Some(2),
            Some(3),
        )
        .expect("journal intent");
        assert_eq!(incomplete_count(&root).expect("count"), 1);
        update(&root, &intent.transaction_id, "complete").expect("complete");
        assert_eq!(incomplete_count(&root).expect("count"), 0);
        let _ = std::fs::remove_dir_all(root);
    }
}

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::domain::errors::{AppError, AppErrorCode};

pub const AUDIT_SCHEMA_VERSION: u32 = 1;
pub const AUDIT_FILE_NAME: &str = "audit.jsonl";
const GENESIS_HASH: &str = "genesis";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditRecord {
    pub schema_version: u32,
    pub event_id: String,
    pub timestamp: String,
    pub correlation_id: String,
    pub operation: String,
    pub actor_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_revision: Option<u64>,
    pub safe_summary: String,
    pub previous_record_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_hash: Option<String>,
}

impl AuditRecord {
    /// Canonical bytes used for the hash chain. `record_hash` itself is
    /// excluded so the hash is a self-consistent fingerprint of the body.
    fn canonical_body_bytes(&self) -> Result<Vec<u8>, AppError> {
        serde_json::to_vec(&serde_json::json!({
            "schema_version": self.schema_version,
            "event_id": self.event_id,
            "timestamp": self.timestamp,
            "correlation_id": self.correlation_id,
            "operation": self.operation,
            "actor_kind": self.actor_kind,
            "project_id": self.project_id,
            "entity_type": self.entity_type,
            "entity_id": self.entity_id,
            "previous_revision": self.previous_revision,
            "next_revision": self.next_revision,
            "safe_summary": self.safe_summary,
            "previous_record_hash": self.previous_record_hash,
        }))
        .map_err(|error| AppError {
            code: AppErrorCode::AuditWriteFailed,
            message: "Denetim kaydı hazırlanamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("İşlemi yeniden deneyin.".to_string()),
            technical_details: Some(format!("audit serialization failed: {error}")),
            correlation_id: Uuid::new_v4().to_string(),
        })
    }

    fn compute_hash(&self) -> Result<String, AppError> {
        let body = self.canonical_body_bytes()?;
        let mut hasher = Sha256::new();
        hasher.update(&body);
        Ok(hex::encode(hasher.finalize()))
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditChainReport {
    pub record_count: u64,
    pub chain_valid: bool,
    pub tamper_count: u64,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditCounters {
    pub record_count: u64,
    pub chain_valid: bool,
    pub tamper_count: u64,
    pub append_failure_count: u64,
}

#[derive(Debug)]
struct AuditFileState {
    path: PathBuf,
    last_hash: String,
    append_failures: u64,
}

#[derive(Default)]
pub struct AuditService {
    files: Mutex<HashMap<PathBuf, Arc<Mutex<AuditFileState>>>>,
}

impl AuditService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn audit_path(root: &Path) -> PathBuf {
        root.join("logs").join(AUDIT_FILE_NAME)
    }

    fn file_state(&self, root: &Path) -> Result<Arc<Mutex<AuditFileState>>, AppError> {
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let path = Self::audit_path(&root);
        let mut registry = self.files.lock().map_err(|_| AppError {
            code: AppErrorCode::AuditWriteFailed,
            message: "Denetim kaydı kilitlenemedi.".to_string(),
            recoverable: true,
            suggested_action: None,
            technical_details: Some("audit registry lock failed".to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        if let Some(state) = registry.get(&root) {
            return Ok(state.clone());
        }
        let last_hash = read_last_hash(&path);
        let state = Arc::new(Mutex::new(AuditFileState {
            path,
            last_hash,
            append_failures: 0,
        }));
        registry.insert(root.clone(), state.clone());
        Ok(state)
    }

    /// Appends an audit record. A failed append is reported to the caller;
    /// critical teacher decisions must not report fake success.
    pub fn append(
        &self,
        project_root: &Path,
        input: AuditEntryInput,
    ) -> Result<AuditRecord, AppError> {
        let state = self.file_state(project_root)?;
        let mut state = state.lock().map_err(|_| AppError {
            code: AppErrorCode::AuditWriteFailed,
            message: "Denetim kaydı kilitlenemedi.".to_string(),
            recoverable: true,
            suggested_action: None,
            technical_details: Some("audit state lock failed".to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;

        let previous_hash = state.last_hash.clone();
        let record = AuditRecord {
            schema_version: AUDIT_SCHEMA_VERSION,
            event_id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            correlation_id: input.correlation_id,
            operation: input.operation,
            actor_kind: input.actor_kind,
            project_id: input.project_id,
            entity_type: input.entity_type,
            entity_id: input.entity_id,
            previous_revision: input.previous_revision,
            next_revision: input.next_revision,
            safe_summary: input.safe_summary,
            previous_record_hash: previous_hash,
            record_hash: None,
        };
        let hash = record.compute_hash()?;
        let mut record = record;
        record.record_hash = Some(hash.clone());

        let mut line = serde_json::to_vec(&record).map_err(|error| AppError {
            code: AppErrorCode::AuditWriteFailed,
            message: "Denetim kaydı serileştirilemedi.".to_string(),
            recoverable: true,
            suggested_action: None,
            technical_details: Some(format!("audit serialize failed: {error}")),
            correlation_id: record.correlation_id.clone(),
        })?;
        line.push(b'\n');

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&state.path)
            .map_err(|error| {
                state.append_failures += 1;
                AppError {
                    code: AppErrorCode::AuditWriteFailed,
                    message: "Denetim kaydı yazılamadı.".to_string(),
                    recoverable: true,
                    suggested_action: Some(
                        "Disk alanını ve klasör izinlerini kontrol edin.".to_string(),
                    ),
                    technical_details: Some(format!("audit open failed: {error}")),
                    correlation_id: record.correlation_id.clone(),
                }
            })?;
        if let Err(error) = file.write_all(&line).and_then(|_| file.sync_all()) {
            state.append_failures += 1;
            return Err(AppError {
                code: AppErrorCode::AuditWriteFailed,
                message: "Denetim kaydı yazılamadı.".to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Disk alanını ve klasör izinlerini kontrol edin.".to_string(),
                ),
                technical_details: Some(format!("audit write failed: {error}")),
                correlation_id: record.correlation_id.clone(),
            });
        }

        state.last_hash = hash;
        Ok(record)
    }

    pub fn verify_chain(&self, project_root: &Path) -> Result<AuditChainReport, AppError> {
        let root = project_root
            .canonicalize()
            .unwrap_or_else(|_| project_root.to_path_buf());
        let path = Self::audit_path(&root);
        let mut records = load_records(&path)?;
        let mut expected_prev = GENESIS_HASH.to_string();
        let mut tamper_count = 0u64;
        let mut reasons = Vec::new();
        for record in &mut records {
            let computed = record.compute_hash()?;
            let recorded = record.record_hash.clone().unwrap_or_default();
            let hash_mismatch = computed != recorded;
            let link_mismatch = record.previous_record_hash != expected_prev;
            if hash_mismatch {
                tamper_count += 1;
                reasons.push(format!("record {} hash mismatch", record.event_id));
            }
            if link_mismatch {
                tamper_count += 1;
                reasons.push(format!("record {} broken chain link", record.event_id));
            }
            expected_prev = recorded;
        }
        Ok(AuditChainReport {
            record_count: records.len() as u64,
            chain_valid: tamper_count == 0,
            tamper_count,
            reasons,
        })
    }

    pub fn read_records(&self, project_root: &Path) -> Result<Vec<AuditRecord>, AppError> {
        let root = project_root
            .canonicalize()
            .unwrap_or_else(|_| project_root.to_path_buf());
        load_records(&Self::audit_path(&root))
    }

    pub fn counters(&self, project_root: &Path) -> AuditCounters {
        match self.verify_chain(project_root) {
            Ok(report) => {
                let state = self
                    .files
                    .lock()
                    .ok()
                    .and_then(|registry| {
                        registry
                            .get(
                                &project_root
                                    .canonicalize()
                                    .unwrap_or_else(|_| project_root.to_path_buf()),
                            )
                            .cloned()
                    })
                    .map(|state| state.lock().map(|s| s.append_failures).unwrap_or(0))
                    .unwrap_or(0);
                AuditCounters {
                    record_count: report.record_count,
                    chain_valid: report.chain_valid,
                    tamper_count: report.tamper_count,
                    append_failure_count: state,
                }
            }
            Err(_) => AuditCounters {
                chain_valid: false,
                ..Default::default()
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuditEntryInput {
    pub correlation_id: String,
    pub operation: String,
    pub actor_kind: String,
    pub project_id: Option<String>,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub previous_revision: Option<u64>,
    pub next_revision: Option<u64>,
    pub safe_summary: String,
}

impl AuditEntryInput {
    pub fn new(operation: &str, safe_summary: &str) -> Self {
        Self {
            correlation_id: Uuid::new_v4().to_string(),
            operation: operation.to_string(),
            actor_kind: "teacher".to_string(),
            project_id: None,
            entity_type: None,
            entity_id: None,
            previous_revision: None,
            next_revision: None,
            safe_summary: safe_summary.to_string(),
        }
    }

    pub fn project(mut self, project_id: &str) -> Self {
        self.project_id = Some(project_id.to_string());
        self
    }

    pub fn entity(mut self, entity_type: &str, entity_id: &str) -> Self {
        self.entity_type = Some(entity_type.to_string());
        self.entity_id = Some(entity_id.to_string());
        self
    }

    pub fn revisions(mut self, previous: Option<u64>, next: Option<u64>) -> Self {
        self.previous_revision = previous;
        self.next_revision = next;
        self
    }

    pub fn correlation(mut self, correlation_id: &str) -> Self {
        self.correlation_id = correlation_id.to_string();
        self
    }
}

fn read_last_hash(path: &Path) -> String {
    match load_records(path) {
        Ok(records) => records
            .last()
            .and_then(|record| record.record_hash.clone())
            .unwrap_or_else(|| GENESIS_HASH.to_string()),
        Err(_) => GENESIS_HASH.to_string(),
    }
}

fn load_records(path: &Path) -> Result<Vec<AuditRecord>, AppError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path).map_err(|error| AppError {
        code: AppErrorCode::AuditChainInvalid,
        message: "Denetim kaydı okunamadı.".to_string(),
        recoverable: true,
        suggested_action: Some("Tanılama raporunu inceleyin.".to_string()),
        technical_details: Some(format!("audit read failed: {error}")),
        correlation_id: Uuid::new_v4().to_string(),
    })?;
    let mut records = Vec::new();
    for (index, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let record: AuditRecord = serde_json::from_str(line).map_err(|error| AppError {
            code: AppErrorCode::AuditChainInvalid,
            message: "Denetim kaydında bozuk satır bulundu.".to_string(),
            recoverable: false,
            suggested_action: Some("Tanılama ekranından denetim zincirini doğrulayın.".to_string()),
            technical_details: Some(format!("audit line {} parse failed: {error}", index + 1)),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        records.push(record);
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("rubrika-audit-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("logs")).unwrap();
        root
    }

    fn sample_input(operation: &str, summary: &str) -> AuditEntryInput {
        AuditEntryInput::new(operation, summary)
            .project("proj-1")
            .entity("assessment", "assess-1")
    }

    #[test]
    fn chain_is_valid_and_sentinel_never_enters_audit_payload() {
        let root = temp_root();
        let service = AuditService::new();
        for index in 0..5 {
            let record = service
                .append(&root, sample_input(&format!("op_{index}"), "güvenli özet"))
                .expect("append");
            assert!(record.record_hash.is_some());
            assert!(!record.safe_summary.contains("STUDENT_SECRET_9f4a"));
        }
        let report = service.verify_chain(&root).unwrap();
        assert!(report.chain_valid);
        assert_eq!(report.record_count, 5);
        assert_eq!(report.tamper_count, 0);

        // The audit file itself never contains the sentinel.
        let raw = std::fs::read_to_string(AuditService::audit_path(&root)).unwrap();
        assert!(!raw.contains("STUDENT_SECRET_9f4a"));
        assert!(!raw.contains("OCR_SECRET_17ce"));
        assert!(!raw.contains("PROMPT_SECRET_a821"));
        assert!(!raw.contains("MODEL_SECRET_47bf"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn tampering_middle_record_breaks_chain() {
        let root = temp_root();
        let service = AuditService::new();
        for index in 0..4 {
            service
                .append(&root, sample_input(&format!("op_{index}"), "özet"))
                .unwrap();
        }
        let path = AuditService::audit_path(&root);
        let mut records = service.read_records(&root).unwrap();
        records[1].safe_summary = "DEĞİŞTİRİLDİ".to_string();
        let mut content = String::new();
        for record in records {
            content.push_str(&serde_json::to_string(&record).unwrap());
            content.push('\n');
        }
        std::fs::write(&path, content).unwrap();

        let report = service.verify_chain(&root).unwrap();
        assert!(!report.chain_valid);
        assert!(report.tamper_count >= 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn deleting_middle_record_breaks_chain() {
        let root = temp_root();
        let service = AuditService::new();
        for index in 0..4 {
            service
                .append(&root, sample_input(&format!("op_{index}"), "özet"))
                .unwrap();
        }
        let path = AuditService::audit_path(&root);
        let mut records = service.read_records(&root).unwrap();
        records.remove(2);
        let mut content = String::new();
        for record in records {
            content.push_str(&serde_json::to_string(&record).unwrap());
            content.push('\n');
        }
        std::fs::write(&path, content).unwrap();

        let report = service.verify_chain(&root).unwrap();
        assert!(!report.chain_valid);
        assert!(report.tamper_count >= 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn concurrent_appends_lose_no_records_and_keep_chain() {
        let root = temp_root();
        let service = Arc::new(AuditService::new());
        let mut handles = Vec::new();
        for thread_index in 0..8 {
            let service = service.clone();
            let root = root.clone();
            handles.push(std::thread::spawn(move || {
                for index in 0..25 {
                    service
                        .append(
                            &root,
                            AuditEntryInput::new(
                                &format!("t{thread_index}_op{index}"),
                                "eşzamanlı özet",
                            )
                            .project("proj-1"),
                        )
                        .expect("concurrent append");
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
        let report = service.verify_chain(&root).unwrap();
        assert_eq!(report.record_count, 8 * 25);
        assert!(report.chain_valid);
        assert_eq!(report.tamper_count, 0);
        let _ = std::fs::remove_dir_all(&root);
    }
}

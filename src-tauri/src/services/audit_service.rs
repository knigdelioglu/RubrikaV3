use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::domain::errors::{AppError, AppErrorCode};
use crate::platform::project_write_lease::acquire_or_share;
use crate::services::transaction_journal;

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    /// Structured recovery/diagnostic context. It is intentionally optional
    /// so historical records remain readable without being rewritten.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
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
            "transaction_id": self.transaction_id,
            "metadata": self.metadata,
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

    pub fn compute_hash(&self) -> Result<String, AppError> {
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
    pub project_revision_divergence_count: u64,
    pub active_revision_divergence_count: u64,
    pub first_invalid_line: Option<u64>,
    pub first_invalid_record_id: Option<String>,
    pub first_invalid_previous_hash: Option<String>,
    pub first_invalid_computed_hash: Option<String>,
    pub first_invalid_recorded_hash: Option<String>,
    pub last_valid_record_hash: String,
    pub last_audit_revision: Option<u64>,
    pub duplicate_revision_count: u64,
    pub missing_revision_count: u64,
    pub original_audit_status: String,
    pub active_audit_status: String,
    pub historical_recovery_anchor_status: String,
    pub classifications: Vec<String>,
}

impl Default for AuditChainReport {
    fn default() -> Self {
        Self {
            record_count: 0,
            chain_valid: false,
            tamper_count: 0,
            reasons: Vec::new(),
            project_revision_divergence_count: 0,
            active_revision_divergence_count: 0,
            first_invalid_line: None,
            first_invalid_record_id: None,
            first_invalid_previous_hash: None,
            first_invalid_computed_hash: None,
            first_invalid_recorded_hash: None,
            last_valid_record_hash: GENESIS_HASH.to_string(),
            last_audit_revision: None,
            duplicate_revision_count: 0,
            missing_revision_count: 0,
            original_audit_status: "INVALID_UNRECOVERED".to_string(),
            active_audit_status: "INVALID_UNRECOVERED".to_string(),
            historical_recovery_anchor_status: "NOT_PRESENT".to_string(),
            classifications: Vec::new(),
        }
    }
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
        // Audit is a persistent project mutation too. The per-root state
        // mutex is acquired first so concurrent threads in one process
        // serialize the short OS-lease acquisition instead of racing and
        // reporting ProjectAlreadyOpen to one another.
        let _project_lease = acquire_or_share(project_root)?;
        let canonical_root = project_root.canonicalize().map_err(|error| AppError {
            code: AppErrorCode::AuditWriteFailed,
            message: "Proje denetim klasörü çözümlenemedi.".to_string(),
            recoverable: true,
            suggested_action: Some("Proje klasörü izinlerini kontrol edin.".to_string()),
            technical_details: Some(format!("audit root canonicalize failed: {error}")),
            correlation_id: input.correlation_id.clone(),
        })?;
        std::fs::create_dir_all(canonical_root.join("logs")).map_err(|error| AppError {
            code: AppErrorCode::AuditWriteFailed,
            message: "Denetim klasörü oluşturulamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("Disk alanını ve klasör izinlerini kontrol edin.".to_string()),
            technical_details: Some(format!("audit directory create failed: {error}")),
            correlation_id: input.correlation_id.clone(),
        })?;

        let previous_hash = state.last_hash.clone();
        let record = AuditRecord {
            schema_version: AUDIT_SCHEMA_VERSION,
            event_id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            correlation_id: input.correlation_id,
            transaction_id: input.transaction_id,
            metadata: input.metadata,
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

    /// Writes application-level consent events when no project is open. The
    /// record deliberately contains only policy/profile metadata, never
    /// student content.
    pub fn append_application_event(
        &self,
        input: AuditEntryInput,
    ) -> Result<AuditRecord, AppError> {
        let root = crate::platform::paths::app_log_dir().join("privacy");
        std::fs::create_dir_all(&root).map_err(|error| AppError {
            code: AppErrorCode::AuditWriteFailed,
            message: "Gizlilik denetim klasörü oluşturulamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("Disk alanını ve klasör izinlerini kontrol edin.".to_string()),
            technical_details: Some(format!("privacy audit directory create failed: {error}")),
            correlation_id: input.correlation_id.clone(),
        })?;
        self.append(&root, input)
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
        let mut first_invalid_line = None;
        let mut first_invalid_record_id = None;
        let mut first_invalid_previous_hash = None;
        let mut first_invalid_computed_hash = None;
        let mut first_invalid_recorded_hash = None;
        let mut last_valid_record_hash = GENESIS_HASH.to_string();
        for (index, record) in records.iter_mut().enumerate() {
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
            if (hash_mismatch || link_mismatch) && first_invalid_line.is_none() {
                first_invalid_line = Some((index + 1) as u64);
                first_invalid_record_id = Some(record.event_id.clone());
                first_invalid_previous_hash = Some(record.previous_record_hash.clone());
                first_invalid_computed_hash = Some(computed.clone());
                first_invalid_recorded_hash = Some(recorded.clone());
            }
            if !hash_mismatch && !link_mismatch {
                last_valid_record_hash = recorded.clone();
            }
            expected_prev = recorded;
        }
        let legacy_format = records.iter().any(|record| {
            record.transaction_id.is_none()
                && record.metadata.is_none()
                && record.previous_revision.is_none()
                && record.next_revision.is_none()
        });
        let mut classifications = Vec::new();
        if legacy_format {
            classifications.push("legacy_audit_format".to_string());
        }
        if first_invalid_line.is_some() {
            classifications.push("audit_hash_or_link_invalid".to_string());
        }
        let chain_valid = tamper_count == 0;
        Ok(AuditChainReport {
            record_count: records.len() as u64,
            chain_valid,
            tamper_count,
            reasons,
            project_revision_divergence_count: 0,
            active_revision_divergence_count: 0,
            first_invalid_line,
            first_invalid_record_id,
            first_invalid_previous_hash,
            first_invalid_computed_hash,
            first_invalid_recorded_hash,
            last_valid_record_hash,
            last_audit_revision: None,
            duplicate_revision_count: 0,
            missing_revision_count: 0,
            original_audit_status: if chain_valid {
                "VALID".to_string()
            } else {
                "INVALID_UNRECOVERED".to_string()
            },
            active_audit_status: if chain_valid {
                "VALID".to_string()
            } else {
                "INVALID_UNRECOVERED".to_string()
            },
            historical_recovery_anchor_status: "NOT_PRESENT".to_string(),
            classifications,
        })
    }

    /// Verifies the hash chain and the revision transitions recorded for one
    /// canonical project. Legacy audit records without revision metadata are
    /// retained but never counted as proof of a current project revision.
    pub fn verify_chain_against_project(
        &self,
        project_root: &Path,
        project: &crate::domain::project::Project,
    ) -> Result<AuditChainReport, AppError> {
        let mut report = self.verify_chain(project_root)?;
        let records = self.read_records(project_root)?;
        let anchor = records.iter().find(|record| {
            record.operation == "recovery_anchor"
                && record
                    .metadata
                    .as_ref()
                    .and_then(|value| value.get("kind"))
                    .and_then(serde_json::Value::as_str)
                    == Some("RecoveryAnchor")
        });
        let anchor_valid = anchor
            .map(|record| verify_recovery_anchor(project_root, record, project))
            .unwrap_or(false);
        if anchor.is_some() {
            if anchor_valid {
                report.historical_recovery_anchor_status = "VALID".to_string();
                report.original_audit_status = "HISTORICAL_INVALID_RECOVERY_ANCHORED".to_string();
                report
                    .classifications
                    .push("historical_invalid_recovery_anchored".to_string());
            } else {
                report.historical_recovery_anchor_status = "INVALID".to_string();
                report.active_audit_status = "ACTIVE_CHAIN_INVALID".to_string();
                report
                    .reasons
                    .push("recovery anchor could not be verified".to_string());
                report.tamper_count = report.tamper_count.saturating_add(1);
                report.chain_valid = false;
            }
        }

        let anchor_revision = anchor
            .and_then(|record| record.metadata.as_ref())
            .and_then(|metadata| metadata.get("projectRevision"))
            .and_then(serde_json::Value::as_u64);
        let mut previous: Option<u64> = anchor_revision;
        let mut seen_revisions = std::collections::BTreeSet::new();
        for record in records
            .iter()
            .filter(|record| record.project_id.as_deref() == Some(project.id.as_str()))
            .filter(|record| record.operation != "recovery_anchor")
        {
            if is_critical_operation(&record.operation) && record.transaction_id.is_none() {
                report.project_revision_divergence_count += 1;
                report.reasons.push(format!(
                    "critical audit record has no transaction id: {}",
                    record.operation
                ));
            }
            let (Some(record_previous), Some(record_next)) =
                (record.previous_revision, record.next_revision)
            else {
                continue;
            };
            if !seen_revisions.insert(record_next) {
                report.duplicate_revision_count += 1;
                report
                    .reasons
                    .push(format!("duplicate audit revision: {record_next}"));
            }
            if previous.is_some_and(|value| value != record_previous)
                || record_next != record_previous.saturating_add(1)
            {
                report.project_revision_divergence_count += 1;
                report.reasons.push(format!(
                    "audit revision transition mismatch: {} {} -> {}",
                    record.operation, record_previous, record_next
                ));
            }
            if let Some(last) = previous {
                if record_previous > last.saturating_add(1) {
                    report.missing_revision_count = report
                        .missing_revision_count
                        .saturating_add(record_previous - last - 1);
                }
            }
            previous = Some(record_next);
        }
        if let Some(last) = previous {
            if last != project.storage_revision {
                report.project_revision_divergence_count += 1;
                report.reasons.push(format!(
                    "audit/project revision divergence: audit={last}; project={}",
                    project.storage_revision
                ));
            }
        } else if project.storage_revision > 0 && anchor_revision.is_none() {
            report.project_revision_divergence_count += 1;
            report.reasons.push(format!(
                "project has revision {} but no matching audit revision",
                project.storage_revision
            ));
        }
        report.active_revision_divergence_count = report.project_revision_divergence_count;
        report.chain_valid = report.chain_valid
            && report.project_revision_divergence_count == 0
            && report.duplicate_revision_count == 0
            && report.missing_revision_count == 0;
        if report.chain_valid {
            report.active_audit_status = "VALID".to_string();
        } else if anchor.is_some() && anchor_valid {
            report.active_audit_status = "ACTIVE_CHAIN_INVALID".to_string();
        }
        report.last_audit_revision = previous;
        Ok(report)
    }

    /// Wraps the post-commit audit boundary in a durable journal. If the
    /// audit append or completion marker fails, the caller receives an
    /// explicit incomplete result and preflight can block future writing.
    pub fn append_transactionally(
        &self,
        project_root: &Path,
        mut input: AuditEntryInput,
        expected_revision: Option<u64>,
        target_revision: Option<u64>,
    ) -> Result<AuditRecord, AppError> {
        let project_id = input.project_id.clone().ok_or_else(|| AppError {
            code: AppErrorCode::AuditCommitIncomplete,
            message: "Denetim işlemi proje kimliği olmadan başlatılamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("İşlemi yeniden deneyin.".to_string()),
            technical_details: None,
            correlation_id: input.correlation_id.clone(),
        })?;
        let transaction = transaction_journal::begin(
            project_root,
            &project_id,
            &input.operation,
            &input.correlation_id,
            expected_revision,
            target_revision,
        )
        .map_err(|error| incomplete_audit_error(&input.correlation_id, error))?;
        input.transaction_id = Some(transaction.transaction_id.clone());
        input.previous_revision = expected_revision;
        input.next_revision = target_revision;
        let record = match self.append(project_root, input) {
            Ok(record) => record,
            Err(error) => {
                let _ = transaction_journal::update(
                    project_root,
                    &transaction.transaction_id,
                    "audit_missing",
                );
                return Err(incomplete_audit_error(&transaction.correlation_id, error));
            }
        };
        if let Err(error) =
            transaction_journal::update(project_root, &transaction.transaction_id, "complete")
        {
            return Err(incomplete_audit_error(&transaction.correlation_id, error));
        }
        Ok(record)
    }

    /// Repairs the specific legacy shape produced when the first project
    /// mutation advanced storage revision 0 -> 1 without appending its audit
    /// record. This is intentionally narrow: it refuses tampered chains,
    /// multiple gaps, and any project that is not exactly one revision ahead
    /// of its valid audit history.
    pub fn repair_missing_initial_revision(
        &self,
        project_root: &Path,
    ) -> Result<AuditRecord, AppError> {
        let root = project_root.canonicalize().map_err(|error| AppError {
            code: AppErrorCode::ProjectLoadFailed,
            message: "Proje klasörü çözümlenemedi.".to_string(),
            recoverable: true,
            suggested_action: Some("Proje klasörünü ve izinlerini kontrol edin.".to_string()),
            technical_details: Some(format!("audit repair root canonicalize failed: {error}")),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        let project_content =
            std::fs::read_to_string(root.join("project.json")).map_err(|error| AppError {
                code: AppErrorCode::ProjectLoadFailed,
                message: "Proje verisi okunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Proje tanılamasını çalıştırıp tekrar deneyin.".to_string()),
                technical_details: Some(format!("audit repair project read failed: {error}")),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        let project: crate::domain::project::Project = serde_json::from_str(&project_content)
            .map_err(|error| AppError {
                code: AppErrorCode::ProjectLoadFailed,
                message: "Proje verisi okunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Proje tanılamasını çalıştırıp tekrar deneyin.".to_string()),
                technical_details: Some(format!("audit repair project parse failed: {error}")),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        let chain = self.verify_chain(&root)?;
        let records = self.read_records(&root)?;
        let project_records = records
            .iter()
            .filter(|record| record.project_id.as_deref() == Some(project.id.as_str()))
            .collect::<Vec<_>>();
        let has_initial_revision = project_records
            .iter()
            .any(|record| record.next_revision == Some(0));
        let has_target_revision = project_records
            .iter()
            .any(|record| record.next_revision == Some(1));
        let can_repair = chain.chain_valid
            && project.storage_revision == 1
            && has_initial_revision
            && !has_target_revision
            && project_records
                .iter()
                .all(|record| record.next_revision.is_none() || record.next_revision == Some(0));
        if !can_repair {
            return Err(AppError {
                code: AppErrorCode::AuditChainInvalid,
                message: "Denetim zinciri otomatik olarak hizalanamadı.".to_string(),
                recoverable: false,
                suggested_action: Some(
                    "Tanılama raporunu inceleyin; bu onarım yalnızca tek eksik başlangıç revizyonunda kullanılabilir."
                        .to_string(),
                ),
                technical_details: Some(format!(
                    "repair precondition failed: storage_revision={}, chain_valid={}, initial_revision={}, target_revision={}, project_record_count={}",
                    project.storage_revision,
                    chain.chain_valid,
                    has_initial_revision,
                    has_target_revision,
                    project_records.len()
                )),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        let incomplete_repairs = crate::services::transaction_journal::list(&root)?
            .into_iter()
            .filter(|record| {
                record.project_id == project.id
                    && record.operation == "audit_revision_repaired"
                    && record.expected_revision == Some(0)
                    && record.target_revision == Some(1)
                    && record.status == "audit_missing"
            })
            .map(|record| record.transaction_id)
            .collect::<Vec<_>>();
        let repaired = self.append_transactionally(
            &root,
            AuditEntryInput::new(
                "audit_revision_repaired",
                "Eksik başlangıç denetim revizyonu hizalandı.",
            )
            .project(&project.id)
            .metadata(serde_json::json!({
                "kind": "missing_initial_revision",
                "repairedRevision": 1,
            })),
            Some(0),
            Some(1),
        )?;
        for transaction_id in incomplete_repairs {
            crate::services::transaction_journal::update(&root, &transaction_id, "aborted")?;
        }
        Ok(repaired)
    }

    /// Repairs the specific legacy shape where the project is exactly one
    /// revision ahead of an otherwise valid audit prefix. This is intentionally
    /// narrower than general recovery: it refuses tampered chains, multiple
    /// missing revisions, and any non-terminal divergence.
    pub fn repair_missing_latest_revision(
        &self,
        project_root: &Path,
    ) -> Result<AuditRecord, AppError> {
        let root = project_root.canonicalize().map_err(|error| AppError {
            code: AppErrorCode::ProjectLoadFailed,
            message: "Proje klasörü çözümlenemedi.".to_string(),
            recoverable: true,
            suggested_action: Some("Proje klasörünü ve izinlerini kontrol edin.".to_string()),
            technical_details: Some(format!(
                "audit latest repair root canonicalize failed: {error}"
            )),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        let project_content =
            std::fs::read_to_string(root.join("project.json")).map_err(|error| AppError {
                code: AppErrorCode::ProjectLoadFailed,
                message: "Proje verisi okunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Proje tanılamasını çalıştırıp tekrar deneyin.".to_string()),
                technical_details: Some(format!(
                    "audit latest repair project read failed: {error}"
                )),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        let project: crate::domain::project::Project = serde_json::from_str(&project_content)
            .map_err(|error| AppError {
                code: AppErrorCode::ProjectLoadFailed,
                message: "Proje verisi okunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Proje tanılamasını çalıştırıp tekrar deneyin.".to_string()),
                technical_details: Some(format!(
                    "audit latest repair project parse failed: {error}"
                )),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        let chain = self.verify_chain_against_project(&root, &project)?;
        let records = self.read_records(&root)?;
        let project_records = records
            .iter()
            .filter(|record| record.project_id.as_deref() == Some(project.id.as_str()))
            .collect::<Vec<_>>();
        let expected_previous = project.storage_revision.checked_sub(1);
        let has_initial_revision = project_records
            .iter()
            .any(|record| record.next_revision == Some(0));
        let can_repair = chain.tamper_count == 0
            && chain.duplicate_revision_count == 0
            && chain.missing_revision_count == 0
            && chain.project_revision_divergence_count == 1
            && has_initial_revision
            && expected_previous.is_some()
            && chain.last_audit_revision == expected_previous
            && project_records.iter().all(|record| {
                record
                    .next_revision
                    .is_some_and(|revision| revision <= expected_previous.unwrap_or_default())
            });
        if !can_repair {
            return Err(AppError {
                code: AppErrorCode::AuditChainInvalid,
                message: "Denetim zincirinin son revizyonu otomatik olarak hizalanamadı.".to_string(),
                recoverable: false,
                suggested_action: Some(
                    "Tanılama raporunu inceleyin; bu onarım yalnızca tek eksik son revizyonda kullanılabilir."
                        .to_string(),
                ),
                technical_details: Some(format!(
                    "latest repair precondition failed: storage_revision={}, last_audit_revision={:?}, chain_tamper_count={}, project_revision_divergence_count={}, duplicate_revision_count={}, missing_revision_count={}",
                    project.storage_revision,
                    chain.last_audit_revision,
                    chain.tamper_count,
                    chain.project_revision_divergence_count,
                    chain.duplicate_revision_count,
                    chain.missing_revision_count
                )),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        let previous_revision = expected_previous.ok_or_else(|| AppError {
            code: AppErrorCode::AuditChainInvalid,
            message: "Denetim zincirinin son revizyonu çözümlenemedi.".to_string(),
            recoverable: false,
            suggested_action: Some("Tanılama raporunu inceleyin.".to_string()),
            technical_details: None,
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        let incomplete_repairs = crate::services::transaction_journal::list(&root)?
            .into_iter()
            .filter(|record| {
                record.project_id == project.id
                    && record.operation == "audit_revision_repaired"
                    && record.expected_revision == Some(previous_revision)
                    && record.target_revision == Some(project.storage_revision)
                    && record.status == "audit_missing"
            })
            .map(|record| record.transaction_id)
            .collect::<Vec<_>>();
        let repaired = self.append_transactionally(
            &root,
            AuditEntryInput::new(
                "audit_revision_repaired",
                "Eksik son denetim revizyonu hizalandı.",
            )
            .project(&project.id)
            .metadata(serde_json::json!({
                "kind": "missing_latest_revision",
                "repairedRevision": project.storage_revision,
            })),
            Some(previous_revision),
            Some(project.storage_revision),
        )?;
        for transaction_id in incomplete_repairs {
            crate::services::transaction_journal::update(&root, &transaction_id, "aborted")?;
        }
        Ok(repaired)
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
    pub transaction_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
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
            transaction_id: None,
            metadata: None,
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

    pub fn transaction(mut self, transaction_id: &str) -> Self {
        self.transaction_id = Some(transaction_id.to_string());
        self
    }

    pub fn metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

fn verify_recovery_anchor(
    project_root: &Path,
    record: &AuditRecord,
    project: &crate::domain::project::Project,
) -> bool {
    let Some(metadata) = record.metadata.as_ref() else {
        return false;
    };
    let historical_path = project_root
        .join("logs")
        .join("recovery")
        .join("historical")
        .join("audit.jsonl");
    let manifest_path = project_root
        .join("logs")
        .join("recovery")
        .join("recovery-manifest.json");
    let original_hash = metadata
        .get("originalAuditSha256")
        .and_then(serde_json::Value::as_str);
    let original_size = metadata
        .get("originalAuditSize")
        .and_then(serde_json::Value::as_u64);
    let expected_project_fingerprint = metadata
        .get("projectFingerprint")
        .and_then(serde_json::Value::as_str);
    let expected_manifest_hash = metadata
        .get("recoveryManifestSha256")
        .and_then(serde_json::Value::as_str);
    let Some(original_hash) = original_hash else {
        return false;
    };
    let Some(original_size) = original_size else {
        return false;
    };
    let Ok(historical_metadata) = std::fs::metadata(&historical_path) else {
        return false;
    };
    if historical_metadata.len() != original_size {
        return false;
    }
    let Ok(historical_hash) = sha256_file(&historical_path) else {
        return false;
    };
    if historical_hash != original_hash {
        return false;
    }
    if let Some(expected_manifest_hash) = expected_manifest_hash {
        let Ok(manifest_hash) = sha256_file(&manifest_path) else {
            return false;
        };
        if manifest_hash != expected_manifest_hash {
            return false;
        }
    }
    if let Some(expected_project_fingerprint) = expected_project_fingerprint {
        let Ok(project_hash) = sha256_file(&project_root.join("project.json")) else {
            return false;
        };
        if project_hash != expected_project_fingerprint {
            return false;
        }
    }
    metadata
        .get("projectRevision")
        .and_then(serde_json::Value::as_u64)
        .is_some_and(|revision| revision == project.storage_revision)
}

fn sha256_file(path: &Path) -> Result<String, std::io::Error> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = std::io::Read::read(&mut file, &mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn incomplete_audit_error(correlation_id: &str, error: AppError) -> AppError {
    AppError {
        code: AppErrorCode::AuditCommitIncomplete,
        message:
            "Değişiklik kaydedildi ancak denetim işlemi tamamlanamadı; başarı bildirimi verilmedi."
                .to_string(),
        recoverable: true,
        suggested_action: Some(
            "Proje yazmaya kapatılıp preflight ile kontrol edilmeli.".to_string(),
        ),
        technical_details: Some(format!("audit transaction incomplete: {error}")),
        correlation_id: correlation_id.to_string(),
    }
}

fn is_critical_operation(operation: &str) -> bool {
    matches!(
        operation,
        "migration"
            | "teacher_ocr_approval"
            | "ocr_generation_accepted"
            | "ocr_generation_rejected"
            | "rubric_confirmed"
            | "rubric_confirmed_all"
            | "question_text_confirmed"
            | "question_text_confirmed_all"
            | "exam_package_frozen"
            | "exam_package_invalidated"
            | "scoring_record_updated"
            | "scoring_override_approved"
            | "document_deleted"
            | "submission_deleted"
            | "speaking_attempt_approved"
            | "generation_gc"
            | "project_restored"
    )
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
    use crate::services::project_store::ProjectStore;

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

    #[test]
    fn repairs_only_the_single_missing_initial_revision() {
        let root = std::env::temp_dir().join(format!("rubrika-audit-repair-{}", Uuid::new_v4()));
        let store = ProjectStore::new();
        let project = store
            .create_project(
                "Repair fixture".to_string(),
                root.to_string_lossy().to_string(),
            )
            .expect("project fixture");
        let service = AuditService::new();
        service
            .append_transactionally(
                Path::new(&project.root_path),
                AuditEntryInput::new("project_created", "Yeni proje oluşturuldu.")
                    .project(&project.id),
                None,
                Some(0),
            )
            .expect("initial audit");
        store
            .update_course_info(
                project.id.clone(),
                "2026-2027".to_string(),
                "tde".to_string(),
                "Türk Dili ve Edebiyatı".to_string(),
                None,
            )
            .expect("first mutation");
        drop(store);

        service
            .repair_missing_initial_revision(Path::new(&project.root_path))
            .expect("audit repair");
        let repaired = serde_json::from_str::<crate::domain::project::Project>(
            &std::fs::read_to_string(root.join("project.json")).expect("project after repair"),
        )
        .expect("project json");
        let report = service
            .verify_chain_against_project(Path::new(&project.root_path), &repaired)
            .expect("verify repaired audit");
        assert!(report.chain_valid);
        assert_eq!(report.last_audit_revision, Some(1));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn repairs_only_a_single_missing_latest_revision() {
        let root =
            std::env::temp_dir().join(format!("rubrika-audit-latest-repair-{}", Uuid::new_v4()));
        let store = ProjectStore::new();
        let project = store
            .create_project(
                "Latest repair fixture".to_string(),
                root.to_string_lossy().to_string(),
            )
            .expect("project fixture");
        let service = AuditService::new();
        service
            .append_transactionally(
                Path::new(&project.root_path),
                AuditEntryInput::new("project_created", "Yeni proje oluşturuldu.")
                    .project(&project.id),
                None,
                Some(0),
            )
            .expect("initial audit");
        store
            .update_course_info(
                project.id.clone(),
                "2026-2027".to_string(),
                "tde".to_string(),
                "Türk Dili ve Edebiyatı".to_string(),
                None,
            )
            .expect("first mutation");
        service
            .append_transactionally(
                Path::new(&project.root_path),
                AuditEntryInput::new("course_info_updated", "Ders bilgileri güncellendi.")
                    .project(&project.id),
                Some(0),
                Some(1),
            )
            .expect("first mutation audit");
        store
            .update_course_info(
                project.id.clone(),
                "2026-2027".to_string(),
                "tde".to_string(),
                "Türk Dili ve Edebiyatı (güncel)".to_string(),
                None,
            )
            .expect("missing-audit mutation");
        drop(store);

        service
            .repair_missing_latest_revision(Path::new(&project.root_path))
            .expect("latest audit repair");
        let repaired = serde_json::from_str::<crate::domain::project::Project>(
            &std::fs::read_to_string(root.join("project.json")).expect("project after repair"),
        )
        .expect("project json");
        let report = service
            .verify_chain_against_project(Path::new(&project.root_path), &repaired)
            .expect("verify repaired audit");
        assert!(report.chain_valid);
        assert_eq!(report.last_audit_revision, Some(2));
        let _ = std::fs::remove_dir_all(&root);
    }
}

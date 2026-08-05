use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex, OnceLock, Weak,
};
use std::time::SystemTime;
use uuid::Uuid;

use crate::domain::document::DocumentRole;
use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::project::Project;
use crate::domain::question::is_question_text_ready;
use crate::domain::rubric::RubricStatus;
use crate::domain::school_class::normalize_school_class_name;
use crate::domain::workflow::{WorkflowSnapshot, WorkflowStage};
use crate::platform::project_paths::TrustedProjectRoot;
use crate::platform::project_write_lease::{acquire_or_share, ProjectWriteLease};
use crate::services::transaction_journal;
use crate::services::workflow_engine;
use serde_json::{Map, Value};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStatusSummary {
    pub has_exam_source: bool,
    pub has_answer_key_or_rubric: bool,
    pub has_student_scan: bool,
    pub question_text_coverage: Option<String>,
    pub rubric_coverage: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectListItem {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub question_count: Option<u32>,
    pub document_roles: Vec<String>,
    pub status_summary: ProjectStatusSummary,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedProject {
    pub path: String,
    pub reason: String,
    pub technical_details: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListProjectsOutput {
    pub projects: Vec<ProjectListItem>,
    pub warnings: Vec<String>,
    pub skipped_projects: Vec<SkippedProject>,
}

/// Opening a project is explicit because inspection, normal writing and
/// migration have different durability contracts. In particular, neither
/// inspection nor a normal open may silently persist a migration or recovery.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectOpenMode {
    InspectReadOnly,
    #[default]
    OpenWithoutMigration,
    MigrateWithVerifiedBackup,
}

/// The canonical read result used by persistence-aware services. The
/// `trusted_root` is deliberately backend-only; commands expose the project
/// and revision metadata without allowing callers to choose a write root.
#[derive(Debug, Clone)]
pub struct ProjectSnapshot {
    pub project: Project,
    pub revision: u64,
    pub content_fingerprint: String,
    pub trusted_root: TrustedProjectRoot,
}

#[derive(Debug, Clone)]
pub struct MutationOptions {
    pub expected_revision: Option<u64>,
    pub expected_fingerprint: Option<String>,
    pub operation: String,
    pub correlation_id: String,
}

impl MutationOptions {
    pub fn new(operation: impl Into<String>) -> Self {
        Self {
            expected_revision: None,
            expected_fingerprint: None,
            operation: operation.into(),
            correlation_id: Uuid::new_v4().to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MutationContext {
    pub project_id: String,
    pub current_revision: u64,
    pub current_fingerprint: String,
    pub trusted_root: TrustedProjectRoot,
    pub operation: String,
    pub correlation_id: String,
}

#[derive(Debug)]
pub struct MutationOutput<T> {
    pub result: T,
    pub snapshot: ProjectSnapshot,
}

/// A long-running job uses the same transactional primitive, but its caller
/// can distinguish a stale candidate from a successful commit.
#[derive(Debug)]
pub enum JobCommitResult<T> {
    Applied(Box<MutationOutput<T>>),
    Stale { reason: String },
    Conflict(AppError),
    EntityMissing,
    Rejected(AppError),
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistenceDiagnostics {
    pub storage_revision: u64,
    pub project_fingerprint_status: String,
    pub stale_job_result_count: u64,
    pub mutation_conflict_count: u64,
    pub external_modification_detected: bool,
    pub legacy_project_without_revision: bool,
}

static PROJECT_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Weak<Mutex<()>>>>> = OnceLock::new();

#[derive(Clone)]
pub struct ProjectStore {
    current_project: Arc<Mutex<Option<Project>>>,
    trusted_root: Arc<Mutex<Option<TrustedProjectRoot>>>,
    write_lease: Arc<Mutex<Option<Arc<ProjectWriteLease>>>>,
    current_fingerprint: Arc<Mutex<Option<String>>>,
    revision_history: Arc<Mutex<HashMap<(String, u64), Project>>>,
    stale_job_result_count: Arc<AtomicU64>,
    mutation_conflict_count: Arc<AtomicU64>,
    external_modification_detected: Arc<AtomicBool>,
    legacy_project_without_revision: Arc<AtomicBool>,
}

impl Default for ProjectStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectStore {
    pub fn new() -> Self {
        Self {
            current_project: Arc::new(Mutex::new(None)),
            trusted_root: Arc::new(Mutex::new(None)),
            write_lease: Arc::new(Mutex::new(None)),
            current_fingerprint: Arc::new(Mutex::new(None)),
            revision_history: Arc::new(Mutex::new(HashMap::new())),
            stale_job_result_count: Arc::new(AtomicU64::new(0)),
            mutation_conflict_count: Arc::new(AtomicU64::new(0)),
            external_modification_detected: Arc::new(AtomicBool::new(false)),
            legacy_project_without_revision: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn create_project(&self, name: String, root_path: String) -> Result<Project, AppError> {
        self.create_project_with_setup(name, root_path, None, None, None)
    }

    pub fn update_course_info(
        &self,
        project_id: String,
        academic_year_id: String,
        course_id: String,
        course_name: String,
        expected_revision: Option<u64>,
    ) -> Result<Project, AppError> {
        let academic_year_id = academic_year_id.trim().to_string();
        let course_id = course_id.trim().to_string();
        let course_name = course_name.trim().to_string();

        if academic_year_id.is_empty() || course_id.is_empty() || course_name.is_empty() {
            return Err(AppError {
                code: AppErrorCode::ProjectSaveFailed,
                message: "Ders kodu, ders adı ve eğitim yılı boş olamaz.".to_string(),
                recoverable: true,
                suggested_action: Some("Geçerli ders bilgileri girip tekrar deneyin.".to_string()),
                technical_details: Some("empty course metadata fields".to_string()),
                correlation_id: uuid::Uuid::new_v4().to_string(),
            });
        }

        let output = self.mutate(
            &project_id,
            MutationOptions {
                expected_revision,
                expected_fingerprint: None,
                operation: "update_course_info".to_string(),
                correlation_id: Uuid::new_v4().to_string(),
            },
            move |project, _context| {
                project.academic_year_id = Some(academic_year_id);
                project.course_id = Some(course_id);
                project.course_name = Some(course_name);
                Ok(project.clone())
            },
        )?;
        Ok(output.result)
    }

    pub fn create_project_with_setup(
        &self,
        name: String,
        root_path: String,
        academic_year_id: Option<String>,
        course_id: Option<String>,
        course_name: Option<String>,
    ) -> Result<Project, AppError> {
        let trusted_root = TrustedProjectRoot::for_create(Path::new(&root_path))?;
        let root = trusted_root.root().to_path_buf();
        let root_was_created = if root.exists() {
            let project_file = root.join("project.json");
            if project_file.exists() {
                return Err(AppError {
                    code: AppErrorCode::ProjectAlreadyExists,
                    message: "Bu klasörde zaten bir Rubrika projesi bulunuyor.".to_string(),
                    recoverable: true,
                    suggested_action: Some("Yeni proje için boş bir klasör seçin.".to_string()),
                    technical_details: Some(project_file.to_string_lossy().to_string()),
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
            if std::fs::read_dir(&root)
                .map_err(|error| AppError {
                    code: AppErrorCode::ProjectSaveFailed,
                    message: "Seçilen proje klasörü okunamadı.".to_string(),
                    recoverable: true,
                    suggested_action: Some("Klasör izinlerini kontrol edin.".to_string()),
                    technical_details: Some(error.to_string()),
                    correlation_id: Uuid::new_v4().to_string(),
                })?
                .next()
                .is_some()
            {
                return Err(AppError {
                    code: AppErrorCode::ProjectDirectoryNotEmpty,
                    message: "Seçilen klasör boş değil. Yeni proje için boş bir klasör seçin."
                        .to_string(),
                    recoverable: true,
                    suggested_action: Some("Boş bir klasör seçip tekrar deneyin.".to_string()),
                    technical_details: Some(root.to_string_lossy().to_string()),
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
            false
        } else {
            if let Some(parent) = root.parent() {
                std::fs::create_dir_all(parent).map_err(|error| AppError {
                    code: AppErrorCode::ProjectSaveFailed,
                    message: "Yeni proje için parent klasör oluşturulamadı.".to_string(),
                    recoverable: true,
                    suggested_action: Some("Klasör izinlerini kontrol edin.".to_string()),
                    technical_details: Some(error.to_string()),
                    correlation_id: Uuid::new_v4().to_string(),
                })?;
            }
            std::fs::create_dir(&root).map_err(|error| AppError {
                code: AppErrorCode::ProjectSaveFailed,
                message: "Yeni proje klasörü oluşturulamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Klasör izinlerini kontrol edin.".to_string()),
                technical_details: Some(error.to_string()),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
            true
        };
        let project_id = Uuid::new_v4().to_string();

        let dirs = [
            "documents",
            "cache/page_previews",
            "cache/model_raw",
            "cache/model_inputs",
            "crops",
            "outputs",
            "outputs/previews",
            "outputs/ocr_generations",
            "logs",
            "logs/jobs",
        ];

        let setup_result = (|| -> Result<(), AppError> {
            for dir in &dirs {
                let managed = trusted_root.managed(dir)?;
                trusted_root.ensure_managed_directory(&root.join(managed.as_path()))?;
            }
            Ok(())
        })();
        if let Err(error) = setup_result {
            if root_was_created {
                let _ = std::fs::remove_dir_all(&root);
            }
            return Err(error);
        }

        let now = chrono::Utc::now().to_rfc3339();

        let project = Project {
            id: project_id,
            name,
            created_at: now.clone(),
            updated_at: now,
            root_path: trusted_root.root_string(),
            storage_revision: 0,
            academic_year_id,
            course_id,
            course_name,
            sections: vec![],
            students: vec![],
            school_classes: vec![],
            teaching_assignments: vec![],
            assessment_activities: vec![],
            student_scan_batches: vec![],
            student_submissions: vec![],
            student_scan_document_id: None,
            student_grouping_mode: None,
            student_pages_per_student: None,
            student_grouping_complete_at: None,
            expected_question_count: None,
            exam_package_freeze: None,
            documents: vec![],
            questions: vec![],
            scoring_records: vec![],
            scoring_anchors: vec![],
            speaking_exams: vec![],
            latest_scoring_run_id: None,
            student_answer_ocr_records: vec![],
            student_answer_ocr_generations: vec![],
            student_answer_crop_template: Default::default(),
            student_identity_crop_template: None,
            workflow: WorkflowSnapshot {
                current_stage: WorkflowStage::DocumentsMissing,
                current_stage_label: "Belgeler Eksik".to_string(),
                blocking_reasons: vec![],
                next_actions: vec![],
                summary: crate::domain::workflow::WorkflowSummary::default(),
            },
        };

        let mut project = project;
        project.workflow = workflow_engine::evaluate_workflow(&project);
        let content = serde_json::to_string_pretty(&project).map_err(|error| AppError {
            code: AppErrorCode::ProjectSaveFailed,
            message: "Yeni proje verisi hazırlanamadı.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        self.ensure_write_lease(&trusted_root)?;
        let project_path = trusted_root.managed("project.json")?;
        if trusted_root.resolve_existing_file(&project_path).is_ok() {
            if root_was_created {
                let _ = std::fs::remove_dir_all(&root);
            }
            return Err(AppError {
                code: AppErrorCode::ProjectAlreadyExists,
                message: "Bu klasörde zaten bir Rubrika projesi bulunuyor.".to_string(),
                recoverable: true,
                suggested_action: Some("Yeni proje için boş bir klasör seçin.".to_string()),
                technical_details: Some(root.display().to_string()),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        if let Err(error) = trusted_root.atomic_write(&project_path, &content) {
            if root_was_created {
                let _ = std::fs::remove_dir_all(&root);
            }
            return Err(error);
        }

        self.set_trusted_root(trusted_root.clone())?;
        self.set_session_project(project.clone(), fingerprint(&content))?;

        Ok(project)
    }

    pub fn open_project(&self, root_path: String) -> Result<Project, AppError> {
        self.open_project_with_mode(root_path, ProjectOpenMode::OpenWithoutMigration)
            .map(|(project, _)| project)
    }

    /// Opens a project under an explicit durability contract.
    pub fn open_project_with_mode(
        &self,
        root_path: String,
        mode: ProjectOpenMode,
    ) -> Result<(Project, Vec<String>), AppError> {
        let trusted_root = TrustedProjectRoot::open_selected(Path::new(&root_path))?;
        if mode != ProjectOpenMode::InspectReadOnly {
            self.ensure_write_lease(&trusted_root)?;
        }
        let mut loaded = Self::load_project_file_with_root(&trusted_root, true)?;
        let migration_required = loaded.migration_changed || loaded.legacy_revision_missing;

        if mode == ProjectOpenMode::OpenWithoutMigration && migration_required {
            return Err(project_migration_required_error(&trusted_root, &loaded));
        }

        if mode == ProjectOpenMode::MigrateWithVerifiedBackup && migration_required {
            let backup = crate::services::backup_service::create_verified_backup(
                trusted_root.root(),
                None,
                &tokio_util::sync::CancellationToken::new(),
            )?;
            canonicalize_speaking_attempts(&mut loaded.project);
            Self::persist_migrated_project(&trusted_root, &loaded.project)?;
            loaded.warnings.push(format!(
                "Migration verified backup created at {} before canonical commit.",
                backup.archive_path
            ));
        }

        if mode == ProjectOpenMode::InspectReadOnly {
            if migration_required {
                loaded.warnings.push(
                    "Bu proje yeni veri biçimine geçirilmelidir; salt-okunur inceleme hiçbir değişiklik yapmadı."
                        .to_string(),
                );
            }
            return Ok((loaded.project, loaded.warnings));
        }

        self.legacy_project_without_revision
            .store(loaded.legacy_revision_missing, Ordering::Relaxed);
        let project_path =
            trusted_root.resolve_existing_file(&trusted_root.managed("project.json")?)?;
        let content = std::fs::read_to_string(&project_path).map_err(|error| {
            project_error(
                AppErrorCode::ProjectLoadFailed,
                "Proje dosyası okunamadı.",
                Some(error.to_string()),
            )
        })?;
        self.set_trusted_root(trusted_root)?;
        self.set_session_project(loaded.project.clone(), fingerprint(&content))?;
        Ok((loaded.project, loaded.warnings))
    }

    /// Ensures this store holds the OS-level write lease for `root`.
    ///
    /// A different root releases the previous lease first. If another
    /// process holds the lease, `ProjectAlreadyOpen` is returned and no
    /// writer path may proceed.
    fn ensure_write_lease(&self, root: &TrustedProjectRoot) -> Result<(), AppError> {
        let mut lease_slot = self.write_lease.lock().map_err(lock_error)?;
        if let Some(lease) = lease_slot.as_ref() {
            if lease.lock_path().parent() == Some(root.root()) {
                return Ok(());
            }
            *lease_slot = None;
        }
        let lease = acquire_or_share(root.root())?;
        *lease_slot = Some(lease);
        Ok(())
    }

    pub fn open_project_with_warnings(
        &self,
        root_path: String,
    ) -> Result<(Project, Vec<String>), AppError> {
        let trusted_root = TrustedProjectRoot::open_selected(Path::new(&root_path))?;
        self.ensure_write_lease(&trusted_root)?;
        let mut loaded = Self::load_project_file_with_root(&trusted_root, true)?;
        let interrupted_generations = loaded
            .project
            .student_answer_ocr_generations
            .iter()
            .filter(|generation| {
                generation.status == crate::domain::student::OcrGenerationStatus::Candidate
            })
            .count();
        self.set_trusted_root(trusted_root.clone())?;
        if interrupted_generations > 0 {
            match self.mutate(
                &loaded.project.id,
                MutationOptions::new("recover_orphaned_ocr_generations"),
                |project, _context| {
                    project.recover_orphaned_ocr_generations();
                    Ok(())
                },
            ) {
                Ok(recovery) => {
                    loaded.project = recovery.snapshot.project;
                    loaded.warnings.push(format!(
                        "{} OCR candidate uygulama yeniden açılırken interrupted olarak işaretlendi; aktif sonuç korundu.",
                        interrupted_generations
                    ));
                }
                Err(error) => loaded.warnings.push(format!(
                    "OCR recovery tamamlanamadı; aktif sonuç korundu ve yeniden deneme önerildi: {}",
                    error.message
                )),
            }
        }
        self.legacy_project_without_revision
            .store(loaded.legacy_revision_missing, Ordering::Relaxed);
        if loaded.migration_changed {
            canonicalize_speaking_attempts(&mut loaded.project);
            let backup_path = Self::persist_migrated_project(&trusted_root, &loaded.project)?;
            loaded.warnings.push(format!(
                "Legacy class/batch migration was persisted atomically after creating backup {}.",
                backup_path.display()
            ));
        }

        let project_path =
            trusted_root.resolve_existing_file(&trusted_root.managed("project.json")?)?;
        let content = std::fs::read_to_string(&project_path).map_err(|error| {
            project_error(
                AppErrorCode::ProjectLoadFailed,
                "Proje dosyası okunamadı.",
                Some(error.to_string()),
            )
        })?;
        self.set_session_project(loaded.project.clone(), fingerprint(&content))?;

        Ok((loaded.project, loaded.warnings))
    }

    fn set_trusted_root(&self, root: TrustedProjectRoot) -> Result<(), AppError> {
        let mut lock = self.trusted_root.lock().map_err(|error| AppError {
            code: AppErrorCode::UnknownError,
            message: "Project store lock failed.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        *lock = Some(root);
        Ok(())
    }

    fn set_session_project(
        &self,
        project: Project,
        content_fingerprint: String,
    ) -> Result<(), AppError> {
        let mut project_lock = self.current_project.lock().map_err(lock_error)?;
        if let Some(previous) = project_lock.as_ref() {
            let mut history = self.revision_history.lock().map_err(lock_error)?;
            history.insert(
                (previous.id.clone(), previous.storage_revision),
                previous.clone(),
            );
            if history.len() > 64 {
                if let Some(key) = history.keys().next().cloned() {
                    history.remove(&key);
                }
            }
        }
        *project_lock = Some(project);
        drop(project_lock);
        let mut fingerprint_lock = self.current_fingerprint.lock().map_err(lock_error)?;
        *fingerprint_lock = Some(content_fingerprint);
        Ok(())
    }

    pub fn trusted_project_root(&self, project_id: &str) -> Result<TrustedProjectRoot, AppError> {
        let project_lock = self.current_project.lock().map_err(|error| AppError {
            code: AppErrorCode::UnknownError,
            message: "Project store lock failed.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        if project_lock
            .as_ref()
            .map(|project| project.id != project_id)
            .unwrap_or(true)
        {
            return Err(AppError {
                code: AppErrorCode::ProjectNotFound,
                message: "İstenen proje açık değil.".to_string(),
                recoverable: true,
                suggested_action: Some("Projeyi yeniden açıp tekrar deneyin.".to_string()),
                technical_details: Some(format!("project_id={project_id}")),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        drop(project_lock);
        let root_lock = self.trusted_root.lock().map_err(|error| AppError {
            code: AppErrorCode::UnknownError,
            message: "Project store lock failed.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        root_lock.clone().ok_or_else(|| AppError {
            code: AppErrorCode::ProjectNotFound,
            message: "Güvenilir proje kökü bulunamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("Projeyi yeniden açıp tekrar deneyin.".to_string()),
            technical_details: None,
            correlation_id: Uuid::new_v4().to_string(),
        })
    }

    pub fn persistence_diagnostics(&self) -> PersistenceDiagnostics {
        let storage_revision = self
            .current_project
            .lock()
            .ok()
            .and_then(|project| project.as_ref().map(|project| project.storage_revision))
            .unwrap_or(0);
        let fingerprint_known = self
            .current_fingerprint
            .lock()
            .ok()
            .and_then(|value| value.clone())
            .is_some_and(|value| !value.is_empty());
        PersistenceDiagnostics {
            storage_revision,
            project_fingerprint_status: if fingerprint_known {
                "known".to_string()
            } else {
                "unknown".to_string()
            },
            stale_job_result_count: self.stale_job_result_count.load(Ordering::Relaxed),
            mutation_conflict_count: self.mutation_conflict_count.load(Ordering::Relaxed),
            external_modification_detected: self
                .external_modification_detected
                .load(Ordering::Relaxed),
            legacy_project_without_revision: self
                .legacy_project_without_revision
                .load(Ordering::Relaxed),
        }
    }

    fn session_fingerprint(&self) -> Result<Option<String>, AppError> {
        self.current_fingerprint
            .lock()
            .map(|value| value.clone())
            .map_err(lock_error)
    }

    fn read_disk_snapshot(
        &self,
        trusted_root: &TrustedProjectRoot,
    ) -> Result<ProjectSnapshot, AppError> {
        let project_path =
            trusted_root.resolve_existing_file(&trusted_root.managed("project.json")?)?;
        let content = std::fs::read_to_string(&project_path).map_err(|error| {
            project_error(
                AppErrorCode::ProjectLoadFailed,
                "Proje dosyası okunamadı; değişiklik uygulanmadı.",
                Some(error.to_string()),
            )
        })?;
        let loaded = Self::load_project_file_with_root(trusted_root, true)?;
        let revision = loaded.project.storage_revision;
        Ok(ProjectSnapshot {
            project: loaded.project,
            revision,
            content_fingerprint: fingerprint(&content),
            trusted_root: trusted_root.clone(),
        })
    }

    fn persist_migrated_project(
        trusted_root: &TrustedProjectRoot,
        project: &Project,
    ) -> Result<PathBuf, AppError> {
        let project_file =
            trusted_root.resolve_existing_file(&trusted_root.managed("project.json")?)?;
        let original_content =
            std::fs::read_to_string(&project_file).map_err(|error| AppError {
                code: AppErrorCode::ProjectLoadFailed,
                message: "Migration öncesi proje dosyası okunamadı.".to_string(),
                recoverable: false,
                suggested_action: Some("Proje klasörü izinlerini kontrol edin.".to_string()),
                technical_details: Some(format!(
                    "project_file={}; read_error={error}",
                    project_file.display()
                )),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%.9fZ");
        let backup_name = format!("project.json.migration.{timestamp}.bak");
        let backup_path = project_file
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(backup_name);
        let backup_name = backup_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("project.json.bak");
        let backup_relative = trusted_root.managed(backup_name)?;
        trusted_root
            .atomic_write(&backup_relative, &original_content)
            .map_err(|error| AppError {
                code: AppErrorCode::ProjectSaveFailed,
                message: "Migration yedeği oluşturulamadığı için proje değiştirilmedi.".to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Proje klasöründe yazma izni ve disk alanını kontrol edin.".to_string(),
                ),
                technical_details: Some(format!(
                    "backup_path={}; write_error={error}",
                    backup_path.display()
                )),
                correlation_id: Uuid::new_v4().to_string(),
            })?;

        let migrated_content = serde_json::to_string_pretty(project).map_err(|error| AppError {
            code: AppErrorCode::ProjectSaveFailed,
            message: "Göç edilen proje verisi hazırlanamadı.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        trusted_root
            .atomic_write(&trusted_root.managed("project.json")?, &migrated_content)
            .map_err(|error| AppError {
                code: AppErrorCode::ProjectSaveFailed,
                message: "Göç edilen proje atomik olarak kaydedilemedi; yedek korundu.".to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Yedek dosyayı koruyup klasör izinlerini kontrol edin.".to_string(),
                ),
                technical_details: Some(format!(
                    "project_file={}; backup_path={}; write_error={error}",
                    project_file.display(),
                    backup_path.display()
                )),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        Ok(backup_path)
    }

    pub fn list_projects(&self) -> ListProjectsOutput {
        Self::list_projects_in_dir(&default_projects_root_dir())
    }

    /// Transactional canonical mutation. The project lock is acquired before
    /// the disk read, so a read-modify-write is one indivisible operation for
    /// this project while unrelated project roots remain independent.
    pub fn mutate<T, F>(
        &self,
        project_id: &str,
        options: MutationOptions,
        mutation: F,
    ) -> Result<MutationOutput<T>, AppError>
    where
        F: FnOnce(&mut Project, &MutationContext) -> Result<T, AppError>,
    {
        let trusted_root = self.trusted_project_root(project_id)?;
        self.ensure_write_lease(&trusted_root)?;
        let project_lock = project_lock_for(trusted_root.root())?;
        let _guard = project_lock.lock().map_err(lock_error)?;
        let current = self.read_disk_snapshot(&trusted_root)?;

        if let Some(expected_fingerprint) = options.expected_fingerprint.as_deref() {
            if expected_fingerprint != current.content_fingerprint {
                self.external_modification_detected
                    .store(true, Ordering::Relaxed);
                self.mutation_conflict_count.fetch_add(1, Ordering::Relaxed);
                return Err(external_modification_error(&options.correlation_id));
            }
        } else if self.session_fingerprint()?.as_deref()
            != Some(current.content_fingerprint.as_str())
        {
            self.external_modification_detected
                .store(true, Ordering::Relaxed);
            self.mutation_conflict_count.fetch_add(1, Ordering::Relaxed);
            return Err(external_modification_error(&options.correlation_id));
        }
        if let Some(expected_revision) = options.expected_revision {
            if expected_revision != current.revision {
                self.mutation_conflict_count.fetch_add(1, Ordering::Relaxed);
                return Err(revision_conflict_error(
                    expected_revision,
                    current.revision,
                    &options.correlation_id,
                ));
            }
        }

        let next_revision = current.revision.checked_add(1).ok_or_else(|| {
            project_error(
                AppErrorCode::ProjectMutationRejected,
                "Proje revision sınırına ulaştı; değişiklik kaydedilmedi.",
                None,
            )
        })?;
        let transaction = transaction_journal::begin(
            trusted_root.root(),
            project_id,
            &options.operation,
            &options.correlation_id,
            Some(current.revision),
            Some(next_revision),
        )?;

        let mut project = current.project;
        let context = MutationContext {
            project_id: project_id.to_string(),
            current_revision: project.storage_revision,
            current_fingerprint: current.content_fingerprint.clone(),
            trusted_root: trusted_root.clone(),
            operation: options.operation,
            correlation_id: options.correlation_id.clone(),
        };
        let result = match mutation(&mut project, &context) {
            Ok(result) => result,
            Err(error) => {
                transaction_journal::update(
                    trusted_root.root(),
                    &transaction.transaction_id,
                    "aborted",
                )?;
                return Err(error);
            }
        };
        canonicalize_speaking_attempts(&mut project);
        normalize_document_paths_for_save(&trusted_root, &mut project);
        project.root_path = trusted_root.root_string();
        project.updated_at = chrono::Utc::now().to_rfc3339();
        project.storage_revision = next_revision;
        project.workflow = workflow_engine::evaluate_workflow(&project);
        let content = serde_json::to_string_pretty(&project).map_err(|error| {
            project_error(
                AppErrorCode::ProjectSaveFailed,
                "Proje verisi hazırlanamadı; mevcut dosya korunuyor.",
                Some(error.to_string()),
            )
        })?;
        trusted_root.atomic_write(&trusted_root.managed("project.json")?, &content)?;
        transaction_journal::update(trusted_root.root(), &transaction.transaction_id, "complete")?;
        let new_fingerprint = fingerprint(&content);
        self.set_session_project(project.clone(), new_fingerprint.clone())?;
        Ok(MutationOutput {
            result,
            snapshot: ProjectSnapshot {
                revision: project.storage_revision,
                project,
                content_fingerprint: new_fingerprint,
                trusted_root,
            },
        })
    }

    /// Long jobs should call this at commit time and perform source/entity
    /// validation inside the closure. A stale source returns a typed stale
    /// outcome and never writes the candidate.
    pub fn commit_job<T, F>(
        &self,
        project_id: &str,
        options: MutationOptions,
        mutation: F,
    ) -> JobCommitResult<T>
    where
        F: FnOnce(&mut Project, &MutationContext) -> Result<T, AppError>,
    {
        match self.mutate(project_id, options, mutation) {
            Ok(output) => JobCommitResult::Applied(Box::new(output)),
            Err(error) if error.code == AppErrorCode::ProjectEntityStale => {
                self.stale_job_result_count.fetch_add(1, Ordering::Relaxed);
                JobCommitResult::Stale {
                    reason: error.message,
                }
            }
            Err(error) if error.code == AppErrorCode::ProjectEntityNotFound => {
                JobCommitResult::EntityMissing
            }
            Err(error)
                if matches!(
                    error.code,
                    AppErrorCode::ProjectRevisionConflict
                        | AppErrorCode::ProjectExternallyModified
                        | AppErrorCode::ProjectMutationConflict
                ) =>
            {
                self.mutation_conflict_count.fetch_add(1, Ordering::Relaxed);
                JobCommitResult::Conflict(error)
            }
            Err(error) => JobCommitResult::Rejected(error),
        }
    }

    /// Transitional CAS adapter for callers that already produce a candidate
    /// snapshot. It is conflict-safe, but new code should prefer a closure
    /// that edits the latest entity inside `mutate`; this adapter is retained
    /// only while legacy services are moved incrementally.
    pub(crate) fn commit_snapshot_cas(&self, project: &Project) -> Result<Project, AppError> {
        let candidate = project.clone();
        let project_id = candidate.id.clone();
        let base = self
            .revision_history
            .lock()
            .map_err(lock_error)?
            .get(&(project_id.clone(), candidate.storage_revision))
            .cloned()
            .or_else(|| {
                self.current_project
                    .lock()
                    .ok()
                    .and_then(|value| value.clone())
                    .filter(|value| value.storage_revision == candidate.storage_revision)
            });
        let Some(base) = base else {
            return Err(project_error(
                AppErrorCode::ProjectEntityStale,
                "Proje değişikliği artık geçerli bir snapshot'a dayanmıyor; işlem uygulanmadı.",
                Some(format!(
                    "missing_base_revision={}; project_id={}",
                    candidate.storage_revision, candidate.id
                )),
            ));
        };
        let output = self.mutate(
            &project_id,
            MutationOptions::new("legacy_snapshot_merge"),
            move |current, _context| {
                merge_candidate_project(&base, &candidate, current)?;
                Ok(())
            },
        )?;
        Ok(output.snapshot.project)
    }

    /// Fixture-only whole-project replacement. Production modules cannot use
    /// this API; canonical mutations must use `mutate` or `commit_job`.
    #[cfg(test)]
    pub(crate) fn save_project(&self, project: &Project) -> Result<(), AppError> {
        if self.trusted_project_root(&project.id).is_err() {
            let root =
                TrustedProjectRoot::from_canonical_root(PathBuf::from(&project.root_path), false)?;
            self.set_trusted_root(root.clone())?;
            self.set_session_project(project.clone(), String::new())?;
        }
        let expected_revision = project.storage_revision;
        let expected_fingerprint = self.session_fingerprint()?;
        let candidate = project.clone();
        let _ = self.mutate(
            &project.id,
            MutationOptions {
                expected_revision: Some(expected_revision),
                expected_fingerprint: expected_fingerprint.filter(|value| !value.is_empty()),
                operation: "test_fixture_replace".to_string(),
                correlation_id: Uuid::new_v4().to_string(),
            },
            move |current, _| {
                *current = candidate;
                Ok(())
            },
        )?;
        Ok(())
    }

    pub fn get_project_snapshot(&self, project_id: String) -> Result<Project, AppError> {
        Ok(self
            .get_project_snapshot_with_metadata(&project_id)?
            .project)
    }

    pub fn get_project_snapshot_with_metadata(
        &self,
        project_id: &str,
    ) -> Result<ProjectSnapshot, AppError> {
        let trusted_root = self.trusted_project_root(project_id)?;
        let content_fingerprint = self.session_fingerprint()?.unwrap_or_default();
        let lock = self.current_project.lock().map_err(|e| AppError {
            code: AppErrorCode::UnknownError,
            message: "Project store lock failed.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(e.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        if let Some(project) = lock.as_ref() {
            if project.id == project_id {
                let mut snapshot = project.clone();
                hydrate_speaking_attempts(&mut snapshot);
                let revision = snapshot.storage_revision;
                Ok(ProjectSnapshot {
                    project: snapshot,
                    revision,
                    content_fingerprint,
                    trusted_root,
                })
            } else {
                Err(AppError {
                    code: AppErrorCode::ProjectNotFound,
                    message: "Requested project is not open.".to_string(),
                    recoverable: true,
                    suggested_action: Some("Open the project before continuing.".to_string()),
                    technical_details: None,
                    correlation_id: Uuid::new_v4().to_string(),
                })
            }
        } else {
            Err(AppError {
                code: AppErrorCode::ProjectNotFound,
                message: "No project is currently open.".to_string(),
                recoverable: true,
                suggested_action: Some("Open a project first.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            })
        }
    }

    pub fn open_project_at_path(project_path: &Path) -> Result<Project, AppError> {
        let project_file = project_path.join("project.json");
        Self::load_project_file(&project_file, true).map(|loaded| loaded.project)
    }

    /// Read-only migration inspection. It parses and normalizes in memory but
    /// never acquires a writer lease or persists the normalized value.
    pub fn migration_required_at_path(project_path: &Path) -> Result<bool, AppError> {
        let trusted_root = TrustedProjectRoot::open_selected(project_path)?;
        let loaded = Self::load_project_file_with_root(&trusted_root, true)?;
        Ok(loaded.migration_changed || loaded.legacy_revision_missing)
    }

    pub(crate) fn list_projects_in_dir(projects_root: &Path) -> ListProjectsOutput {
        let mut warnings = Vec::new();
        let mut skipped_projects = Vec::new();
        let mut entries = Vec::new();

        let read_dir = match std::fs::read_dir(projects_root) {
            Ok(read_dir) => read_dir,
            Err(error) => {
                if error.kind() != std::io::ErrorKind::NotFound {
                    warnings.push(format!("Project root could not be read: {}", error));
                }
                return ListProjectsOutput {
                    projects: vec![],
                    warnings,
                    skipped_projects,
                };
            }
        };

        for entry in read_dir.flatten() {
            let project_dir = entry.path();
            if !project_dir.is_dir() {
                continue;
            }

            let project_file = project_dir.join("project.json");
            if !project_file.is_file() {
                continue;
            }

            let modified_at = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .ok();

            match Self::load_project_file(&project_file, true) {
                Ok(loaded) => {
                    let mut project = loaded.project;
                    project.workflow = workflow_engine::evaluate_workflow(&project);
                    if !loaded.warnings.is_empty() {
                        warnings.extend(
                            loaded
                                .warnings
                                .into_iter()
                                .map(|warning| format!("{}: {}", project_dir.display(), warning)),
                        );
                    }
                    entries.push(ProjectListEntry {
                        sort_key: system_time_sort_key(modified_at),
                        item: project_to_list_item(&project, &project_dir),
                    });
                }
                Err(error) => {
                    skipped_projects.push(SkippedProject {
                        path: project_dir.to_string_lossy().to_string(),
                        reason: format!("Invalid project.json: {}", error.message),
                        technical_details: error.technical_details.clone(),
                    });
                }
            }
        }

        entries.sort_by(|left, right| right.sort_key.cmp(&left.sort_key));

        ListProjectsOutput {
            projects: entries.into_iter().map(|entry| entry.item).collect(),
            warnings,
            skipped_projects,
        }
    }
}

fn lock_error<T>(error: std::sync::PoisonError<T>) -> AppError {
    project_error(
        AppErrorCode::UnknownError,
        "Proje store kilidi kullanılamadı.",
        Some(error.to_string()),
    )
}

fn project_error(code: AppErrorCode, message: &str, technical_details: Option<String>) -> AppError {
    AppError {
        code,
        message: message.to_string(),
        recoverable: true,
        suggested_action: None,
        technical_details,
        correlation_id: Uuid::new_v4().to_string(),
    }
}

fn project_migration_required_error(root: &TrustedProjectRoot, loaded: &LoadedProject) -> AppError {
    AppError {
        code: AppErrorCode::ProjectMigrationRequired,
        message: "Bu proje yeni veri biçimine geçirilmelidir. Önce doğrulanmış yedek oluşturulacaktır."
            .to_string(),
        recoverable: true,
        suggested_action: Some(
            "Migration modunda açmayı seçin; migration başlamadan önce bağımsız doğrulanmış yedek alınır."
                .to_string(),
        ),
        technical_details: Some(format!(
            "root={}; migration_changed={}; legacy_revision_missing={}",
            root.root().display(),
            loaded.migration_changed,
            loaded.legacy_revision_missing
        )),
        correlation_id: Uuid::new_v4().to_string(),
    }
}

fn external_modification_error(correlation_id: &str) -> AppError {
    AppError {
        code: AppErrorCode::ProjectExternallyModified,
        message: "Proje siz çalışırken dışarıdan güncellendi. Son durumu yenileyip işlemi yeniden deneyin.".to_string(),
        recoverable: true,
        suggested_action: Some("Son durumu yenileyip işlemi yeniden deneyin.".to_string()),
        technical_details: Some("project.json content fingerprint changed".to_string()),
        correlation_id: correlation_id.to_string(),
    }
}

fn revision_conflict_error(expected: u64, current: u64, correlation_id: &str) -> AppError {
    AppError {
        code: AppErrorCode::ProjectRevisionConflict,
        message: "Proje siz çalışırken başka bir işlem tarafından güncellendi. Son durumu yenileyip işlemi yeniden deneyin.".to_string(),
        recoverable: true,
        suggested_action: Some("Son durumu yenileyip işlemi yeniden deneyin.".to_string()),
        technical_details: Some(format!("expected_revision={expected}; current_revision={current}")),
        correlation_id: correlation_id.to_string(),
    }
}

fn fingerprint(content: &str) -> String {
    // FNV-1a is deterministic, dependency-free, and sufficient for detecting
    // accidental/external project.json replacement. It is not a security hash.
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in content.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn project_lock_for(root: &Path) -> Result<Arc<Mutex<()>>, AppError> {
    let registry = PROJECT_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut registry = registry.lock().map_err(lock_error)?;
    registry.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = registry.get(root).and_then(Weak::upgrade) {
        return Ok(lock);
    }
    let lock = Arc::new(Mutex::new(()));
    registry.insert(root.to_path_buf(), Arc::downgrade(&lock));
    Ok(lock)
}

fn merge_candidate_project(
    base: &Project,
    candidate: &Project,
    current: &mut Project,
) -> Result<(), AppError> {
    let base_value = serde_json::to_value(base).map_err(|error| {
        project_error(
            AppErrorCode::ProjectMutationRejected,
            "Proje değişikliği doğrulanamadı.",
            Some(error.to_string()),
        )
    })?;
    let candidate_value = serde_json::to_value(candidate).map_err(|error| {
        project_error(
            AppErrorCode::ProjectMutationRejected,
            "Proje değişikliği doğrulanamadı.",
            Some(error.to_string()),
        )
    })?;
    let mut current_value = serde_json::to_value(&*current).map_err(|error| {
        project_error(
            AppErrorCode::ProjectMutationRejected,
            "Güncel proje değişikliği doğrulanamadı.",
            Some(error.to_string()),
        )
    })?;
    merge_json_values(&base_value, &candidate_value, &mut current_value, "project")?;
    *current = serde_json::from_value(current_value).map_err(|error| {
        project_error(
            AppErrorCode::ProjectMutationRejected,
            "Proje değişikliği geçerli bir proje oluşturmadı.",
            Some(error.to_string()),
        )
    })?;
    Ok(())
}

fn merge_json_values(
    base: &Value,
    candidate: &Value,
    current: &mut Value,
    path: &str,
) -> Result<(), AppError> {
    if candidate == base || candidate == current {
        return Ok(());
    }
    if current == base {
        *current = candidate.clone();
        return Ok(());
    }

    match (base, candidate, current) {
        (Value::Object(base), Value::Object(candidate), Value::Object(current)) => {
            let keys = base
                .keys()
                .chain(candidate.keys())
                .cloned()
                .collect::<HashSet<_>>();
            for key in keys {
                if matches!(
                    key.as_str(),
                    "storageRevision" | "updatedAt" | "workflow" | "rootPath"
                ) {
                    continue;
                }
                let base_value = base.get(&key).unwrap_or(&Value::Null);
                let candidate_value = candidate.get(&key).unwrap_or(&Value::Null);
                let current_value = current.get_mut(&key);
                match (candidate.contains_key(&key), current_value) {
                    (false, Some(current_value)) => {
                        if current_value == base_value {
                            current.remove(&key);
                        } else if candidate_value != current_value {
                            return Err(merge_conflict(&format!("{path}.{key}")));
                        }
                    }
                    (true, Some(current_value)) => merge_json_values(
                        base_value,
                        candidate_value,
                        current_value,
                        &format!("{path}.{key}"),
                    )?,
                    (true, None) => {
                        if base_value == &Value::Null {
                            current.insert(key, candidate_value.clone());
                        } else {
                            return Err(merge_conflict(&format!("{path}.{key}")));
                        }
                    }
                    (false, None) => {}
                }
            }
            Ok(())
        }
        (Value::Array(base), Value::Array(candidate), Value::Array(current))
            if arrays_have_stable_ids(base, candidate, current) =>
        {
            merge_id_arrays(base, candidate, current, path)
        }
        _ => Err(merge_conflict(path)),
    }
}

fn arrays_have_stable_ids(values: &[Value], candidate: &[Value], current: &[Value]) -> bool {
    values
        .iter()
        .chain(candidate)
        .chain(current)
        .all(|value| value.get("id").and_then(Value::as_str).is_some())
}

fn merge_id_arrays(
    base: &[Value],
    candidate: &[Value],
    current: &mut Vec<Value>,
    path: &str,
) -> Result<(), AppError> {
    for base_value in base {
        let Some(id) = base_value.get("id").and_then(Value::as_str) else {
            return Err(merge_conflict(path));
        };
        let candidate_value = candidate
            .iter()
            .find(|value| value.get("id").and_then(Value::as_str) == Some(id));
        let current_index = current
            .iter()
            .position(|value| value.get("id").and_then(Value::as_str) == Some(id));
        match (candidate_value, current_index) {
            (None, Some(index)) => {
                if current[index] == *base_value {
                    current.remove(index);
                } else {
                    return Err(merge_conflict(&format!("{path}[{id}]")));
                }
            }
            (Some(candidate_value), Some(index)) => merge_json_values(
                base_value,
                candidate_value,
                &mut current[index],
                &format!("{path}[{id}]"),
            )?,
            (Some(_), None) => return Err(merge_conflict(&format!("{path}[{id}]"))),
            (None, None) => {}
        }
    }

    for candidate_value in candidate {
        let Some(id) = candidate_value.get("id").and_then(Value::as_str) else {
            return Err(merge_conflict(path));
        };
        let in_base = base
            .iter()
            .any(|value| value.get("id").and_then(Value::as_str) == Some(id));
        let current_value = current
            .iter()
            .find(|value| value.get("id").and_then(Value::as_str) == Some(id));
        if !in_base && current_value.is_none() {
            current.push(candidate_value.clone());
        } else if !in_base {
            let current_value = current
                .iter()
                .find(|value| value.get("id").and_then(Value::as_str) == Some(id))
                .ok_or_else(|| merge_conflict(&format!("{path}[{id}]")))?;
            if current_value != candidate_value {
                return Err(merge_conflict(&format!("{path}[{id}]")));
            }
        }
    }
    Ok(())
}

fn merge_conflict(path: &str) -> AppError {
    project_error(
        AppErrorCode::ProjectMutationConflict,
        "Aynı proje kaydında çakışan değişiklik bulundu. Son durumu yenileyip işlemi yeniden deneyin.",
        Some(format!("conflicting_path={path}")),
    )
}

struct ProjectListEntry {
    sort_key: u128,
    item: ProjectListItem,
}

fn default_projects_root_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join("Documents").join("RubrikaV3").join("Projects")
}

fn system_time_sort_key(value: Option<SystemTime>) -> u128 {
    value
        .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

impl ProjectStore {
    fn load_project_file(
        project_file: &Path,
        refresh_workflow: bool,
    ) -> Result<LoadedProject, AppError> {
        let selected_root = project_file.parent().ok_or_else(|| AppError {
            code: AppErrorCode::ProjectLoadFailed,
            message: "Proje klasörü belirlenemedi.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(project_file.display().to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        let trusted_root = TrustedProjectRoot::open_selected(selected_root)?;
        Self::load_project_file_with_root(&trusted_root, refresh_workflow)
    }

    fn load_project_file_with_root(
        trusted_root: &TrustedProjectRoot,
        refresh_workflow: bool,
    ) -> Result<LoadedProject, AppError> {
        let project_file =
            trusted_root.resolve_existing_file(&trusted_root.managed("project.json")?)?;
        let content = std::fs::read_to_string(&project_file).map_err(|error| AppError {
            code: AppErrorCode::ProjectLoadFailed,
            message: "Could not read project file.".to_string(),
            recoverable: false,
            suggested_action: Some("Ensure the project folder contains project.json.".to_string()),
            technical_details: Some(format!(
                "project_file={}; read_error={}",
                project_file.display(),
                error
            )),
            correlation_id: Uuid::new_v4().to_string(),
        })?;

        let (mut project, mut warnings, migration_changed, legacy_revision_missing) =
            Self::deserialize_project(&project_file, &content)?;
        if project.root_path.trim() != trusted_root.root_string() {
            warnings.push(format!(
                "Project root metadata mismatch: stored root_path was not used; runtime root is {}.",
                trusted_root.root_string()
            ));
        }
        adapt_loaded_document_paths(trusted_root, &mut project, &mut warnings);
        // Keep all existing services on the session-bound canonical root. The
        // serialized root_path remains legacy/display metadata and can never
        // select a write target.
        project.root_path = trusted_root.root_string();
        if refresh_workflow {
            project.workflow = workflow_engine::evaluate_workflow(&project);
        }
        Ok(LoadedProject {
            project,
            warnings,
            migration_changed,
            legacy_revision_missing,
        })
    }

    fn deserialize_project(
        project_file: &Path,
        content: &str,
    ) -> Result<(Project, Vec<String>, bool, bool), AppError> {
        let mut value: Value = serde_json::from_str(content).map_err(|error| AppError {
            code: AppErrorCode::ProjectLoadFailed,
            message: "Project JSON syntax error.".to_string(),
            recoverable: false,
            suggested_action: Some("Check the JSON syntax or restore a backup copy.".to_string()),
            technical_details: Some(format!(
                "project_file={}; json_error={}; line={}; column={}",
                project_file.display(),
                error,
                error.line(),
                error.column()
            )),
            correlation_id: Uuid::new_v4().to_string(),
        })?;

        let legacy_revision_missing = value.get("storageRevision").is_none();
        let (mut warnings, migration_changed) = normalize_project_json(project_file, &mut value);

        let project_json = serde_json::to_string(&value).map_err(|error| AppError {
            code: AppErrorCode::ProjectLoadFailed,
            message: "Project JSON could not be normalized.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(format!(
                "project_file={}; json_error={}",
                project_file.display(),
                error
            )),
            correlation_id: Uuid::new_v4().to_string(),
        })?;

        let mut deserializer = serde_json::Deserializer::from_str(&project_json);
        let project = serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
            let path = error.path().to_string();
            let inner = error.into_inner();
            AppError {
                code: AppErrorCode::ProjectLoadFailed,
                message: "Project schema compatibility error.".to_string(),
                recoverable: false,
                suggested_action: Some(
                    "Open the project in a newer app version or restore a backup copy.".to_string(),
                ),
                technical_details: Some(format!(
                    "project_file={}; path={}; serde_error={}; line={}; column={}",
                    project_file.display(),
                    path,
                    inner,
                    inner.line(),
                    inner.column()
                )),
                correlation_id: Uuid::new_v4().to_string(),
            }
        })?;

        warnings.extend(school_class_reference_warnings(project_file, &project));
        Ok((
            project,
            warnings,
            migration_changed,
            legacy_revision_missing,
        ))
    }
}

fn adapt_loaded_document_paths(
    trusted_root: &TrustedProjectRoot,
    project: &mut Project,
    warnings: &mut Vec<String>,
) {
    for document in &mut project.documents {
        match trusted_root.adapt_legacy_document_path(&document.stored_path) {
            Ok(managed) => {
                if managed.as_str() != document.stored_path {
                    warnings.push(format!(
                        "Legacy document path normalized in memory: document_id={}",
                        document.id
                    ));
                    document.stored_path = managed.as_str().to_string();
                }
            }
            Err(error) => {
                warnings.push(format!(
                    "Legacy document path unresolved: document_id={}; code={:?}",
                    document.id, error.code
                ));
            }
        }
    }
}

fn normalize_document_paths_for_save(trusted_root: &TrustedProjectRoot, project: &mut Project) {
    for document in &mut project.documents {
        if let Ok(managed) = trusted_root.adapt_legacy_document_path(&document.stored_path) {
            document.stored_path = managed.as_str().to_string();
        }
    }
}

struct LoadedProject {
    project: Project,
    warnings: Vec<String>,
    migration_changed: bool,
    legacy_revision_missing: bool,
}

fn normalize_project_json(project_file: &Path, value: &mut Value) -> (Vec<String>, bool) {
    let mut warnings = Vec::new();
    let Some(project) = value.as_object_mut() else {
        return (warnings, false);
    };

    let class_changed = normalize_school_class_storage(project_file, project, &mut warnings);
    let assessment_changed =
        normalize_assessment_organization(project_file, project, &mut warnings);
    let crop_template_changed =
        normalize_student_answer_crop_template(project_file, project, &mut warnings);
    normalize_student_answer_ocr_records(project_file, project, &mut warnings);
    normalize_student_identity_records(project_file, project, &mut warnings);
    let scoring_changed = normalize_scoring_records(project_file, project, &mut warnings);
    let scoring_anchors_changed = normalize_scoring_anchors(project_file, project, &mut warnings);

    (
        warnings,
        class_changed
            || assessment_changed
            || crop_template_changed
            || scoring_changed
            || scoring_anchors_changed,
    )
}

fn normalize_scoring_anchors(
    project_file: &Path,
    project: &mut Map<String, Value>,
    warnings: &mut Vec<String>,
) -> bool {
    if project.get("scoringAnchors").is_some() {
        return false;
    }
    project.insert("scoringAnchors".to_string(), Value::Array(Vec::new()));
    warnings.push(format!(
        "{}.scoringAnchors alanı boş koleksiyon olarak tek yönlü göç edildi; mevcut scoring kayıtları korunuyor.",
        project_file.display()
    ));
    true
}

fn normalize_student_answer_crop_template(
    project_file: &Path,
    project: &mut Map<String, Value>,
    warnings: &mut Vec<String>,
) -> bool {
    let Some(template) = project
        .get_mut("studentAnswerCropTemplate")
        .and_then(Value::as_object_mut)
    else {
        return false;
    };

    if template.get("templates").is_some() {
        return false;
    }
    let Some(items) = template
        .remove("items")
        .and_then(|value| value.as_array().cloned())
    else {
        return false;
    };

    let mut grouped: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for (index, item) in items.iter().enumerate() {
        let Some(item_object) = item.as_object() else {
            warnings.push(format!(
                "{}.studentAnswerCropTemplate.items[{index}] nesne değildi; migration durduruldu.",
                project_file.display()
            ));
            template.insert("items".to_string(), Value::Array(items));
            return false;
        };
        let Some(question_id) = item_object
            .get("questionId")
            .or_else(|| item_object.get("question_id"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
        else {
            warnings.push(format!(
                "{}.studentAnswerCropTemplate.items[{index}] questionId eksikti; migration durduruldu.",
                project_file.display()
            ));
            template.insert("items".to_string(), Value::Array(items));
            return false;
        };
        let Some(bbox) = item_object.get("bbox").and_then(Value::as_object) else {
            warnings.push(format!(
                "{}.studentAnswerCropTemplate.items[{index}] bbox eksikti; migration durduruldu.",
                project_file.display()
            ));
            template.insert("items".to_string(), Value::Array(items));
            return false;
        };
        let page_offset = item_object
            .get("pageIndexWithinSubmission")
            .or_else(|| item_object.get("page_index_within_submission"))
            .or_else(|| bbox.get("pageIndex"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let region_index = grouped.get(question_id).map_or(0, Vec::len);
        let mut region = serde_json::Map::new();
        region.insert(
            "regionId".to_string(),
            Value::String(format!("{question_id}-region-{region_index}")),
        );
        region.insert("pageOffset".to_string(), Value::from(page_offset));
        region.insert("order".to_string(), Value::from(region_index));
        region.insert(
            "normalizedBBox".to_string(),
            serde_json::json!({
                "x": bbox.get("x").and_then(Value::as_f64).unwrap_or(0.0),
                "y": bbox.get("y").and_then(Value::as_f64).unwrap_or(0.0),
                "width": bbox.get("width").and_then(Value::as_f64).unwrap_or(0.0),
                "height": bbox.get("height").and_then(Value::as_f64).unwrap_or(0.0),
            }),
        );
        region.insert(
            "regionRole".to_string(),
            Value::String("primary".to_string()),
        );
        region.insert(
            "continuationPolicy".to_string(),
            Value::String("independent".to_string()),
        );
        if let Some(label) = item_object.get("label") {
            region.insert("label".to_string(), label.clone());
        }
        if let Some(note) = item_object.get("note") {
            region.insert("note".to_string(), note.clone());
        }
        grouped
            .entry(question_id.to_string())
            .or_default()
            .push(Value::Object(region));
    }

    template.insert(
        "templates".to_string(),
        Value::Array(
            grouped
                .into_iter()
                .map(|(question_id, regions)| {
                    serde_json::json!({
                        "questionId": question_id,
                        "regions": regions,
                    })
                })
                .collect(),
        ),
    );
    warnings.push(format!(
        "{}.studentAnswerCropTemplate eski single-region items alanı QuestionAnswerTemplate.regions biçimine taşındı.",
        project_file.display()
    ));
    true
}

fn normalize_assessment_organization(
    project_file: &Path,
    project: &mut Map<String, Value>,
    warnings: &mut Vec<String>,
) -> bool {
    let mut changed = false;
    let activity_snapshot = project
        .get("assessmentActivities")
        .cloned()
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();
    let class_snapshot = project
        .get("schoolClasses")
        .cloned()
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();

    if let Some(activities) = project
        .get_mut("assessmentActivities")
        .and_then(Value::as_array_mut)
    {
        for activity in activities {
            let Some(activity) = activity.as_object_mut() else {
                continue;
            };
            let assessment_type = activity
                .get("assessmentType")
                .and_then(Value::as_str)
                .unwrap_or("written")
                .to_string();
            let workflow_family = match assessment_type.as_str() {
                "speaking" => "speaking",
                "performance" => "performance",
                _ => "written",
            };
            if activity.get("workflowFamily").and_then(Value::as_str) != Some(workflow_family) {
                activity.insert(
                    "workflowFamily".to_string(),
                    Value::String(workflow_family.to_string()),
                );
                changed = true;
            }
            if activity.get("title").is_none() {
                activity.insert("title".to_string(), Value::String(String::new()));
                changed = true;
            }
            if assessment_type == "performance" && activity.get("performanceDetails").is_none() {
                activity.insert("performanceDetails".to_string(), Value::Object(Map::new()));
                changed = true;
            }
            if let Some(applications) = activity
                .get_mut("classApplications")
                .and_then(Value::as_array_mut)
            {
                for application in applications {
                    let Some(application) = application.as_object_mut() else {
                        continue;
                    };
                    if application.get("schoolClassId").is_none() {
                        if let Some(legacy) = application.get("classSectionId").cloned() {
                            application.insert("schoolClassId".to_string(), legacy);
                            changed = true;
                        }
                    }
                    if application.get("studentScopeIds").is_none() {
                        application.insert("studentScopeIds".to_string(), Value::Array(vec![]));
                        changed = true;
                    }
                    if application.get("speakingAttempts").is_none() {
                        application.insert("speakingAttempts".to_string(), Value::Array(vec![]));
                        changed = true;
                    }
                    if assessment_type == "performance"
                        && application.get("performanceAssessments").is_none()
                    {
                        application
                            .insert("performanceAssessments".to_string(), Value::Array(vec![]));
                        changed = true;
                    }
                }
            }
        }
    }

    let mut legacy_links: Vec<(String, String, Vec<String>)> = Vec::new();
    if let Some(exams) = project
        .get_mut("speakingExams")
        .and_then(Value::as_array_mut)
    {
        for exam in exams {
            let Some(exam) = exam.as_object_mut() else {
                continue;
            };
            let exam_id = exam
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("legacy-speaking-exam")
                .to_string();
            let activity_id = exam
                .get("assessmentActivityId")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
                .or_else(|| {
                    let candidates = activity_snapshot
                        .iter()
                        .filter(|activity| {
                            activity
                                .get("assessmentType")
                                .and_then(Value::as_str)
                                == Some("speaking")
                        })
                        .filter(|activity| {
                            let exam_title = exam.get("title").and_then(Value::as_str).unwrap_or("");
                            let activity_title = activity
                                .get("title")
                                .and_then(Value::as_str)
                                .unwrap_or("");
                            exam_title.is_empty()
                                || activity_title.is_empty()
                                || exam_title == activity_title
                        })
                        .filter(|activity| {
                            let exam_task = exam.get("taskText").and_then(Value::as_str).unwrap_or("");
                            let activity_task = activity
                                .get("speakingConfiguration")
                                .and_then(Value::as_object)
                                .and_then(|config| config.get("taskText"))
                                .and_then(Value::as_str)
                                .unwrap_or("");
                            exam_task.is_empty()
                                || activity_task.is_empty()
                                || exam_task == activity_task
                        })
                        .filter_map(|activity| activity.get("id").and_then(Value::as_str));
                    let candidates = candidates.collect::<Vec<_>>();
                    match candidates.as_slice() {
                        [only] => Some((*only).to_string()),
                        [] => {
                            warnings.push(format!(
                                "{}.speakingExams[{exam_id}] legacy ilişki için uygun AssessmentActivity bulunamadı; unresolved bırakıldı.",
                                project_file.display()
                            ));
                            None
                        }
                        _ => {
                            warnings.push(format!(
                                "{}.speakingExams[{exam_id}] birden fazla AssessmentActivity ile eşleşiyor; yanlış bağlama yapılmadı.",
                                project_file.display()
                            ));
                            None
                        }
                    }
                });

            let Some(activity_id) = activity_id else {
                continue;
            };
            if exam.get("assessmentActivityId").and_then(Value::as_str)
                != Some(activity_id.as_str())
            {
                exam.insert(
                    "assessmentActivityId".to_string(),
                    Value::String(activity_id.clone()),
                );
                changed = true;
            }
            let mut class_ids = exam
                .get("assignedClassIds")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if class_ids.is_empty() {
                if let Some(class_id) = exam.get("classId").and_then(Value::as_str) {
                    if !class_id.trim().is_empty() {
                        class_ids.push(class_id.to_string());
                    }
                }
            }
            class_ids.sort();
            class_ids.dedup();
            legacy_links.push((exam_id.clone(), activity_id.clone(), class_ids.clone()));

            if let Some(attempts) = exam.get_mut("attempts").and_then(Value::as_array_mut) {
                for attempt in attempts {
                    let Some(attempt) = attempt.as_object_mut() else {
                        continue;
                    };
                    if attempt.get("assessmentActivityId").is_none() {
                        attempt.insert(
                            "assessmentActivityId".to_string(),
                            Value::String(activity_id.clone()),
                        );
                        changed = true;
                    }
                    if attempt.get("schoolClassId").is_none() && class_ids.len() == 1 {
                        attempt.insert(
                            "schoolClassId".to_string(),
                            Value::String(class_ids[0].clone()),
                        );
                        changed = true;
                    }
                }
            }
        }
    }

    for (exam_id, activity_id, class_ids) in legacy_links {
        let Some(activities) = project
            .get_mut("assessmentActivities")
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        let Some(activity) = activities.iter_mut().find(|activity| {
            activity.get("id").and_then(Value::as_str) == Some(activity_id.as_str())
        }) else {
            warnings.push(format!(
                "{}.speakingExams[{exam_id}] AssessmentActivity referansı bulunamadı; unresolved bırakıldı.",
                project_file.display()
            ));
            continue;
        };
        let Some(activity_object) = activity.as_object_mut() else {
            continue;
        };
        let created_at = activity_object
            .get("createdAt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let applications = activity_object
            .entry("classApplications".to_string())
            .or_insert_with(|| Value::Array(vec![]));
        let Some(applications) = applications.as_array_mut() else {
            continue;
        };
        let mut all_classes_resolved = true;
        for class_id in class_ids {
            let class_exists = class_snapshot.iter().any(|school_class| {
                school_class.get("id").and_then(Value::as_str) == Some(class_id.as_str())
            });
            if !class_exists {
                all_classes_resolved = false;
                warnings.push(format!(
                    "{}.speakingExams[{exam_id}] sınıfı {class_id} bulunamadı; ClassApplication oluşturulmadı.",
                    project_file.display()
                ));
                continue;
            }
            if applications.iter().any(|application| {
                application.get("schoolClassId").and_then(Value::as_str) == Some(class_id.as_str())
            }) {
                continue;
            }
            let application_id = legacy_storage_id(
                "legacy-speaking-application",
                &format!("{activity_id}:{class_id}"),
            );
            applications.push(serde_json::json!({
                "id": application_id,
                "activityId": activity_id,
                "schoolClassId": class_id,
                "status": "scheduled",
                "documentIds": [],
                "studentScopeIds": [],
                "speakingAttempts": [],
                "createdAt": created_at,
                "updatedAt": created_at,
            }));
            changed = true;
        }
        if all_classes_resolved {
            if let Some(exams) = project
                .get_mut("speakingExams")
                .and_then(Value::as_array_mut)
            {
                if let Some(exam) = exams
                    .iter_mut()
                    .find(|exam| exam.get("id").and_then(Value::as_str) == Some(exam_id.as_str()))
                {
                    if let Some(exam) = exam.as_object_mut() {
                        exam.insert("assignedClassIds".to_string(), Value::Array(vec![]));
                        exam.insert("classId".to_string(), Value::Null);
                        changed = true;
                    }
                }
            }
        }
    }

    let application_snapshot = project
        .get("assessmentActivities")
        .cloned()
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();
    if let Some(exams) = project
        .get_mut("speakingExams")
        .and_then(Value::as_array_mut)
    {
        for exam in exams {
            let Some(exam) = exam.as_object_mut() else {
                continue;
            };
            let Some(activity_id) = exam
                .get("assessmentActivityId")
                .and_then(Value::as_str)
                .map(str::to_string)
            else {
                continue;
            };
            let applications = application_snapshot
                .iter()
                .find(|activity| {
                    activity.get("id").and_then(Value::as_str) == Some(activity_id.as_str())
                })
                .and_then(|activity| activity.get("classApplications"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let legacy_exam_id = exam
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("legacy")
                .to_string();
            let Some(attempts) = exam.get_mut("attempts").and_then(Value::as_array_mut) else {
                continue;
            };
            for attempt in attempts {
                let Some(attempt) = attempt.as_object_mut() else {
                    continue;
                };
                if attempt
                    .get("classApplicationId")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty())
                {
                    continue;
                }
                let school_class_id = attempt
                    .get("schoolClassId")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let candidates = applications
                    .iter()
                    .filter(|application| {
                        application.get("schoolClassId").and_then(Value::as_str)
                            == Some(school_class_id)
                    })
                    .filter_map(|application| application.get("id").and_then(Value::as_str))
                    .collect::<Vec<_>>();
                if let [application_id] = candidates.as_slice() {
                    attempt.insert(
                        "classApplicationId".to_string(),
                        Value::String((*application_id).to_string()),
                    );
                    changed = true;
                } else if !applications.is_empty() {
                    warnings.push(format!(
                        "{}.speakingExams[{}] attempt için ClassApplication eşleşmesi bulunamadı; unresolved bırakıldı.",
                        project_file.display(),
                        legacy_exam_id
                    ));
                }
            }
        }
    }

    changed
}

fn normalize_school_class_storage(
    project_file: &Path,
    project: &mut Map<String, Value>,
    warnings: &mut Vec<String>,
) -> bool {
    let original_project = project.clone();
    let project_id = project
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("legacy-project")
        .to_string();
    let created_at = project
        .get("createdAt")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let updated_at = project
        .get("updatedAt")
        .and_then(Value::as_str)
        .unwrap_or(&created_at)
        .to_string();

    let legacy_class_names = collect_legacy_class_names(project);
    let mut classes = match project.get("schoolClasses") {
        Some(Value::Array(values)) => values.clone(),
        Some(_) => {
            warnings.push(format!(
                "{}.schoolClasses was not an array; ignored during compatibility loading.",
                project_file.display()
            ));
            Vec::new()
        }
        None => Vec::new(),
    };
    let had_classes = !classes.is_empty();

    for (index, class) in classes.iter_mut().enumerate() {
        let Some(class) = class.as_object_mut() else {
            continue;
        };
        let raw_name = class
            .get("normalizedName")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .or_else(|| class.get("name").and_then(Value::as_str))
            .unwrap_or("");
        let normalized_name =
            normalize_school_class_name(raw_name).unwrap_or_else(|| raw_name.trim().to_uppercase());
        if !normalized_name.is_empty() {
            class.insert("name".to_string(), Value::String(normalized_name.clone()));
            class.insert(
                "normalizedName".to_string(),
                Value::String(normalized_name.clone()),
            );
        }
        let display_name = class
            .get("displayName")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&normalized_name)
            .to_string();
        class.insert("displayName".to_string(), Value::String(display_name));
        if class.get("academicYearId").is_none() {
            if let Some(academic_year) = class.get("academicYear").cloned() {
                class.insert("academicYearId".to_string(), academic_year);
            }
        }
        if class
            .get("id")
            .and_then(Value::as_str)
            .map_or(true, str::is_empty)
        {
            class.insert(
                "id".to_string(),
                Value::String(legacy_storage_id(
                    "legacy-class",
                    &format!("{project_id}:{normalized_name}:{index}"),
                )),
            );
        }
        class
            .entry("displayOrder".to_string())
            .or_insert_with(|| Value::from(index as u64));
        class
            .entry("status".to_string())
            .or_insert_with(|| Value::String("active".to_string()));
        class
            .entry("createdAt".to_string())
            .or_insert_with(|| Value::String(created_at.clone()));
        class
            .entry("updatedAt".to_string())
            .or_insert_with(|| Value::String(updated_at.clone()));
    }

    if classes.is_empty() {
        for (index, normalized_name) in legacy_class_names.keys().enumerate() {
            let (grade_level, section) = split_legacy_class_name(normalized_name);
            let mut class = Map::new();
            class.insert(
                "id".to_string(),
                Value::String(legacy_storage_id(
                    "legacy-class",
                    &format!("{project_id}:{normalized_name}"),
                )),
            );
            class.insert("name".to_string(), Value::String(normalized_name.clone()));
            class.insert(
                "displayName".to_string(),
                Value::String(normalized_name.clone()),
            );
            class.insert(
                "normalizedName".to_string(),
                Value::String(normalized_name.clone()),
            );
            if let Some(grade_level) = grade_level {
                class.insert("gradeLevel".to_string(), Value::from(grade_level));
            }
            if let Some(section) = section {
                class.insert("section".to_string(), Value::String(section));
            }
            class.insert("displayOrder".to_string(), Value::from(index as u64));
            class.insert("status".to_string(), Value::String("active".to_string()));
            class.insert("createdAt".to_string(), Value::String(created_at.clone()));
            class.insert("updatedAt".to_string(), Value::String(updated_at.clone()));
            classes.push(Value::Object(class));
        }
    }

    let class_ids = classes
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|class| {
            Some((
                class.get("normalizedName")?.as_str()?.to_string(),
                class.get("id")?.as_str()?.to_string(),
            ))
        })
        .collect::<BTreeMap<_, _>>();

    let documents = project
        .get("documents")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let legacy_document_id = project
        .get("studentScanDocumentId")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            let student_scan_ids = documents
                .iter()
                .filter_map(Value::as_object)
                .filter(|document| {
                    document.get("role").and_then(Value::as_str) == Some("student_scan")
                })
                .filter_map(|document| document.get("id").and_then(Value::as_str))
                .collect::<Vec<_>>();
            (student_scan_ids.len() == 1).then(|| student_scan_ids[0].to_string())
        });

    let mut batches = match project.get("studentScanBatches") {
        Some(Value::Array(values)) => values.clone(),
        Some(_) => {
            warnings.push(format!(
                "{}.studentScanBatches was not an array; ignored during compatibility loading.",
                project_file.display()
            ));
            Vec::new()
        }
        None => Vec::new(),
    };
    let had_batches = !batches.is_empty();

    if batches.is_empty() {
        if let (Some(document_id), [(_, class_id)]) = (
            legacy_document_id.as_deref(),
            class_ids.iter().collect::<Vec<_>>().as_slice(),
        ) {
            if let Some(document) = documents.iter().find_map(|value| {
                let document = value.as_object()?;
                (document.get("id").and_then(Value::as_str) == Some(document_id))
                    .then_some(document)
            }) {
                let original_file_name = document
                    .get("fileName")
                    .and_then(Value::as_str)
                    .unwrap_or("student-scan.pdf");
                let document_created_at = document
                    .get("addedAt")
                    .and_then(Value::as_str)
                    .unwrap_or(&created_at);
                let batch_id =
                    legacy_storage_id("legacy-batch", &format!("{project_id}:{document_id}"));
                let mut batch = Map::new();
                batch.insert("id".to_string(), Value::String(batch_id));
                batch.insert("classId".to_string(), Value::String((*class_id).clone()));
                batch.insert(
                    "documentId".to_string(),
                    Value::String(document_id.to_string()),
                );
                batch.insert(
                    "originalFileName".to_string(),
                    Value::String(original_file_name.to_string()),
                );
                batch.insert(
                    "displayName".to_string(),
                    Value::String(original_file_name.to_string()),
                );
                if let Some(value) = project.get("studentPagesPerStudent").cloned() {
                    batch.insert("pagesPerStudent".to_string(), value);
                }
                if let Some(value) = project.get("studentGroupingMode").cloned() {
                    batch.insert("groupingMode".to_string(), value);
                }
                if let Some(value) = project.get("studentGroupingCompleteAt").cloned() {
                    batch.insert("groupingCompletedAt".to_string(), value);
                }
                batch.insert(
                    "createdAt".to_string(),
                    Value::String(document_created_at.to_string()),
                );
                batch.insert("updatedAt".to_string(), Value::String(updated_at.clone()));
                batches.push(Value::Object(batch));
            }
        }
    }

    for (index, batch) in batches.iter_mut().enumerate() {
        let Some(batch) = batch.as_object_mut() else {
            continue;
        };
        let document_id = batch
            .get("documentId")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if batch
            .get("id")
            .and_then(Value::as_str)
            .map_or(true, str::is_empty)
        {
            batch.insert(
                "id".to_string(),
                Value::String(legacy_storage_id(
                    "legacy-batch",
                    &format!("{project_id}:{document_id}:{index}"),
                )),
            );
        }
        if let Some(document) = documents.iter().find_map(|value| {
            let document = value.as_object()?;
            (document.get("id").and_then(Value::as_str) == Some(document_id.as_str()))
                .then_some(document)
        }) {
            let file_name = document
                .get("fileName")
                .and_then(Value::as_str)
                .unwrap_or("student-scan.pdf");
            batch
                .entry("originalFileName".to_string())
                .or_insert_with(|| Value::String(file_name.to_string()));
            batch
                .entry("displayName".to_string())
                .or_insert_with(|| Value::String(file_name.to_string()));
            let document_created_at = document
                .get("addedAt")
                .and_then(Value::as_str)
                .unwrap_or(&created_at);
            batch
                .entry("createdAt".to_string())
                .or_insert_with(|| Value::String(document_created_at.to_string()));
        }
        batch
            .entry("updatedAt".to_string())
            .or_insert_with(|| Value::String(updated_at.clone()));
    }

    let batch_bindings = batches
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|batch| {
            Some((
                batch.get("documentId")?.as_str()?.to_string(),
                (
                    batch.get("id")?.as_str()?.to_string(),
                    batch.get("classId")?.as_str()?.to_string(),
                ),
            ))
        })
        .collect::<HashMap<_, _>>();
    let mut assigned_submission_count = 0usize;
    if let Some(submissions) = project
        .get_mut("studentSubmissions")
        .and_then(Value::as_array_mut)
    {
        for submission in submissions {
            let Some(submission) = submission.as_object_mut() else {
                continue;
            };
            let Some(document_id) = submission.get("documentId").and_then(Value::as_str) else {
                continue;
            };
            let Some((batch_id, class_id)) = batch_bindings.get(document_id) else {
                continue;
            };
            let mut assigned = false;
            if submission
                .get("scanBatchId")
                .and_then(Value::as_str)
                .is_none()
            {
                submission.insert("scanBatchId".to_string(), Value::String(batch_id.clone()));
                assigned = true;
            }
            if submission.get("classId").and_then(Value::as_str).is_none() {
                submission.insert("classId".to_string(), Value::String(class_id.clone()));
                assigned = true;
            }
            if submission
                .get("classMembershipSource")
                .and_then(Value::as_str)
                .is_none()
            {
                submission.insert(
                    "classMembershipSource".to_string(),
                    Value::String("inherited_from_batch".to_string()),
                );
                assigned = true;
            }
            assigned_submission_count += usize::from(assigned);
        }
    }

    let created_classes = !had_classes && !classes.is_empty();
    let created_batches = !had_batches && !batches.is_empty();
    project.insert("schoolClasses".to_string(), Value::Array(classes));
    project.insert("studentScanBatches".to_string(), Value::Array(batches));

    if created_classes || created_batches || assigned_submission_count > 0 {
        warnings.push(format!(
            "{}: legacy student scan/class fields were mapped in memory (classes_created={created_classes}; batches_created={created_batches}; submissions_assigned={assigned_submission_count}); OCR and scoring records were not modified.",
            project_file.display()
        ));
    }
    *project != original_project
}

fn collect_legacy_class_names(project: &Map<String, Value>) -> BTreeMap<String, String> {
    let mut names = BTreeMap::new();
    let Some(students) = project.get("students").and_then(Value::as_array) else {
        return names;
    };
    for student in students.iter().filter_map(Value::as_object) {
        let class_name = student
            .get("className")
            .and_then(Value::as_str)
            .or_else(|| {
                student
                    .get("identityOcr")
                    .and_then(Value::as_object)
                    .and_then(|identity| identity.get("className"))
                    .and_then(Value::as_str)
            });
        if let Some(normalized_name) = class_name.and_then(normalize_school_class_name) {
            names.insert(normalized_name.clone(), normalized_name);
        }
    }
    names
}

fn split_legacy_class_name(normalized_name: &str) -> (Option<u32>, Option<String>) {
    let Some((grade, section)) = normalized_name.split_once('-') else {
        return (normalized_name.parse().ok(), None);
    };
    (
        grade.parse().ok(),
        (!section.is_empty()).then(|| section.to_string()),
    )
}

fn legacy_storage_id(prefix: &str, seed: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in seed.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{prefix}-{hash:016x}")
}

/// The speaking engine still uses a runtime aggregate while it processes audio,
/// but persisted ownership belongs to the activity's ClassApplication. These
/// two helpers keep that compatibility projection out of project.json and make
/// the canonical direction explicit.
fn canonicalize_speaking_attempts(project: &mut Project) {
    for activity in &mut project.assessment_activities {
        if activity.assessment_type != crate::domain::assessment::AssessmentType::Speaking {
            continue;
        }
        let activity_id = activity.id.clone();
        for application in &mut activity.class_applications {
            let application_id = application.id.clone();
            let runtime_attempts = project
                .speaking_exams
                .iter()
                .filter(|exam| {
                    exam.assessment_activity_id.as_deref() == Some(activity_id.as_str())
                        || exam.id == activity_id
                })
                .flat_map(|exam| exam.attempts.iter())
                .filter(|attempt| {
                    attempt.class_application_id.as_deref() == Some(application_id.as_str())
                })
                .cloned()
                .collect::<Vec<_>>();
            if !runtime_attempts.is_empty() {
                application.speaking_attempts = runtime_attempts;
            }
        }
        for exam in &mut project.speaking_exams {
            if exam.assessment_activity_id.as_deref() == Some(activity_id.as_str())
                || exam.id == activity_id
            {
                exam.attempts.clear();
            }
        }
    }
}

fn hydrate_speaking_attempts(project: &mut Project) {
    for activity in &project.assessment_activities {
        if activity.assessment_type != crate::domain::assessment::AssessmentType::Speaking {
            continue;
        }
        let Some(exam) = project.speaking_exams.iter_mut().find(|exam| {
            exam.assessment_activity_id.as_deref() == Some(activity.id.as_str())
                || exam.id == activity.id
        }) else {
            continue;
        };
        exam.attempts = activity
            .class_applications
            .iter()
            .flat_map(|application| application.speaking_attempts.iter().cloned())
            .collect();
    }
}

fn school_class_reference_warnings(project_file: &Path, project: &Project) -> Vec<String> {
    let class_ids = project
        .school_classes
        .iter()
        .map(|school_class| school_class.id.as_str())
        .collect::<HashSet<_>>();
    let batch_by_id = project
        .student_scan_batches
        .iter()
        .map(|batch| (batch.id.as_str(), batch))
        .collect::<HashMap<_, _>>();
    let mut warnings = Vec::new();
    for batch in &project.student_scan_batches {
        if !class_ids.contains(batch.class_id.as_str()) {
            warnings.push(format!(
                "{}.studentScanBatches[{}] references unknown classId {:?}; project remained loadable.",
                project_file.display(), batch.id, batch.class_id
            ));
        }
    }
    for submission in &project.student_submissions {
        if let Some(class_id) = submission.class_id.as_deref() {
            if !class_ids.contains(class_id) {
                warnings.push(format!(
                    "{}.studentSubmissions[{}] references unknown classId {:?}; project remained loadable.",
                    project_file.display(), submission.id, class_id
                ));
            }
        }
        if let Some(batch_id) = submission.scan_batch_id.as_deref() {
            match batch_by_id.get(batch_id) {
                None => warnings.push(format!(
                    "{}.studentSubmissions[{}] references unknown scanBatchId {:?}; project remained loadable.",
                    project_file.display(), submission.id, batch_id
                )),
                Some(batch)
                    if submission.class_id.as_deref().is_some_and(|class_id| {
                        class_id != batch.class_id
                    }) =>
                {
                    warnings.push(format!(
                        "{}.studentSubmissions[{}] classId does not match scan batch {}; project remained loadable.",
                        project_file.display(), submission.id, batch.id
                    ));
                }
                Some(_) => {}
            }
        }
    }
    for exam in &project.speaking_exams {
        for class_id in exam.assigned_class_ids() {
            if !class_ids.contains(class_id.as_str()) {
                warnings.push(format!(
                    "{}.speakingExams[{}] references unknown classId {:?}; project remained loadable.",
                    project_file.display(), exam.id, class_id
                ));
            }
        }
    }
    warnings
}

fn normalize_student_answer_ocr_records(
    project_file: &Path,
    project: &mut Map<String, Value>,
    warnings: &mut Vec<String>,
) {
    let Some(records) = project
        .get_mut("studentAnswerOcrRecords")
        .and_then(Value::as_array_mut)
    else {
        return;
    };

    for (index, record) in records.iter_mut().enumerate() {
        let Some(record) = record.as_object_mut() else {
            continue;
        };
        let base_path = format!(
            "{}.studentAnswerOcrRecords[{}]",
            project_file.display(),
            index
        );
        normalize_ocr_metadata_object(record, &base_path, warnings);
    }
}

fn normalize_student_identity_records(
    project_file: &Path,
    project: &mut Map<String, Value>,
    warnings: &mut Vec<String>,
) {
    let Some(students) = project.get_mut("students").and_then(Value::as_array_mut) else {
        return;
    };

    for (student_index, student) in students.iter_mut().enumerate() {
        let Some(student) = student.as_object_mut() else {
            continue;
        };
        let Some(identity) = student
            .get_mut("identityOcr")
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        let base_path = format!(
            "{}.students[{}].identityOcr",
            project_file.display(),
            student_index
        );
        normalize_ocr_metadata_object(identity, &base_path, warnings);
    }
}

fn normalize_scoring_records(
    project_file: &Path,
    project: &mut Map<String, Value>,
    warnings: &mut Vec<String>,
) -> bool {
    let mut changed = false;
    let Some(records) = project
        .get_mut("scoringRecords")
        .and_then(Value::as_array_mut)
    else {
        return false;
    };

    for (index, record) in records.iter_mut().enumerate() {
        let Some(record) = record.as_object_mut() else {
            continue;
        };
        let base_path = format!("{}.scoringRecords[{}]", project_file.display(), index);
        if let Some(run_id) = record.get_mut("runId") {
            if run_id.as_str().is_some_and(|value| value.trim().is_empty()) {
                warnings.push(format!(
                    "{base_path}.runId was empty; kept as legacy history entry."
                ));
            }
        }

        let decision_state = record
            .get("decisionState")
            .and_then(Value::as_str)
            .filter(|value| {
                matches!(
                    *value,
                    "model_candidate"
                        | "deterministic_accepted"
                        | "provisional"
                        | "auto_accepted"
                        | "teacher_approved"
                        | "rejected"
                        | "failed"
                )
            });
        if decision_state.is_none() {
            let teacher_review_status = record
                .get("teacherReviewStatus")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let scoring_applied = record
                .get("scoringApplied")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let needs_review = record
                .get("needsReview")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let has_manual_score = record
                .get("teacherManualScore")
                .is_some_and(|value| !value.is_null());
            let derived_state = if teacher_review_status == "invalidated" {
                "rejected"
            } else if scoring_applied && needs_review {
                "provisional"
            } else if has_manual_score || matches!(teacher_review_status, "approved" | "edited") {
                "teacher_approved"
            } else if scoring_applied {
                "auto_accepted"
            } else {
                "failed"
            };
            record.insert(
                "decisionState".to_string(),
                Value::String(derived_state.to_string()),
            );
            changed = true;
            warnings.push(format!(
                "{base_path}.decisionState eski notlandırma alanlarından {derived_state} olarak taşındı."
            ));
        }
    }
    changed
}

fn normalize_ocr_metadata_object(
    record: &mut Map<String, Value>,
    base_path: &str,
    warnings: &mut Vec<String>,
) {
    let fallback_mode = "clean_grayscale";
    normalize_mode_field(
        record.get_mut("preprocessMode"),
        &format!("{base_path}.preprocessMode"),
        fallback_mode,
        warnings,
    );

    if let Some(variants) = record
        .get_mut("availablePreprocessVariants")
        .and_then(Value::as_array_mut)
    {
        for (index, variant) in variants.iter_mut().enumerate() {
            normalize_mode_field(
                Some(variant),
                &format!("{base_path}.availablePreprocessVariants[{index}]"),
                fallback_mode,
                warnings,
            );
        }
    }

    if let Some(diagnostics) = record
        .get_mut("preprocessDiagnostics")
        .and_then(Value::as_array_mut)
    {
        for (index, diagnostic) in diagnostics.iter_mut().enumerate() {
            if let Some(diagnostic) = diagnostic.as_object_mut() {
                normalize_mode_field(
                    diagnostic.get_mut("mode"),
                    &format!("{base_path}.preprocessDiagnostics[{index}].mode"),
                    fallback_mode,
                    warnings,
                );
            }
        }
    }

    if let Some(structured_answer) = record.get("structuredAnswer") {
        let known_kind = structured_answer
            .get("kind")
            .and_then(Value::as_str)
            .is_some_and(|kind| {
                matches!(
                    kind,
                    "multiple_choice"
                        | "matching"
                        | "ordered_slots"
                        | "numeric"
                        | "table"
                        | "correction_table"
                        | "sentence_annotation"
                        | "grammar_analysis"
                        | "open_text"
                        | "text"
                )
            });
        if !known_kind {
            record.insert("needsReview".to_string(), Value::Bool(true));
            append_string_array_value(record, "reviewReasons", "structured_answer_legacy_unparsed");
            append_string_array_value(record, "warnings", "structured_answer_legacy_unparsed");
            warnings.push(format!(
                "{base_path}.structuredAnswer eski arbitrary JSON olarak review-only salvage biçiminde korunacak."
            ));
        }
    }
}

fn append_string_array_value(record: &mut Map<String, Value>, field: &str, value: &str) {
    let values = record
        .entry(field.to_string())
        .or_insert_with(|| Value::Array(vec![]));
    if let Some(values) = values.as_array_mut() {
        if !values.iter().any(|entry| entry.as_str() == Some(value)) {
            values.push(Value::String(value.to_string()));
        }
    }
}

fn normalize_mode_field(
    value: Option<&mut Value>,
    path: &str,
    fallback: &str,
    warnings: &mut Vec<String>,
) {
    let Some(value) = value else {
        return;
    };
    let Some(mode_text) = value.as_str() else {
        *value = Value::String(fallback.to_string());
        warnings.push(format!("{path} was not a string; defaulted to {fallback}."));
        return;
    };
    if valid_preprocess_mode(mode_text) {
        return;
    }
    warnings.push(format!(
        "{path} had unsupported preprocess mode {mode_text:?}; defaulted to {fallback}."
    ));
    *value = Value::String(fallback.to_string());
}

fn valid_preprocess_mode(value: &str) -> bool {
    matches!(
        value,
        "original"
            | "clean_grayscale"
            | "handwriting_enhanced"
            | "high_contrast"
            | "high_contrast_bw"
            | "high_contrast_bw_optional"
    )
}

fn project_to_list_item(project: &Project, project_dir: &Path) -> ProjectListItem {
    let question_count = project.questions.len() as u32;
    let question_text_ready = project
        .questions
        .iter()
        .filter(|question| is_question_text_ready(&question.question_text))
        .count() as u32;
    let rubric_ready = project
        .questions
        .iter()
        .filter(|question| {
            matches!(
                question.rubric.status,
                RubricStatus::Suggested
                    | RubricStatus::Imported
                    | RubricStatus::Manual
                    | RubricStatus::Confirmed
            )
        })
        .count() as u32;

    ProjectListItem {
        id: project.id.clone(),
        name: project.name.clone(),
        path: project_dir.to_string_lossy().to_string(),
        created_at: Some(project.created_at.clone()),
        updated_at: Some(project.updated_at.clone()),
        question_count: Some(question_count),
        document_roles: project
            .documents
            .iter()
            .map(|document| document_role_key(&document.role))
            .collect(),
        status_summary: ProjectStatusSummary {
            has_exam_source: project
                .documents
                .iter()
                .any(|document| matches!(document.role, DocumentRole::ExamSource)),
            has_answer_key_or_rubric: project.documents.iter().any(|document| {
                matches!(
                    document.role,
                    DocumentRole::AnswerKey | DocumentRole::Rubric
                )
            }),
            has_student_scan: project
                .documents
                .iter()
                .any(|document| matches!(document.role, DocumentRole::StudentScan)),
            question_text_coverage: Some(format!("{question_text_ready}/{question_count}")),
            rubric_coverage: Some(format!("{rubric_ready}/{question_count}")),
        },
    }
}

fn document_role_key(role: &DocumentRole) -> String {
    match role {
        DocumentRole::StudentScan => "student_scan".to_string(),
        DocumentRole::ExamSource => "exam_source".to_string(),
        DocumentRole::AnswerKey => "answer_key".to_string(),
        DocumentRole::Rubric => "rubric".to_string(),
        DocumentRole::Export => "export".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::assessment::{
        AssessmentActivity, AssessmentStatus, AssessmentType, ClassApplication,
        ClassApplicationStatus, SpeakingConfigurationSnapshot, WorkflowFamily,
    };
    use crate::domain::document::Document;
    use crate::domain::project::Project;
    use crate::domain::question::{
        AnswerType, CropTemplate, Question, TextFieldSource, TextFieldState, TextFieldStatus,
    };
    use crate::domain::rubric::{RubricCriterion, RubricState};
    use crate::domain::school_class::{SchoolClass, SchoolClassStatus};
    use crate::domain::scoring::scoring_active_run_id;
    use crate::domain::speaking::{new_exam, SpeakingExamType};
    use crate::domain::student::{
        OcrImagePreprocessDiagnostics, OcrImagePreprocessMode, PageGroupingMode, Student,
        StudentAnswerOcrRecord, StudentAnswerOcrStatus, StudentSubmission,
    };
    use std::fs;
    use std::path::PathBuf;

    fn temp_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("rubrika-project-store-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn sample_project(root_path: &Path, name: &str, updated_at: &str) -> Project {
        Project {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: updated_at.to_string(),
            root_path: root_path.to_string_lossy().to_string(),
            storage_revision: 0,
            academic_year_id: None,
            course_id: None,
            course_name: None,
            sections: vec![],
            students: vec![Student {
                id: Uuid::new_v4().to_string(),
                display_name: Some("Öğrenci".to_string()),
                number: Some("1".to_string()),
                class_name: Some("11A".to_string()),
                warnings: vec![],
                identity_ocr: None,
            }],
            school_classes: vec![],
            teaching_assignments: vec![],
            assessment_activities: vec![],
            student_scan_batches: vec![],
            student_submissions: vec![StudentSubmission {
                id: Uuid::new_v4().to_string(),
                student_id: Uuid::new_v4().to_string(),
                document_id: Uuid::new_v4().to_string(),
                class_id: None,
                scan_batch_id: None,
                class_membership_source: None,
                page_numbers: vec![1],
                status: crate::domain::student::StudentSubmissionStatus::Grouped,
                answer_slots: vec![],
                warnings: vec![],
                updated_at: None,
            }],
            student_scan_document_id: None,
            student_grouping_mode: Some(PageGroupingMode::OnePdfOneStudent),
            student_pages_per_student: None,
            student_grouping_complete_at: None,
            expected_question_count: Some(2),
            exam_package_freeze: None,
            scoring_records: vec![],
            scoring_anchors: vec![],
            speaking_exams: vec![],
            latest_scoring_run_id: None,
            student_answer_ocr_records: vec![],
            student_answer_ocr_generations: vec![],
            student_answer_crop_template: Default::default(),
            student_identity_crop_template: None,
            documents: vec![
                Document {
                    id: Uuid::new_v4().to_string(),
                    role: DocumentRole::ExamSource,
                    file_name: "exam.pdf".to_string(),
                    stored_path: "exam.pdf".to_string(),
                    page_count: 2,
                    added_at: "2026-01-01T00:00:00Z".to_string(),
                    checksum: None,
                    preview: None,
                },
                Document {
                    id: Uuid::new_v4().to_string(),
                    role: DocumentRole::AnswerKey,
                    file_name: "rubric.pdf".to_string(),
                    stored_path: "rubric.pdf".to_string(),
                    page_count: 1,
                    added_at: "2026-01-01T00:00:00Z".to_string(),
                    checksum: None,
                    preview: None,
                },
                Document {
                    id: Uuid::new_v4().to_string(),
                    role: DocumentRole::StudentScan,
                    file_name: "scan.pdf".to_string(),
                    stored_path: "scan.pdf".to_string(),
                    page_count: 3,
                    added_at: "2026-01-01T00:00:00Z".to_string(),
                    checksum: None,
                    preview: None,
                },
            ],
            questions: vec![
                Question {
                    id: Uuid::new_v4().to_string(),
                    number: 1,
                    max_score: 5.0,
                    answer_type: AnswerType::GeneralText,
                    question_text: TextFieldState {
                        value: "Soru 1".to_string(),
                        source: TextFieldSource::Manual,
                        status: TextFieldStatus::Confirmed,
                        confidence: None,
                        warnings: vec![],
                        updated_at: None,
                    },
                    rubric: RubricState {
                        status: RubricStatus::Confirmed,
                        source: None,
                        max_score: Some(5.0),
                        expected_answer: Some("Cevap".to_string()),
                        key_concepts: vec![],
                        criteria: vec![RubricCriterion {
                            id: Uuid::new_v4().to_string(),
                            label: "Kriter".to_string(),
                            description: "Açıklama".to_string(),
                            points: 5.0,
                            levels: vec![],
                        }],
                        partial_credit_hints: vec![],
                        zero_score_conditions: vec![],
                        common_mistakes: vec![],
                        warnings: vec![],
                        updated_at: None,
                    },
                    crop_template: Some(CropTemplate {
                        x: 1.0,
                        y: 1.0,
                        width: 10.0,
                        height: 10.0,
                        page_index: 0,
                    }),
                },
                Question {
                    id: Uuid::new_v4().to_string(),
                    number: 2,
                    max_score: 5.0,
                    answer_type: AnswerType::Essay,
                    question_text: TextFieldState {
                        value: String::new(),
                        source: TextFieldSource::Unknown,
                        status: TextFieldStatus::Missing,
                        confidence: None,
                        warnings: vec![],
                        updated_at: None,
                    },
                    rubric: RubricState {
                        status: RubricStatus::Missing,
                        source: None,
                        max_score: None,
                        expected_answer: None,
                        key_concepts: vec![],
                        criteria: vec![],
                        partial_credit_hints: vec![],
                        zero_score_conditions: vec![],
                        common_mistakes: vec![],
                        warnings: vec![],
                        updated_at: None,
                    },
                    crop_template: None,
                },
            ],
            workflow: WorkflowSnapshot {
                current_stage: WorkflowStage::DocumentsMissing,
                current_stage_label: "Belgeler Eksik".to_string(),
                blocking_reasons: vec![],
                next_actions: vec![],
                summary: crate::domain::workflow::WorkflowSummary::default(),
            },
        }
    }

    fn write_project(root: &Path, project: &Project) {
        fs::create_dir_all(root).unwrap();
        fs::write(
            root.join("project.json"),
            serde_json::to_string_pretty(project).unwrap(),
        )
        .unwrap();
    }

    fn write_project_value(root: &Path, value: &serde_json::Value) {
        fs::create_dir_all(root).unwrap();
        fs::write(
            root.join("project.json"),
            serde_json::to_string_pretty(value).unwrap(),
        )
        .unwrap();
    }

    fn speaking_configuration(task_text: &str) -> SpeakingConfigurationSnapshot {
        SpeakingConfigurationSnapshot {
            speaking_type: "prepared".to_string(),
            task_text: task_text.to_string(),
            target_duration_seconds: 180,
            min_duration_seconds: 120,
            max_duration_seconds: 240,
            rubric_version: "rubric-v1".to_string(),
            scoring_policy_version: "policy-v1".to_string(),
            cleanup_prompt_version: "cleanup-v1".to_string(),
            evaluation_prompt_version: "evaluation-v1".to_string(),
            frozen_model_file_hash: None,
            rubric_snapshot: serde_json::json!({"version": "rubric-v1"}),
        }
    }

    fn speaking_activity(
        id: &str,
        title: &str,
        task_text: &str,
        class_applications: Vec<ClassApplication>,
    ) -> AssessmentActivity {
        AssessmentActivity {
            id: id.to_string(),
            academic_year_id: "2026-2027".to_string(),
            course_id: "turkish".to_string(),
            course_name: "Türk Dili ve Edebiyatı".to_string(),
            title: title.to_string(),
            grade_level: 11,
            term: 1,
            assessment_type: AssessmentType::Speaking,
            workflow_family: WorkflowFamily::Speaking,
            sequence_number: 1,
            status: AssessmentStatus::Draft,
            common_document_ids: vec![],
            listening_details: None,
            speaking_configuration: Some(speaking_configuration(task_text)),
            performance_details: None,
            class_applications,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn class_application(
        activity_id: &str,
        application_id: &str,
        school_class_id: &str,
    ) -> ClassApplication {
        ClassApplication {
            id: application_id.to_string(),
            activity_id: activity_id.to_string(),
            school_class_id: school_class_id.to_string(),
            scheduled_at: None,
            application_date: None,
            status: ClassApplicationStatus::Scheduled,
            notes: None,
            document_ids: vec![],
            student_scope_ids: vec![],
            speaking_attempts: vec![],
            performance_assessments: vec![],
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn list_projects_in_dir_lists_and_sorts_newest_first() {
        let root = temp_root();
        let older = root.join("older");
        let newer = root.join("newer");
        write_project(
            &older,
            &sample_project(&older, "Older", "2026-01-01T00:00:00Z"),
        );
        write_project(
            &newer,
            &sample_project(&newer, "Newer", "2026-01-02T00:00:00Z"),
        );

        let report = ProjectStore::list_projects_in_dir(&root);

        assert_eq!(report.projects.len(), 2);
        assert_eq!(report.projects[0].name, "Newer");
        assert_eq!(report.projects[1].name, "Older");
        assert!(report.projects[0].status_summary.has_exam_source);
        assert!(report.projects[0].status_summary.has_student_scan);
        assert_eq!(
            report.projects[0]
                .status_summary
                .question_text_coverage
                .as_deref(),
            Some("1/2")
        );
        assert_eq!(
            report.projects[0].status_summary.rubric_coverage.as_deref(),
            Some("1/2")
        );
    }

    #[test]
    fn list_projects_in_dir_skips_bad_projects() {
        let root = temp_root();
        let valid = root.join("valid");
        let broken = root.join("broken");
        write_project(
            &valid,
            &sample_project(&valid, "Valid", "2026-01-01T00:00:00Z"),
        );
        fs::create_dir_all(&broken).unwrap();
        fs::write(broken.join("project.json"), "{not json").unwrap();

        let report = ProjectStore::list_projects_in_dir(&root);

        assert_eq!(report.projects.len(), 1);
        assert_eq!(report.projects[0].name, "Valid");
        assert_eq!(report.skipped_projects.len(), 1);
        assert!(report.skipped_projects[0]
            .reason
            .contains("Invalid project.json"));
    }

    #[test]
    fn open_project_at_path_reads_project() {
        let root = temp_root();
        let project = sample_project(&root, "Open Me", "2026-01-01T00:00:00Z");
        write_project(&root, &project);

        let reopened = ProjectStore::open_project_at_path(&root).unwrap();
        assert_eq!(reopened.name, "Open Me");
        assert_eq!(
            reopened.root_path,
            std::fs::canonicalize(&root)
                .unwrap()
                .to_string_lossy()
                .to_string()
        );
        assert_eq!(reopened.questions.len(), 2);
    }

    #[test]
    fn speaking_attempts_persist_under_class_application_and_reload_without_duplication() {
        let root = temp_root();
        let store = ProjectStore::new();
        let mut project = sample_project(&root, "Canonical speaking", "2026-01-01T00:00:00Z");
        write_project(&root, &project);
        store
            .open_project_with_warnings(root.to_string_lossy().to_string())
            .unwrap();
        project.school_classes.push(SchoolClass {
            id: "class-11-a".to_string(),
            name: "11A".to_string(),
            display_name: "11A".to_string(),
            normalized_name: "11-A".to_string(),
            academic_year: Some("2026-2027".to_string()),
            academic_year_id: Some("2026-2027".to_string()),
            grade_level: Some(11),
            section: Some("A".to_string()),
            display_order: 0,
            status: SchoolClassStatus::Active,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        });
        let mut application = class_application("activity-1", "application-1", "class-11-a");
        let attempt: crate::domain::speaking::SpeakingAttempt =
            serde_json::from_value(serde_json::json!({
                "id": "attempt-1",
                "assessmentActivityId": "activity-1",
                "classApplicationId": "application-1",
                "schoolClassId": "class-11-a",
                "examId": "activity-1",
                "studentId": project.students[0].id,
                "attemptNumber": 1,
                "state": "teacher_review",
                "startedAt": "2026-01-01T00:00:00Z"
            }))
            .expect("minimal speaking attempt should deserialize with defaults");
        application.speaking_attempts.push(attempt);
        project.assessment_activities.push(speaking_activity(
            "activity-1",
            "1. Konuşma",
            "Bir anını anlat.",
            vec![application],
        ));
        project.speaking_exams.push(new_exam(
            "1. Konuşma".to_string(),
            vec![],
            SpeakingExamType::Prepared,
            "Bir anını anlat.".to_string(),
            180,
            120,
            240,
        ));
        project.speaking_exams[0].id = "activity-1".to_string();
        project.speaking_exams[0].assessment_activity_id = Some("activity-1".to_string());

        store
            .save_project(&project)
            .expect("canonical project should save");
        let persisted_json = fs::read_to_string(root.join("project.json")).unwrap();
        let persisted_value: serde_json::Value = serde_json::from_str(&persisted_json).unwrap();
        assert_eq!(
            persisted_value["assessmentActivities"][0]["classApplications"][0]["speakingAttempts"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(persisted_value["speakingExams"][0]["attempts"]
            .as_array()
            .unwrap()
            .is_empty());

        let reopened_store = ProjectStore::new();
        let reopened = reopened_store
            .open_project_with_mode(
                root.to_string_lossy().to_string(),
                ProjectOpenMode::InspectReadOnly,
            )
            .map(|(project, _)| project)
            .expect("canonical project should reload");
        assert_eq!(reopened.assessment_activities.len(), 1);
        assert_eq!(
            reopened.assessment_activities[0].class_applications.len(),
            1
        );
        assert_eq!(
            reopened.assessment_activities[0].class_applications[0]
                .speaking_attempts
                .len(),
            1
        );
        let second_reload = ProjectStore::open_project_at_path(&root).unwrap();
        assert_eq!(
            second_reload.assessment_activities[0]
                .class_applications
                .len(),
            1
        );
    }

    #[test]
    fn legacy_assigned_class_ids_migrate_to_one_class_application_idempotently() {
        let root = temp_root();
        let mut project = sample_project(&root, "Legacy speaking", "2026-01-01T00:00:00Z");
        project.school_classes.push(SchoolClass {
            id: "class-11-a".to_string(),
            name: "11A".to_string(),
            display_name: "11A".to_string(),
            normalized_name: "11-A".to_string(),
            academic_year: Some("2026-2027".to_string()),
            academic_year_id: Some("2026-2027".to_string()),
            grade_level: Some(11),
            section: Some("A".to_string()),
            display_order: 0,
            status: SchoolClassStatus::Active,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        });
        project.assessment_activities.push(speaking_activity(
            "activity-legacy",
            "1. Konuşma",
            "Bir anını anlat.",
            vec![],
        ));
        let mut legacy_exam = new_exam(
            "1. Konuşma".to_string(),
            vec!["class-11-a".to_string()],
            SpeakingExamType::Prepared,
            "Bir anını anlat.".to_string(),
            180,
            120,
            240,
        );
        legacy_exam.id = "legacy-exam-1".to_string();
        let mut value = serde_json::to_value(&project).unwrap();
        value["speakingExams"] = serde_json::json!([legacy_exam]);
        write_project_value(&root, &value);

        let store = ProjectStore::new();
        let (first, first_warnings) = store
            .open_project_with_warnings(root.to_string_lossy().to_string())
            .expect("legacy project should remain loadable");
        assert_eq!(first.assessment_activities[0].class_applications.len(), 1);
        assert!(first_warnings
            .iter()
            .any(|warning| warning.contains("backup")));
        let persisted: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("project.json")).unwrap()).unwrap();
        assert!(persisted["speakingExams"][0]["assignedClassIds"]
            .as_array()
            .unwrap()
            .is_empty());

        let (second, _) = ProjectStore::new()
            .open_project_with_warnings(root.to_string_lossy().to_string())
            .expect("migrated project should reload");
        assert_eq!(second.assessment_activities[0].class_applications.len(), 1);
    }

    #[test]
    fn ambiguous_legacy_speaking_relation_stays_unresolved() {
        let root = temp_root();
        let mut project = sample_project(&root, "Ambiguous speaking", "2026-01-01T00:00:00Z");
        project.school_classes.push(SchoolClass {
            id: "class-11-a".to_string(),
            name: "11A".to_string(),
            display_name: "11A".to_string(),
            normalized_name: "11-A".to_string(),
            academic_year: Some("2026-2027".to_string()),
            academic_year_id: Some("2026-2027".to_string()),
            grade_level: Some(11),
            section: Some("A".to_string()),
            display_order: 0,
            status: SchoolClassStatus::Active,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        });
        project.assessment_activities.extend([
            speaking_activity("activity-a", "1. Konuşma", "Aynı görev", vec![]),
            speaking_activity("activity-b", "1. Konuşma", "Aynı görev", vec![]),
        ]);
        let legacy_exam = new_exam(
            "1. Konuşma".to_string(),
            vec!["class-11-a".to_string()],
            SpeakingExamType::Prepared,
            "Aynı görev".to_string(),
            180,
            120,
            240,
        );
        let mut value = serde_json::to_value(&project).unwrap();
        value["speakingExams"] = serde_json::json!([legacy_exam]);
        write_project_value(&root, &value);

        let (reopened, warnings) = ProjectStore::new()
            .open_project_with_warnings(root.to_string_lossy().to_string())
            .expect("ambiguous legacy project should remain loadable");
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("birden fazla")));
        assert!(reopened
            .assessment_activities
            .iter()
            .all(|activity| activity.class_applications.is_empty()));
        let persisted: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("project.json")).unwrap()).unwrap();
        assert_eq!(
            persisted["speakingExams"][0]["assignedClassIds"][0],
            "class-11-a"
        );
    }

    #[test]
    fn missing_legacy_speaking_class_stays_loadable_and_preserves_legacy_relation() {
        let root = temp_root();
        let mut project = sample_project(&root, "Missing legacy class", "2026-01-01T00:00:00Z");
        project.assessment_activities.push(speaking_activity(
            "activity-missing-class",
            "1. Konuşma",
            "Görev",
            vec![],
        ));
        let legacy_exam = new_exam(
            "1. Konuşma".to_string(),
            vec!["missing-class".to_string()],
            SpeakingExamType::Prepared,
            "Görev".to_string(),
            180,
            120,
            240,
        );
        let mut value = serde_json::to_value(&project).unwrap();
        value["speakingExams"] = serde_json::json!([legacy_exam]);
        write_project_value(&root, &value);

        let (reopened, warnings) = ProjectStore::new()
            .open_project_with_warnings(root.to_string_lossy().to_string())
            .expect("missing legacy class must not block project load");
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("bulunamadı")));
        assert!(reopened
            .assessment_activities
            .iter()
            .all(|activity| activity.class_applications.is_empty()));
        let persisted: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("project.json")).unwrap()).unwrap();
        assert_eq!(
            persisted["speakingExams"][0]["assignedClassIds"][0],
            "missing-class"
        );
    }

    #[test]
    fn open_project_at_path_accepts_legacy_workflow_summary_null() {
        let root = temp_root();
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("project.json"),
            format!(
                r#"{{
                    "id":"{}",
                    "name":"Legacy Null",
                    "createdAt":"2026-01-01T00:00:00Z",
                    "updatedAt":"2026-01-01T00:00:00Z",
                    "rootPath":"{}",
                    "documents":[],
                    "questions":[],
                    "workflow":{{
                        "currentStage":"documents_missing",
                        "blockingReasons":[],
                        "nextActions":[],
                        "summary":null
                    }}
                }}"#,
                Uuid::new_v4(),
                root.to_string_lossy()
            ),
        )
        .unwrap();

        let reopened = ProjectStore::open_project_at_path(&root).unwrap();
        assert_eq!(reopened.workflow.current_stage_label, "Belgeler Eksik");
        assert_eq!(
            reopened.workflow.summary.readiness,
            crate::domain::workflow::WorkflowReadiness::default()
        );
    }

    #[test]
    fn open_project_at_path_accepts_legacy_workflow_summary_string_and_missing_fields() {
        let root = temp_root();
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("project.json"),
            format!(
                r#"{{
                    "id":"{}",
                    "name":"Legacy String",
                    "createdAt":"2026-01-01T00:00:00Z",
                    "updatedAt":"2026-01-01T00:00:00Z",
                    "rootPath":"{}",
                    "documents":[],
                    "questions":[],
                    "workflow":{{
                        "currentStage":"question_text_extraction_running",
                        "blockingReasons":[],
                        "nextActions":[],
                        "summary":"Soru metni çıkarımı çalışıyor."
                    }}
                }}"#,
                Uuid::new_v4(),
                root.to_string_lossy()
            ),
        )
        .unwrap();

        let reopened = ProjectStore::open_project_at_path(&root).unwrap();
        assert_eq!(reopened.workflow.current_stage_label, "Belgeler Eksik");
        assert_eq!(
            reopened.workflow.summary.readiness,
            crate::domain::workflow::WorkflowReadiness::default()
        );
    }

    #[test]
    fn open_project_at_path_invalid_path_returns_error() {
        let root = temp_root();
        let result = ProjectStore::open_project_at_path(&root);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, AppErrorCode::ProjectNotFound);
    }

    #[test]
    fn open_project_at_path_reports_serde_path_in_technical_details() {
        let root = temp_root();
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("project.json"),
            r#"{
                "id":"p1",
                "name":"Broken",
                "createdAt":"2026-01-01T00:00:00Z",
                "updatedAt":"2026-01-01T00:00:00Z",
                "rootPath":"/tmp/broken",
                "documents":[],
                "questions":[],
                "workflow":{
                    "currentStage":"documents_missing",
                    "blockingReasons":[],
                    "nextActions":[],
                    "summary":42
                }
            }"#,
        )
        .unwrap();

        let result = ProjectStore::open_project_at_path(&root);
        let error = result.unwrap_err();
        assert_eq!(error.code, AppErrorCode::ProjectLoadFailed);
        let details = error.technical_details.unwrap();
        assert!(details.contains("project_file="));
        assert!(details.contains("path=workflow.summary"));
    }

    #[test]
    fn open_project_at_path_accepts_new_preprocess_modes_and_missing_metadata() {
        let root = temp_root();
        let mut project = sample_project(&root, "OCR Compat", "2026-01-01T00:00:00Z");
        let mut record = StudentAnswerOcrRecord::default();
        let now = chrono::Utc::now();
        record.id = Uuid::new_v4().to_string();
        record.submission_id = project.student_submissions[0].id.clone();
        record.question_id = project.questions[0].id.clone();
        record.question_number = 1;
        record.source_page_numbers = vec![1];
        record.source_image_refs = vec!["source.png".to_string()];
        record.crop_refs = vec!["crop.png".to_string()];
        record.original_crop_refs = vec!["crop.png".to_string()];
        record.preprocessed_crop_refs = vec!["handwriting.png".to_string()];
        record.model_input_crop_ref = Some("handwriting.png".to_string());
        record.preprocess_mode = Some(OcrImagePreprocessMode::HandwritingEnhanced);
        record.preprocess_version = Some("ocr_image_preprocess_v2".to_string());
        record.preprocess_applied = true;
        record.preprocess_diagnostics = vec![OcrImagePreprocessDiagnostics {
            mode: OcrImagePreprocessMode::HandwritingEnhanced,
            preprocess_version: "ocr_image_preprocess_v2".to_string(),
            source_image_path: "source.png".to_string(),
            output_image_path: "handwriting.png".to_string(),
            source_width: 1,
            source_height: 1,
            output_width: 1,
            output_height: 1,
            source_bytes: 1,
            output_bytes: 1,
            cache_hit: false,
            applied: true,
            warnings: vec![],
            error_message: None,
            technical_details: None,
        }];
        record.available_preprocess_variants = vec![
            OcrImagePreprocessMode::Original,
            OcrImagePreprocessMode::CleanGrayscale,
            OcrImagePreprocessMode::HandwritingEnhanced,
            OcrImagePreprocessMode::HighContrast,
            OcrImagePreprocessMode::HighContrastBw,
        ];
        record.full_page_preview_refs = vec!["page.png".to_string()];
        record.answer_text = "cevap".to_string();
        record.status = StudentAnswerOcrStatus::Succeeded;
        record.prompt_version = "student_answer_ocr_v2".to_string();
        record.created_at = now;
        record.updated_at = now;
        project.student_answer_ocr_records = vec![record];
        let value = serde_json::to_value(&project).unwrap();
        write_project_value(&root, &value);

        let reopened = ProjectStore::open_project_at_path(&root).unwrap();
        let record = &reopened.student_answer_ocr_records[0];
        assert_eq!(
            record.preprocess_mode,
            Some(OcrImagePreprocessMode::HandwritingEnhanced)
        );
        assert_eq!(
            record.preprocess_version.as_deref(),
            Some("ocr_image_preprocess_v2")
        );
        assert_eq!(
            record.model_input_crop_ref.as_deref(),
            Some("handwriting.png")
        );
        assert_eq!(record.available_preprocess_variants.len(), 5);
    }

    #[test]
    fn open_project_at_path_accepts_high_contrast_bw_optional_alias_and_unknown_mode_falls_back() {
        let root = temp_root();
        let mut project = sample_project(&root, "OCR Alias", "2026-01-01T00:00:00Z");
        let mut record = StudentAnswerOcrRecord::default();
        let now = chrono::Utc::now();
        record.id = Uuid::new_v4().to_string();
        record.submission_id = project.student_submissions[0].id.clone();
        record.question_id = project.questions[0].id.clone();
        record.question_number = 1;
        record.source_page_numbers = vec![1];
        record.answer_text = "cevap".to_string();
        record.status = StudentAnswerOcrStatus::Succeeded;
        record.prompt_version = "student_answer_ocr_v2".to_string();
        record.created_at = now;
        record.updated_at = now;
        project.student_answer_ocr_records = vec![record];

        let mut value = serde_json::to_value(&project).unwrap();
        let record_value = value
            .get_mut("studentAnswerOcrRecords")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|records| records.get_mut(0))
            .and_then(serde_json::Value::as_object_mut)
            .unwrap();
        record_value.insert(
            "preprocessMode".to_string(),
            serde_json::Value::String("high_contrast_bw_optional".to_string()),
        );
        record_value.insert(
            "availablePreprocessVariants".to_string(),
            serde_json::json!(["original", "high_contrast_bw_optional"]),
        );
        record_value.insert(
            "preprocessDiagnostics".to_string(),
            serde_json::json!([
                {
                    "mode": "high_contrast_bw_optional",
                    "preprocessVersion": "ocr_image_preprocess_v2",
                    "sourceImagePath": "source.png",
                    "outputImagePath": "bw.png",
                    "sourceWidth": 1,
                    "sourceHeight": 1,
                    "outputWidth": 1,
                    "outputHeight": 1,
                    "sourceBytes": 1,
                    "outputBytes": 1,
                    "cacheHit": false,
                    "applied": true,
                    "warnings": [],
                    "errorMessage": null,
                    "technicalDetails": null
                }
            ]),
        );
        write_project_value(&root, &value);

        let reopened = ProjectStore::open_project_at_path(&root).unwrap();
        assert_eq!(
            reopened.student_answer_ocr_records[0].preprocess_mode,
            Some(OcrImagePreprocessMode::HighContrastBw)
        );

        let mut value = serde_json::to_value(&project).unwrap();
        let record_value = value
            .get_mut("studentAnswerOcrRecords")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|records| records.get_mut(0))
            .and_then(serde_json::Value::as_object_mut)
            .unwrap();
        record_value.insert(
            "preprocessMode".to_string(),
            serde_json::Value::String("mystery_mode".to_string()),
        );
        record_value.insert(
            "availablePreprocessVariants".to_string(),
            serde_json::json!(["mystery_mode"]),
        );
        write_project_value(&root, &value);

        let loaded = ProjectStore::new()
            .open_project_with_warnings(root.to_string_lossy().to_string())
            .unwrap();
        assert_eq!(
            loaded.0.student_answer_ocr_records[0].preprocess_mode,
            Some(OcrImagePreprocessMode::CleanGrayscale)
        );
        assert!(loaded
            .1
            .iter()
            .any(|warning| warning.contains("unsupported preprocess mode")));
    }

    #[test]
    fn open_project_at_path_accepts_missing_ocr_metadata_and_legacy_scoring_run_ids() {
        let root = temp_root();
        let mut project = sample_project(&root, "Legacy OCR", "2026-01-01T00:00:00Z");
        project.latest_scoring_run_id = None;
        project.scoring_records = vec![crate::domain::scoring::ScoringRecord {
            id: Uuid::new_v4().to_string(),
            run_id: String::new(),
            submission_id: project.student_submissions[0].id.clone(),
            student_id: project.students[0].id.clone(),
            student_display_name: Some("Öğrenci".to_string()),
            student_number: Some("1".to_string()),
            student_class_name: Some("11A".to_string()),
            question_id: project.questions[0].id.clone(),
            question_number: 1,
            max_score: 5.0,
            awarded_score: Some(5.0),
            scoring_applied: true,
            decision_state: crate::domain::scoring::ScoringDecisionState::TeacherApproved,
            decision_version: "v1".to_string(),
            criterion_scores: vec![],
            semantic_decisions: vec![],
            rationale: "ok".to_string(),
            confidence: 0.9,
            needs_review: false,
            review_reasons: vec![],
            warnings: vec![],
            raw_model_output: "{}".to_string(),
            parse_diagnostics: None,
            reconciliation_diagnostics: None,
            execution_diagnostics: None,
            cache_provenance: None,
            reuse_provenance: None,
            consistency_review: None,
            scoring_fingerprint: String::new(),
            policy_version: String::new(),
            answer_normalized_hash: String::new(),
            answer_raw_hash: String::new(),
            ocr_generation: String::new(),
            source_hash: "source".to_string(),
            package_hash: "package".to_string(),
            ocr_record_hash: "ocr".to_string(),
            question_text_hash: "qtext".to_string(),
            rubric_hash: "rubric".to_string(),
            teacher_review_status: crate::domain::scoring::ScoringReviewStatus::Approved,
            teacher_manual_score: None,
            teacher_reviewed_at: None,
            teacher_notes: None,
            invalidated_at: None,
            invalidation_reason: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }];
        project.student_answer_ocr_records = vec![];

        let mut value = serde_json::to_value(&project).unwrap();
        let record_value = serde_json::json!({
            "id": Uuid::new_v4().to_string(),
            "submissionId": project.student_submissions[0].id.clone(),
            "questionId": project.questions[0].id.clone(),
            "questionNumber": 1,
            "sourcePageNumbers": [1],
            "sourceImageRefs": [],
            "cropRefs": [],
            "fullPagePreviewRefs": [],
            "answerText": "cevap",
            "structuredAnswer": null,
            "confidence": null,
            "uncertainSpans": [],
            "suggestedCorrections": [],
            "criticalTermWarnings": [],
            "ocrSemanticWarnings": [],
            "criticalKeywordUncertain": false,
            "status": "succeeded",
            "needsReview": false,
            "reviewReasons": [],
            "warnings": [],
            "promptVersion": "student_answer_ocr_v2",
            "createdAt": chrono::Utc::now(),
            "updatedAt": chrono::Utc::now()
        });
        value["studentAnswerOcrRecords"] = serde_json::json!([record_value]);
        let record = value
            .get_mut("studentAnswerOcrRecords")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|records| records.get_mut(0))
            .and_then(serde_json::Value::as_object_mut)
            .unwrap();
        record.remove("preprocessMode");
        record.remove("preprocessVersion");
        record.remove("modelInputCropRef");
        record.remove("originalCropRefs");
        record.remove("preprocessedCropRefs");
        record.remove("availablePreprocessVariants");
        record.remove("preprocessDiagnostics");
        record.remove("preprocessWarnings");
        record.remove("runId");
        let legacy_scoring_record = value
            .get_mut("scoringRecords")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|records| records.get_mut(0))
            .and_then(serde_json::Value::as_object_mut)
            .unwrap();
        legacy_scoring_record.remove("scoringApplied");
        legacy_scoring_record.remove("reviewReasons");
        legacy_scoring_record.remove("decisionState");
        legacy_scoring_record.insert("needsReview".to_string(), serde_json::Value::Bool(true));
        value.as_object_mut().unwrap().remove("latestScoringRunId");
        write_project_value(&root, &value);

        let reopened = ProjectStore::open_project_at_path(&root).unwrap();
        assert!(reopened.student_answer_ocr_records[0]
            .preprocess_mode
            .is_none());
        assert!(reopened.student_answer_ocr_records[0]
            .available_preprocess_variants
            .is_empty());
        assert!(reopened.latest_scoring_run_id.is_none());
        assert_eq!(scoring_active_run_id(&reopened), None);
        assert_eq!(reopened.scoring_records[0].run_id, "");
        assert_eq!(reopened.scoring_records[0].awarded_score, Some(5.0));
        assert!(reopened.scoring_records[0].scoring_applied);
        assert_eq!(
            reopened.scoring_records[0].decision_state,
            crate::domain::scoring::ScoringDecisionState::Provisional
        );
        assert!(reopened.scoring_records[0].review_reasons.is_empty());

        let (migrated, warnings) = ProjectStore::new()
            .open_project_with_warnings(root.to_string_lossy().to_string())
            .unwrap();
        assert_eq!(
            migrated.scoring_records[0].decision_state,
            crate::domain::scoring::ScoringDecisionState::Provisional
        );
        assert_eq!(migrated.scoring_records[0].awarded_score, Some(5.0));
        assert!(warnings.iter().any(|warning| warning.contains("backup")));
        let persisted = fs::read_to_string(root.join("project.json")).unwrap();
        let persisted_value: serde_json::Value = serde_json::from_str(&persisted).unwrap();
        assert_eq!(
            persisted_value["scoringRecords"][0]["decisionState"],
            serde_json::Value::String("provisional".to_string())
        );
    }

    #[test]
    fn create_project_writes_project_and_can_be_reopened() {
        let base = temp_root();
        let project_root = base.join("created-project");
        let store = ProjectStore::new();

        let project = store
            .create_project(
                "Created Project".to_string(),
                project_root.to_string_lossy().to_string(),
            )
            .unwrap();

        assert_eq!(project.name, "Created Project");
        assert_eq!(
            project.root_path,
            std::fs::canonicalize(&project_root)
                .unwrap()
                .to_string_lossy()
                .to_string()
        );
        assert!(project_root.join("project.json").is_file());
        assert!(project_root.join("documents").is_dir());
        assert!(project_root.join("cache").join("page_previews").is_dir());

        let report = ProjectStore::list_projects_in_dir(&base);
        assert_eq!(report.projects.len(), 1);
        assert_eq!(report.projects[0].name, "Created Project");

        let reopened = ProjectStore::open_project_at_path(&project_root).unwrap();
        assert_eq!(reopened.id, project.id);
    }

    #[test]
    fn new_project_rejects_existing_project_and_non_empty_directory() {
        let base = temp_root();
        let existing = base.join("existing");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("project.json"), b"original").unwrap();
        let store = ProjectStore::new();
        let error = store
            .create_project("Existing".into(), existing.to_string_lossy().to_string())
            .unwrap_err();
        assert_eq!(error.code, AppErrorCode::ProjectAlreadyExists);
        assert_eq!(
            fs::read(existing.join("project.json")).unwrap(),
            b"original"
        );

        let non_empty = base.join("non-empty");
        fs::create_dir_all(&non_empty).unwrap();
        fs::write(non_empty.join("keep.txt"), b"keep").unwrap();
        let error = ProjectStore::new()
            .create_project("Non empty".into(), non_empty.to_string_lossy().to_string())
            .unwrap_err();
        assert_eq!(error.code, AppErrorCode::ProjectDirectoryNotEmpty);
        assert_eq!(fs::read(non_empty.join("keep.txt")).unwrap(), b"keep");
    }

    #[test]
    fn malicious_stored_root_cannot_redirect_save_to_another_project() {
        let base = temp_root();
        let opened_root = base.join("opened");
        let malicious_root = base.join("malicious");
        fs::create_dir_all(&malicious_root).unwrap();
        let malicious_original = b"do-not-change";
        fs::write(malicious_root.join("project.json"), malicious_original).unwrap();

        let store = ProjectStore::new();
        let project = store
            .create_project("Opened".into(), opened_root.to_string_lossy().to_string())
            .unwrap();
        let opened_file = opened_root.join("project.json");
        let before = fs::read(&opened_file).unwrap();
        let mut tampered = project.clone();
        tampered.root_path = malicious_root.to_string_lossy().to_string();
        store.save_project(&tampered).unwrap();

        assert_eq!(
            fs::read(malicious_root.join("project.json")).unwrap(),
            malicious_original
        );
        assert_ne!(fs::read(opened_file).unwrap(), before);
    }

    #[test]
    fn moved_project_opens_from_new_root_and_warns_without_using_old_root() {
        let base = temp_root();
        let old_root = base.join("old");
        let new_root = base.join("new");
        let store = ProjectStore::new();
        let project = store
            .create_project("Moved".into(), old_root.to_string_lossy().to_string())
            .unwrap();
        let old_root_string = project.root_path.clone();
        drop(store);
        fs::rename(&old_root, &new_root).unwrap();

        let reopened_store = ProjectStore::new();
        let (reopened, warnings) = reopened_store
            .open_project_with_warnings(new_root.to_string_lossy().to_string())
            .unwrap();
        assert_eq!(reopened.id, project.id);
        assert_eq!(
            reopened.root_path,
            fs::canonicalize(&new_root).unwrap().to_string_lossy()
        );
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("root metadata mismatch")));
        assert!(!Path::new(&old_root_string).exists());
        reopened_store.save_project(&reopened).unwrap();
        assert!(new_root.join("project.json").is_file());
    }

    #[test]
    fn legacy_singular_student_scan_is_mapped_to_stable_class_and_batch_in_memory() {
        let root = temp_root();
        let mut project = sample_project(&root, "Legacy 11-C", "2026-01-01T00:00:00Z");
        project.students[0].class_name = Some(" 11 c ".to_string());
        let scan_document_id = project
            .documents
            .iter()
            .find(|document| document.role == DocumentRole::StudentScan)
            .unwrap()
            .id
            .clone();
        project.student_scan_document_id = Some(scan_document_id.clone());
        project.student_submissions[0].document_id = scan_document_id.clone();

        let mut value = serde_json::to_value(&project).unwrap();
        value.as_object_mut().unwrap().remove("schoolClasses");
        value.as_object_mut().unwrap().remove("studentScanBatches");
        let submission = value
            .get_mut("studentSubmissions")
            .and_then(Value::as_array_mut)
            .and_then(|submissions| submissions.first_mut())
            .and_then(Value::as_object_mut)
            .unwrap();
        submission.remove("classId");
        submission.remove("scanBatchId");
        submission.remove("classMembershipSource");
        let original_ocr = value["studentAnswerOcrRecords"].clone();
        let original_scoring = value["scoringRecords"].clone();
        let content = serde_json::to_string(&value).unwrap();

        let (first, warnings, first_changed, _) =
            ProjectStore::deserialize_project(&root.join("project.json"), &content).unwrap();
        let (second, _, second_changed, _) =
            ProjectStore::deserialize_project(&root.join("project.json"), &content).unwrap();

        assert_eq!(first.school_classes.len(), 1);
        assert_eq!(first.school_classes[0].name, "11-C");
        assert_eq!(first.school_classes[0].normalized_name, "11-C");
        assert_eq!(first.school_classes[0].grade_level, Some(11));
        assert_eq!(first.school_classes[0].section.as_deref(), Some("C"));
        assert_eq!(first.student_scan_batches.len(), 1);
        assert_eq!(first.student_scan_batches[0].document_id, scan_document_id);
        assert_eq!(
            first.student_submissions[0].class_id.as_deref(),
            Some(first.school_classes[0].id.as_str())
        );
        assert_eq!(
            first.student_submissions[0].scan_batch_id.as_deref(),
            Some(first.student_scan_batches[0].id.as_str())
        );
        assert_eq!(
            first.student_submissions[0].class_membership_source,
            Some(crate::domain::student::ClassMembershipSource::InheritedFromBatch)
        );
        assert_eq!(first.school_classes[0].id, second.school_classes[0].id);
        assert_eq!(
            first.student_scan_batches[0].id,
            second.student_scan_batches[0].id
        );
        assert_eq!(
            serde_json::to_value(&first.student_answer_ocr_records).unwrap(),
            original_ocr
        );
        assert_eq!(
            serde_json::to_value(&first.scoring_records).unwrap(),
            original_scoring
        );
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("mapped in memory")));
        assert!(first_changed);
        assert!(second_changed);
    }

    #[test]
    fn unknown_class_reference_warns_without_blocking_project_load() {
        let root = temp_root();
        let mut project = sample_project(&root, "Unknown class", "2026-01-01T00:00:00Z");
        project.student_submissions[0].class_id = Some("missing-class".to_string());
        write_project(&root, &project);

        let (loaded, warnings) = ProjectStore::new()
            .open_project_with_warnings(root.to_string_lossy().to_string())
            .unwrap();

        assert_eq!(
            loaded.student_submissions[0].class_id.as_deref(),
            Some("missing-class")
        );
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("unknown classId")));
    }

    #[test]
    fn explicit_open_persists_legacy_migration_after_exact_backup_but_listing_is_read_only() {
        let base = temp_root();
        let root = base.join("legacy-migration");
        let mut project = sample_project(&root, "Legacy backup", "2026-01-01T00:00:00Z");
        project.students[0].class_name = Some("11 C".to_string());
        let scan_document_id = project
            .documents
            .iter()
            .find(|document| document.role == DocumentRole::StudentScan)
            .unwrap()
            .id
            .clone();
        project.student_scan_document_id = Some(scan_document_id.clone());
        project.student_submissions[0].document_id = scan_document_id;
        let mut value = serde_json::to_value(&project).unwrap();
        value.as_object_mut().unwrap().remove("schoolClasses");
        value.as_object_mut().unwrap().remove("studentScanBatches");
        let submission = value["studentSubmissions"][0].as_object_mut().unwrap();
        submission.remove("classId");
        submission.remove("scanBatchId");
        submission.remove("classMembershipSource");
        std::fs::create_dir_all(&root).unwrap();
        let original_content = serde_json::to_string_pretty(&value).unwrap();
        std::fs::write(root.join("project.json"), &original_content).unwrap();

        let list_report = ProjectStore::list_projects_in_dir(&base);
        assert_eq!(list_report.projects.len(), 1);
        assert_eq!(
            std::fs::read_to_string(root.join("project.json")).unwrap(),
            original_content
        );
        assert_eq!(migration_backup_paths(&root).len(), 0);

        let store = ProjectStore::new();
        let (loaded, warnings) = store
            .open_project_with_warnings(root.to_string_lossy().to_string())
            .unwrap();
        assert_eq!(loaded.school_classes[0].normalized_name, "11-C");
        let backups = migration_backup_paths(&root);
        assert_eq!(backups.len(), 1);
        assert_eq!(
            std::fs::read_to_string(&backups[0]).unwrap(),
            original_content
        );
        let persisted: Value =
            serde_json::from_str(&std::fs::read_to_string(root.join("project.json")).unwrap())
                .unwrap();
        assert_eq!(persisted["schoolClasses"].as_array().unwrap().len(), 1);
        assert_eq!(persisted["studentScanBatches"].as_array().unwrap().len(), 1);
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("persisted atomically")));

        store
            .open_project_with_warnings(root.to_string_lossy().to_string())
            .unwrap();
        assert_eq!(migration_backup_paths(&root).len(), 1);
    }

    fn migration_backup_paths(root: &Path) -> Vec<PathBuf> {
        let mut paths = std::fs::read_dir(root)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.starts_with("project.json.migration.") && name.ends_with(".bak")
                    })
            })
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }

    #[test]
    fn transactional_mutations_preserve_disjoint_updates_and_increment_revision_once() {
        let root = temp_root();
        let project = sample_project(&root, "Concurrent", "2026-01-01T00:00:00Z");
        write_project(&root, &project);
        let store = ProjectStore::new();
        let opened = store
            .open_project_with_warnings(root.to_string_lossy().to_string())
            .unwrap()
            .0;
        let project_id = opened.id.clone();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));

        let class_store = store.clone();
        let class_barrier = barrier.clone();
        let class_project_id = project_id.clone();
        let class_task = std::thread::spawn(move || {
            class_barrier.wait();
            class_store
                .mutate(
                    &class_project_id,
                    MutationOptions::new("test_update_school_class"),
                    |project, _| {
                        project.school_classes.push(SchoolClass {
                            id: "class-a".to_string(),
                            name: "11-A".to_string(),
                            display_name: "11-A".to_string(),
                            normalized_name: "11-A".to_string(),
                            academic_year: Some("2026".to_string()),
                            academic_year_id: Some("2026".to_string()),
                            grade_level: Some(11),
                            section: Some("A".to_string()),
                            display_order: 1,
                            status: SchoolClassStatus::Active,
                            created_at: "now".to_string(),
                            updated_at: "now".to_string(),
                        });
                        Ok(())
                    },
                )
                .unwrap()
        });

        let activity_store = store.clone();
        let activity_barrier = barrier;
        let activity_project_id = project_id.clone();
        let activity_task = std::thread::spawn(move || {
            activity_barrier.wait();
            activity_store
                .mutate(
                    &activity_project_id,
                    MutationOptions::new("test_update_assessment_activity"),
                    |project, _| {
                        project.assessment_activities.push(AssessmentActivity {
                            id: "activity-a".to_string(),
                            academic_year_id: "2026".to_string(),
                            course_id: "turkce".to_string(),
                            course_name: "Türkçe".to_string(),
                            title: "1. Yazılı".to_string(),
                            grade_level: 11,
                            term: 1,
                            assessment_type: AssessmentType::Written,
                            workflow_family: WorkflowFamily::Written,
                            sequence_number: 1,
                            status: AssessmentStatus::Draft,
                            common_document_ids: vec![],
                            listening_details: None,
                            speaking_configuration: None,
                            performance_details: None,
                            class_applications: vec![],
                            created_at: "now".to_string(),
                            updated_at: "now".to_string(),
                        });
                        Ok(())
                    },
                )
                .unwrap()
        });

        let first = class_task.join().unwrap();
        let second = activity_task.join().unwrap();
        let revisions = [first.snapshot.revision, second.snapshot.revision];
        assert!(revisions.contains(&1));
        assert!(revisions.contains(&2));

        let persisted = ProjectStore::open_project_at_path(&root).unwrap();
        assert_eq!(persisted.storage_revision, 2);
        assert!(persisted
            .school_classes
            .iter()
            .any(|value| value.id == "class-a"));
        assert!(persisted
            .assessment_activities
            .iter()
            .any(|value| value.id == "activity-a"));
    }

    #[test]
    fn stale_candidates_from_one_initial_snapshot_do_not_lose_disjoint_updates() {
        let root = temp_root();
        let project = sample_project(&root, "Stale candidates", "2026-01-01T00:00:00Z");
        write_project(&root, &project);
        let store = ProjectStore::new();
        let initial = store
            .open_project_with_warnings(root.to_string_lossy().to_string())
            .unwrap()
            .0;

        let mut class_candidate = initial.clone();
        class_candidate.school_classes.push(SchoolClass {
            id: "stale-class".to_string(),
            name: "11-B".to_string(),
            display_name: "11-B".to_string(),
            normalized_name: "11-B".to_string(),
            academic_year: Some("2026".to_string()),
            academic_year_id: Some("2026".to_string()),
            grade_level: Some(11),
            section: Some("B".to_string()),
            display_order: 2,
            status: SchoolClassStatus::Active,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        });
        let mut activity_candidate = initial.clone();
        activity_candidate
            .assessment_activities
            .push(AssessmentActivity {
                id: "stale-activity".to_string(),
                academic_year_id: "2026".to_string(),
                course_id: "turkce".to_string(),
                course_name: "Türkçe".to_string(),
                title: "2. Yazılı".to_string(),
                grade_level: 11,
                term: 1,
                assessment_type: AssessmentType::Written,
                workflow_family: WorkflowFamily::Written,
                sequence_number: 2,
                status: AssessmentStatus::Draft,
                common_document_ids: vec![],
                listening_details: None,
                speaking_configuration: None,
                performance_details: None,
                class_applications: vec![],
                created_at: "now".to_string(),
                updated_at: "now".to_string(),
            });

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let class_store = store.clone();
        let class_barrier = barrier.clone();
        let class_task = std::thread::spawn(move || {
            class_barrier.wait();
            class_store.commit_snapshot_cas(&class_candidate).unwrap();
        });
        let activity_store = store.clone();
        let activity_barrier = barrier;
        let activity_task = std::thread::spawn(move || {
            activity_barrier.wait();
            activity_store
                .commit_snapshot_cas(&activity_candidate)
                .unwrap();
        });
        class_task.join().unwrap();
        activity_task.join().unwrap();

        let persisted = ProjectStore::open_project_at_path(&root).unwrap();
        assert_eq!(persisted.storage_revision, 2);
        assert!(persisted
            .school_classes
            .iter()
            .any(|value| value.id == "stale-class"));
        assert!(persisted
            .assessment_activities
            .iter()
            .any(|value| value.id == "stale-activity"));
    }

    #[test]
    fn snapshot_without_revision_history_is_rejected_without_a_noop_success() {
        let root = temp_root();
        let project = sample_project(&root, "Missing base", "2026-01-01T00:00:00Z");
        write_project(&root, &project);
        let store = ProjectStore::new();
        let opened = store
            .open_project_with_warnings(root.to_string_lossy().to_string())
            .unwrap()
            .0;

        let mut stale = opened.clone();
        stale.storage_revision = opened.storage_revision + 10_000;
        let error = store
            .commit_snapshot_cas(&stale)
            .expect_err("a snapshot without a known base must not report success");

        assert_eq!(error.code, AppErrorCode::ProjectEntityStale);
        let persisted = ProjectStore::open_project_at_path(&root).unwrap();
        assert_eq!(persisted.storage_revision, opened.storage_revision);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn expected_revision_conflict_does_not_use_last_write_wins() {
        let root = temp_root();
        let project = sample_project(&root, "CAS", "2026-01-01T00:00:00Z");
        write_project(&root, &project);
        let store = ProjectStore::new();
        let opened = store
            .open_project_with_warnings(root.to_string_lossy().to_string())
            .unwrap()
            .0;
        let revision = opened.storage_revision;
        store
            .mutate(
                &opened.id,
                MutationOptions {
                    expected_revision: Some(revision),
                    expected_fingerprint: None,
                    operation: "first".to_string(),
                    correlation_id: "first".to_string(),
                },
                |project, _| {
                    project.name = "first".to_string();
                    Ok(())
                },
            )
            .unwrap();

        let error = store
            .mutate(
                &opened.id,
                MutationOptions {
                    expected_revision: Some(revision),
                    expected_fingerprint: None,
                    operation: "stale".to_string(),
                    correlation_id: "stale".to_string(),
                },
                |project, _| {
                    project.name = "stale overwrite".to_string();
                    Ok(())
                },
            )
            .unwrap_err();
        assert_eq!(error.code, AppErrorCode::ProjectRevisionConflict);
        let persisted = ProjectStore::open_project_at_path(&root).unwrap();
        assert_eq!(persisted.name, "first");
        assert_eq!(persisted.storage_revision, 1);
    }

    #[test]
    fn external_project_json_change_is_rejected_by_fingerprint() {
        let root = temp_root();
        let project = sample_project(&root, "External", "2026-01-01T00:00:00Z");
        write_project(&root, &project);
        let store = ProjectStore::new();
        let opened = store
            .open_project_with_warnings(root.to_string_lossy().to_string())
            .unwrap()
            .0;
        let snapshot = store
            .get_project_snapshot_with_metadata(&opened.id)
            .unwrap();
        let mut external = snapshot.project.clone();
        external.name = "external edit".to_string();
        std::fs::write(
            root.join("project.json"),
            serde_json::to_string_pretty(&external).unwrap(),
        )
        .unwrap();

        let error = store
            .mutate(
                &opened.id,
                MutationOptions {
                    expected_revision: Some(snapshot.revision),
                    expected_fingerprint: Some(snapshot.content_fingerprint),
                    operation: "after_external_change".to_string(),
                    correlation_id: "external".to_string(),
                },
                |project, _| {
                    project.course_name = Some("must not overwrite".to_string());
                    Ok(())
                },
            )
            .unwrap_err();
        assert_eq!(error.code, AppErrorCode::ProjectExternallyModified);
        let persisted = ProjectStore::open_project_at_path(&root).unwrap();
        assert_eq!(persisted.name, "external edit");
        assert_eq!(persisted.storage_revision, 0);
    }

    #[test]
    fn different_project_roots_do_not_share_the_mutation_lock() {
        let left = temp_root().canonicalize().unwrap();
        let right = temp_root().canonicalize().unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let started = std::time::Instant::now();
        let left_barrier = barrier.clone();
        let left_task = std::thread::spawn(move || {
            let lock = project_lock_for(&left).unwrap();
            let _guard = lock.lock().unwrap();
            left_barrier.wait();
            std::thread::sleep(std::time::Duration::from_millis(120));
        });
        let right_barrier = barrier;
        let right_task = std::thread::spawn(move || {
            let lock = project_lock_for(&right).unwrap();
            let _guard = lock.lock().unwrap();
            right_barrier.wait();
            std::thread::sleep(std::time::Duration::from_millis(120));
        });
        left_task.join().unwrap();
        right_task.join().unwrap();
        assert!(started.elapsed() < std::time::Duration::from_millis(220));
    }

    #[test]
    fn job_narrow_commit_preserves_unrelated_mutation_and_stale_source_is_not_applied() {
        let root = temp_root();
        let project = sample_project(&root, "Job", "2026-01-01T00:00:00Z");
        write_project(&root, &project);
        let store = ProjectStore::new();
        let opened = store
            .open_project_with_warnings(root.to_string_lossy().to_string())
            .unwrap()
            .0;
        let project_id = opened.id.clone();
        let document_id = opened.documents[0].id.clone();
        let source_checksum = opened.documents[0].checksum.clone();

        store
            .mutate(
                &project_id,
                MutationOptions::new("unrelated_school_class_edit"),
                |project, _| {
                    project.school_classes.push(SchoolClass {
                        id: "class-job".to_string(),
                        name: "10-B".to_string(),
                        display_name: "10-B".to_string(),
                        normalized_name: "10-B".to_string(),
                        academic_year: None,
                        academic_year_id: None,
                        grade_level: Some(10),
                        section: Some("B".to_string()),
                        display_order: 1,
                        status: SchoolClassStatus::Active,
                        created_at: "now".to_string(),
                        updated_at: "now".to_string(),
                    });
                    Ok(())
                },
            )
            .unwrap();

        let applied = store.commit_job(
            &project_id,
            MutationOptions::new("ocr_narrow_commit"),
            move |project, _| {
                let document = project
                    .documents
                    .iter_mut()
                    .find(|document| document.id == document_id)
                    .ok_or_else(|| {
                        project_error(
                            AppErrorCode::ProjectEntityNotFound,
                            "Belge bulunamadı.",
                            None,
                        )
                    })?;
                if document.checksum != source_checksum {
                    return Err(project_error(
                        AppErrorCode::ProjectEntityStale,
                        "Belge değişti; iş sonucu güncel değil.",
                        None,
                    ));
                }
                document.checksum = Some("ocr-result".to_string());
                Ok(())
            },
        );
        assert!(matches!(applied, JobCommitResult::Applied(_)));
        let persisted = ProjectStore::open_project_at_path(&root).unwrap();
        assert_eq!(
            persisted.documents[0].checksum.as_deref(),
            Some("ocr-result")
        );
        assert!(persisted
            .school_classes
            .iter()
            .any(|value| value.id == "class-job"));

        let stale_document_id = persisted.documents[0].id.clone();
        store
            .mutate(
                &project_id,
                MutationOptions::new("document_source_changed"),
                move |project, _| {
                    project.documents[0].checksum = Some("new-source".to_string());
                    Ok(())
                },
            )
            .unwrap();
        let stale = store.commit_job(
            &project_id,
            MutationOptions::new("stale_ocr_result"),
            move |project, _| {
                let document = project
                    .documents
                    .iter_mut()
                    .find(|document| document.id == stale_document_id)
                    .ok_or_else(|| {
                        project_error(
                            AppErrorCode::ProjectEntityNotFound,
                            "Belge bulunamadı.",
                            None,
                        )
                    })?;
                if document.checksum.as_deref() != Some("ocr-result") {
                    return Err(project_error(
                        AppErrorCode::ProjectEntityStale,
                        "Belge değişti; iş sonucu güncel değil.",
                        None,
                    ));
                }
                document.checksum = Some("stale-result-must-not-apply".to_string());
                Ok(())
            },
        );
        assert!(matches!(stale, JobCommitResult::Stale { .. }));
        let persisted = ProjectStore::open_project_at_path(&root).unwrap();
        assert_eq!(
            persisted.documents[0].checksum.as_deref(),
            Some("new-source")
        );
    }

    #[test]
    fn fifty_concurrent_disjoint_mutations_keep_valid_json_and_all_entities() {
        let root = temp_root();
        let project = sample_project(&root, "Stress", "2026-01-01T00:00:00Z");
        write_project(&root, &project);
        let store = ProjectStore::new();
        let opened = store
            .open_project_with_warnings(root.to_string_lossy().to_string())
            .unwrap()
            .0;
        let project_id = opened.id;
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(50));
        let mut tasks = Vec::new();
        for index in 0..50u32 {
            let store = store.clone();
            let project_id = project_id.clone();
            let barrier = barrier.clone();
            tasks.push(std::thread::spawn(move || {
                barrier.wait();
                store
                    .mutate(
                        &project_id,
                        MutationOptions::new(format!("stress_{index}")),
                        move |project, _| {
                            match index % 3 {
                                0 => project.school_classes.push(SchoolClass {
                                    id: format!("stress-class-{index}"),
                                    name: format!("{index}-A"),
                                    display_name: format!("{index}-A"),
                                    normalized_name: format!("{index}-A"),
                                    academic_year: None,
                                    academic_year_id: None,
                                    grade_level: Some(index + 1),
                                    section: Some("A".to_string()),
                                    display_order: index,
                                    status: SchoolClassStatus::Active,
                                    created_at: "now".to_string(),
                                    updated_at: "now".to_string(),
                                }),
                                1 => project.assessment_activities.push(AssessmentActivity {
                                    id: format!("stress-activity-{index}"),
                                    academic_year_id: "2026".to_string(),
                                    course_id: format!("course-{index}"),
                                    course_name: "Türkçe".to_string(),
                                    title: format!("Activity {index}"),
                                    grade_level: 1,
                                    term: 1,
                                    assessment_type: AssessmentType::Written,
                                    workflow_family: WorkflowFamily::Written,
                                    sequence_number: index + 1,
                                    status: AssessmentStatus::Draft,
                                    common_document_ids: vec![],
                                    listening_details: None,
                                    speaking_configuration: None,
                                    performance_details: None,
                                    class_applications: vec![],
                                    created_at: "now".to_string(),
                                    updated_at: "now".to_string(),
                                }),
                                _ => project.documents.push(Document {
                                    id: format!("stress-document-{index}"),
                                    role: DocumentRole::Export,
                                    file_name: format!("note-{index}.pdf"),
                                    stored_path: format!("outputs/note-{index}.pdf"),
                                    page_count: 1,
                                    added_at: "now".to_string(),
                                    checksum: Some(format!("checksum-{index}")),
                                    preview: None,
                                }),
                            }
                            Ok(())
                        },
                    )
                    .unwrap();
            }));
        }
        for task in tasks {
            task.join().unwrap();
        }

        let persisted = ProjectStore::open_project_at_path(&root).unwrap();
        assert_eq!(persisted.storage_revision, 50);
        assert_eq!(persisted.school_classes.len(), 18);
        assert_eq!(persisted.assessment_activities.len(), 17);
        assert_eq!(persisted.documents.len(), 19);
        let json: Value =
            serde_json::from_str(&std::fs::read_to_string(root.join("project.json")).unwrap())
                .unwrap();
        assert!(json.is_object());
    }

    #[test]
    fn atomic_write_failure_keeps_previous_project_and_revision() {
        let root = temp_root();
        let project = sample_project(&root, "Atomic failure", "2026-01-01T00:00:00Z");
        write_project(&root, &project);
        let store = ProjectStore::new();
        let opened = store
            .open_project_with_warnings(root.to_string_lossy().to_string())
            .unwrap()
            .0;
        let before = fs::read_to_string(root.join("project.json")).unwrap();
        fs::create_dir(root.join("project.tmp")).unwrap();

        let error = store
            .mutate(
                &opened.id,
                MutationOptions::new("atomic_write_failure"),
                |project, _| {
                    project.name = "must not persist".to_string();
                    Ok(())
                },
            )
            .unwrap_err();

        assert_eq!(error.code, AppErrorCode::ProjectSaveFailed);
        assert_eq!(
            fs::read_to_string(root.join("project.json")).unwrap(),
            before
        );
        assert_eq!(
            ProjectStore::open_project_at_path(&root)
                .unwrap()
                .storage_revision,
            0
        );
    }

    #[test]
    fn production_services_do_not_use_blind_full_project_save() {
        let service_files = [
            "document_service.rs",
            "student_scan_service.rs",
            "student_answer_ocr_service.rs",
            "pdf_preview_service.rs",
            "rubric_extraction_service.rs",
            "exam_package_build_service.rs",
            "scoring_service.rs",
            "school_class_service.rs",
            "assessment_organization_service.rs",
            "speaking_exam_service.rs",
            "analysis_service.rs",
            "workflow_engine.rs",
        ];
        let services_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/services");
        for file_name in service_files {
            let source = fs::read_to_string(services_root.join(file_name)).unwrap();
            let production_lines = source
                .lines()
                .take_while(|line| !line.contains("#[cfg(test)]"))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                !production_lines.contains("save_project("),
                "production writer remained in {file_name}"
            );
        }
        let store_source = fs::read_to_string(services_root.join("project_store.rs")).unwrap();
        assert!(store_source.contains("#[cfg(test)]\n    pub(crate) fn save_project"));
    }

    #[test]
    fn legacy_project_starts_at_revision_zero_and_first_mutation_writes_one() {
        let base = temp_root();
        let root = base.join("legacy-revision");
        let setup_store = ProjectStore::new();
        let project = setup_store
            .create_project(
                "Legacy revision".to_string(),
                root.to_string_lossy().to_string(),
            )
            .unwrap();
        let mut value = serde_json::to_value(&project).unwrap();
        value.as_object_mut().unwrap().remove("storageRevision");
        let original = serde_json::to_string_pretty(&value).unwrap();
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("project.json"), &original).unwrap();
        let store = ProjectStore::new();
        let opened = store
            .open_project_with_warnings(root.to_string_lossy().to_string())
            .unwrap()
            .0;
        assert_eq!(opened.storage_revision, 0);
        assert_eq!(
            std::fs::read_to_string(root.join("project.json")).unwrap(),
            original
        );
        store
            .mutate(
                &opened.id,
                MutationOptions::new("first_legacy_mutation"),
                |project, _| {
                    project.name = "migrated by mutation".to_string();
                    Ok(())
                },
            )
            .unwrap();
        let reopened = ProjectStore::open_project_at_path(&root).unwrap();
        assert_eq!(reopened.storage_revision, 1);
    }

    #[test]
    fn list_projects_in_dir_keeps_legacy_projects_visible() {
        let root = temp_root();
        let legacy = root.join("legacy");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(
            legacy.join("project.json"),
            format!(
                r#"{{
                    "id":"{}",
                    "name":"Legacy Visible",
                    "createdAt":"2026-01-01T00:00:00Z",
                    "updatedAt":"2026-01-01T00:00:00Z",
                    "rootPath":"{}",
                    "documents":[],
                    "questions":[],
                    "workflow":{{
                        "currentStage":"documents_missing",
                        "blockingReasons":[],
                        "nextActions":[],
                        "summary":"Legacy text"
                    }}
                }}"#,
                Uuid::new_v4(),
                legacy.to_string_lossy()
            ),
        )
        .unwrap();

        let report = ProjectStore::list_projects_in_dir(&root);
        assert_eq!(report.projects.len(), 1);
        assert_eq!(report.projects[0].name, "Legacy Visible");
        assert!(report.skipped_projects.is_empty());
    }

    #[test]
    fn proof_35_stale_job_cannot_overwrite_teacher_change() {
        job_narrow_commit_preserves_unrelated_mutation_and_stale_source_is_not_applied();
    }

    #[test]
    fn migrates_legacy_single_region_crop_without_losing_region_data() {
        let mut value = serde_json::json!({
            "studentAnswerCropTemplate": {
                "items": [{
                    "questionId": "q1",
                    "questionNumber": 1,
                    "pageIndexWithinSubmission": 2,
                    "bbox": {"x": 0.1, "y": 0.2, "width": 0.3, "height": 0.4, "pageIndex": 2},
                    "label": "Soru 1",
                    "note": "devam ediyor"
                }]
            }
        });

        let (warnings, changed) =
            normalize_project_json(Path::new("legacy/project.json"), &mut value);
        let template = value
            .get("studentAnswerCropTemplate")
            .and_then(Value::as_object)
            .expect("canonical crop template");
        let region = template
            .get("templates")
            .and_then(Value::as_array)
            .and_then(|templates| templates.first())
            .and_then(|template| template.get("regions"))
            .and_then(Value::as_array)
            .and_then(|regions| regions.first())
            .expect("migrated region");

        assert!(changed);
        assert!(template.get("items").is_none());
        assert_eq!(region.get("pageOffset").and_then(Value::as_u64), Some(2));
        assert_eq!(region.get("order").and_then(Value::as_u64), Some(0));
        assert_eq!(region.get("label").and_then(Value::as_str), Some("Soru 1"));
        assert_eq!(
            region.get("note").and_then(Value::as_str),
            Some("devam ediyor")
        );
        assert!(warnings.iter().any(|warning| warning.contains("regions")));
    }

    #[test]
    fn migrates_missing_scoring_anchor_collection_without_losing_scoring_records() {
        let original_record = serde_json::json!({
            "id": "record-keep",
            "opaqueTeacherEvidence": "keep this record"
        });
        let mut value = serde_json::json!({
            "scoringRecords": [original_record.clone()]
        });

        let (warnings, changed) =
            normalize_project_json(Path::new("legacy/anchors/project.json"), &mut value);

        assert!(changed);
        assert_eq!(
            value
                .get("scoringAnchors")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(0)
        );
        let migrated_record = value
            .get("scoringRecords")
            .and_then(Value::as_array)
            .and_then(|records| records.first())
            .and_then(Value::as_object)
            .expect("migrated scoring record");
        assert_eq!(
            migrated_record.get("id").and_then(Value::as_str),
            Some("record-keep")
        );
        assert_eq!(
            migrated_record
                .get("opaqueTeacherEvidence")
                .and_then(Value::as_str),
            original_record
                .get("opaqueTeacherEvidence")
                .and_then(Value::as_str)
        );
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("scoringAnchors")));

        let (_warnings, second_changed) =
            normalize_project_json(Path::new("legacy/anchors/project.json"), &mut value);
        assert!(!second_changed);
    }

    #[test]
    fn salvages_legacy_structured_answer_without_deleting_raw_data() {
        let raw_answer = serde_json::json!({
            "legacyRows": [{"value": "keep this"}],
            "legacyMarker": "arbitrary-json"
        });
        let mut value = serde_json::json!({
            "studentAnswerOcrRecords": [{
                "structuredAnswer": raw_answer,
                "needsReview": false,
                "reviewReasons": [],
                "warnings": []
            }]
        });

        let (warnings, _changed) =
            normalize_project_json(Path::new("legacy/ocr/project.json"), &mut value);
        let record = value
            .get("studentAnswerOcrRecords")
            .and_then(Value::as_array)
            .and_then(|records| records.first())
            .and_then(Value::as_object)
            .expect("legacy OCR record");

        assert_eq!(record.get("structuredAnswer"), Some(&raw_answer));
        assert_eq!(
            record.get("needsReview").and_then(Value::as_bool),
            Some(true)
        );
        assert!(record
            .get("reviewReasons")
            .and_then(Value::as_array)
            .is_some_and(|reasons| reasons
                .iter()
                .any(|reason| { reason.as_str() == Some("structured_answer_legacy_unparsed") })));
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("review-only salvage")));
    }

    #[test]
    fn performance_activity_legacy_json_opens_with_defaults_idempotently() {
        let root = temp_root();
        let mut project = sample_project(&root, "Performance legacy", "2026-01-01T00:00:00Z");
        project.school_classes.push(SchoolClass {
            id: "class-9-a".to_string(),
            name: "9-A".to_string(),
            display_name: "9-A".to_string(),
            normalized_name: "9-A".to_string(),
            academic_year: Some("2026-2027".to_string()),
            academic_year_id: Some("2026-2027".to_string()),
            grade_level: Some(9),
            section: Some("A".to_string()),
            display_order: 0,
            status: SchoolClassStatus::Active,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        });
        project.assessment_activities.push(AssessmentActivity {
            id: "perf-1".to_string(),
            academic_year_id: "2026-2027".to_string(),
            course_id: "tde".to_string(),
            course_name: "Türk Dili ve Edebiyatı".to_string(),
            title: "1. Performans".to_string(),
            grade_level: 9,
            term: 1,
            assessment_type: AssessmentType::Performance,
            workflow_family: WorkflowFamily::Performance,
            sequence_number: 1,
            status: AssessmentStatus::Draft,
            common_document_ids: vec![],
            listening_details: None,
            speaking_configuration: None,
            performance_details: Some(crate::domain::performance::PerformanceDetails::default()),
            class_applications: vec![class_application("perf-1", "app-perf", "class-9-a")],
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        });
        let mut value = serde_json::to_value(&project).unwrap();
        let activity = value["assessmentActivities"][0].as_object_mut().unwrap();
        activity.remove("performanceDetails");
        activity.insert(
            "workflowFamily".to_string(),
            Value::String("written".to_string()),
        );
        activity["classApplications"][0]
            .as_object_mut()
            .unwrap()
            .remove("performanceAssessments");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("project.json"),
            serde_json::to_string_pretty(&value).unwrap(),
        )
        .unwrap();

        let (first, warnings) = ProjectStore::new()
            .open_project_with_warnings(root.to_string_lossy().to_string())
            .expect("legacy performance project must remain loadable");
        assert_eq!(
            first.assessment_activities[0].assessment_type,
            AssessmentType::Performance
        );
        assert_eq!(
            first.assessment_activities[0].workflow_family,
            WorkflowFamily::Performance
        );
        assert!(first.assessment_activities[0].performance_details.is_some());
        assert!(first.assessment_activities[0].class_applications[0]
            .performance_assessments
            .is_empty());
        assert!(warnings.iter().any(|warning| warning.contains("backup")));

        let (second, _) = ProjectStore::new()
            .open_project_with_warnings(root.to_string_lossy().to_string())
            .expect("migrated project should reload idempotently");
        assert_eq!(second.assessment_activities.len(), 1);
        assert_eq!(
            second.assessment_activities[0].workflow_family,
            WorkflowFamily::Performance
        );
        assert!(second.assessment_activities[0]
            .performance_details
            .is_some());

        let reopened = ProjectStore::open_project_at_path(&root).unwrap();
        assert!(reopened.assessment_activities[0]
            .performance_details
            .is_some());
        assert_eq!(
            reopened.assessment_activities[0].class_applications[0]
                .performance_assessments
                .len(),
            0
        );
    }
}

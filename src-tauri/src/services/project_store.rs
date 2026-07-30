use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use uuid::Uuid;

use crate::domain::document::DocumentRole;
use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::project::Project;
use crate::domain::question::is_question_text_ready;
use crate::domain::rubric::RubricStatus;
use crate::domain::school_class::normalize_school_class_name;
use crate::domain::workflow::{WorkflowSnapshot, WorkflowStage};
use crate::platform::file_access::atomic_write;
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

#[derive(Clone)]
pub struct ProjectStore {
    current_project: Arc<Mutex<Option<Project>>>,
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
        }
    }

    pub fn create_project(&self, name: String, root_path: String) -> Result<Project, AppError> {
        let root = PathBuf::from(&root_path);
        let project_id = Uuid::new_v4().to_string();

        let dirs = [
            root.join("documents"),
            root.join("cache").join("page_previews"),
            root.join("cache").join("model_raw"),
            root.join("cache").join("model_inputs"),
            root.join("crops"),
            root.join("outputs"),
            root.join("logs"),
            root.join("logs").join("jobs"),
        ];

        for dir in &dirs {
            std::fs::create_dir_all(dir).map_err(|e| AppError {
                code: AppErrorCode::ProjectSaveFailed,
                message: format!("Failed to create project directory: {}", e),
                recoverable: false,
                suggested_action: Some("Check permissions for the project location.".to_string()),
                technical_details: Some(e.to_string()),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        }

        let now = chrono::Utc::now().to_rfc3339();

        let project = Project {
            id: project_id,
            name,
            created_at: now.clone(),
            updated_at: now,
            root_path: root_path.clone(),
            sections: vec![],
            students: vec![],
            school_classes: vec![],
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
            speaking_exams: vec![],
            latest_scoring_run_id: None,
            student_answer_ocr_records: vec![],
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
        self.save_project(&project)?;

        let mut lock = self.current_project.lock().map_err(|e| AppError {
            code: AppErrorCode::UnknownError,
            message: "Project store lock failed.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(e.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        *lock = Some(project.clone());

        Ok(project)
    }

    pub fn open_project(&self, root_path: String) -> Result<Project, AppError> {
        self.open_project_with_warnings(root_path)
            .map(|(project, _)| project)
    }

    pub fn open_project_with_warnings(
        &self,
        root_path: String,
    ) -> Result<(Project, Vec<String>), AppError> {
        let project_file = Path::new(&root_path).join("project.json");
        let mut loaded = Self::load_project_file(&project_file, true)?;
        if loaded.migration_changed {
            let backup_path = Self::persist_migrated_project(&project_file, &loaded.project)?;
            loaded.warnings.push(format!(
                "Legacy class/batch migration was persisted atomically after creating backup {}.",
                backup_path.display()
            ));
        }

        let mut lock = self.current_project.lock().map_err(|e| AppError {
            code: AppErrorCode::UnknownError,
            message: "Project store lock failed.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(e.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        *lock = Some(loaded.project.clone());

        Ok((loaded.project, loaded.warnings))
    }

    fn persist_migrated_project(
        project_file: &Path,
        project: &Project,
    ) -> Result<PathBuf, AppError> {
        let original_content = std::fs::read_to_string(project_file).map_err(|error| AppError {
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
        atomic_write(&backup_path, &original_content).map_err(|error| AppError {
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
        atomic_write(project_file, &migrated_content).map_err(|error| AppError {
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

    pub fn save_project(&self, project: &Project) -> Result<(), AppError> {
        let mut project = project.clone();
        project.updated_at = chrono::Utc::now().to_rfc3339();
        let project_file = PathBuf::from(&project.root_path).join("project.json");
        // The in-memory project lock also serializes project.json writes. Without
        // this guard, concurrent jobs could race on the same temporary file and
        // leave a valid user action unpersisted.
        let mut lock = self.current_project.lock().map_err(|e| AppError {
            code: AppErrorCode::UnknownError,
            message: "Project store lock failed.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(e.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;

        let content = serde_json::to_string_pretty(&project).map_err(|e| AppError {
            code: AppErrorCode::ProjectSaveFailed,
            message: "Could not serialize project data.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(e.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;

        atomic_write(&project_file, &content).map_err(|e| AppError {
            code: AppErrorCode::ProjectSaveFailed,
            message: "Failed to write project file.".to_string(),
            recoverable: false,
            suggested_action: Some("Check disk space and permissions.".to_string()),
            technical_details: Some(e.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;

        if let Some(p) = lock.as_mut() {
            if p.id == project.id {
                *p = project.clone();
            }
        }

        Ok(())
    }

    pub fn get_project_snapshot(&self, project_id: String) -> Result<Project, AppError> {
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
                Ok(project.clone())
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
        let content = std::fs::read_to_string(project_file).map_err(|error| AppError {
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

        let (mut project, warnings, migration_changed) =
            Self::deserialize_project(project_file, &content)?;
        if refresh_workflow {
            project.workflow = workflow_engine::evaluate_workflow(&project);
        }
        Ok(LoadedProject {
            project,
            warnings,
            migration_changed,
        })
    }

    fn deserialize_project(
        project_file: &Path,
        content: &str,
    ) -> Result<(Project, Vec<String>, bool), AppError> {
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
        Ok((project, warnings, migration_changed))
    }
}

struct LoadedProject {
    project: Project,
    warnings: Vec<String>,
    migration_changed: bool,
}

fn normalize_project_json(project_file: &Path, value: &mut Value) -> (Vec<String>, bool) {
    let mut warnings = Vec::new();
    let Some(project) = value.as_object_mut() else {
        return (warnings, false);
    };

    let migration_changed = normalize_school_class_storage(project_file, project, &mut warnings);
    normalize_speaking_exams(project_file, project, &mut warnings);
    normalize_student_answer_ocr_records(project_file, project, &mut warnings);
    normalize_student_identity_records(project_file, project, &mut warnings);
    normalize_scoring_records(project_file, project, &mut warnings);

    (warnings, migration_changed)
}

fn normalize_speaking_exams(
    _project_file: &Path,
    project: &mut Map<String, Value>,
    _warnings: &mut Vec<String>,
) {
    let Some(exams) = project
        .get_mut("speakingExams")
        .and_then(Value::as_array_mut)
    else {
        return;
    };

    for exam in exams {
        let Some(exam) = exam.as_object_mut() else {
            continue;
        };

        let has_assigned = exam
            .get("assignedClassIds")
            .and_then(Value::as_array)
            .is_some_and(|arr| !arr.is_empty());

        if !has_assigned {
            if let Some(class_id) = exam.get("classId").and_then(Value::as_str) {
                if !class_id.trim().is_empty() {
                    exam.insert(
                        "assignedClassIds".to_string(),
                        Value::Array(vec![Value::String(class_id.to_string())]),
                    );
                }
            }
        }
    }
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
) {
    let Some(records) = project
        .get_mut("scoringRecords")
        .and_then(Value::as_array_mut)
    else {
        return;
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
    }
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
    use crate::domain::document::Document;
    use crate::domain::project::Project;
    use crate::domain::question::{
        AnswerType, CropTemplate, Question, TextFieldSource, TextFieldState, TextFieldStatus,
    };
    use crate::domain::rubric::{RubricCriterion, RubricState};
    use crate::domain::scoring::scoring_active_run_id;
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
            speaking_exams: vec![],
            latest_scoring_run_id: None,
            student_answer_ocr_records: vec![],
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
                        criteria: vec![RubricCriterion {
                            id: Uuid::new_v4().to_string(),
                            label: "Kriter".to_string(),
                            description: "Açıklama".to_string(),
                            points: 5.0,
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
        assert_eq!(reopened.root_path, root.to_string_lossy().to_string());
        assert_eq!(reopened.questions.len(), 2);
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
        assert_eq!(result.unwrap_err().code, AppErrorCode::ProjectLoadFailed);
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
            criterion_scores: vec![],
            rationale: "ok".to_string(),
            confidence: 0.9,
            needs_review: false,
            review_reasons: vec![],
            warnings: vec![],
            raw_model_output: "{}".to_string(),
            parse_diagnostics: None,
            reconciliation_diagnostics: None,
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
        assert!(reopened.scoring_records[0].review_reasons.is_empty());
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
            project_root.to_string_lossy().to_string()
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

        let (first, warnings, first_changed) =
            ProjectStore::deserialize_project(&root.join("project.json"), &content).unwrap();
        let (second, _, second_changed) =
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
}

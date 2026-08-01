use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::domain::document::{Document, DocumentRole};
use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::job::{DuplicatePolicy, JobKind};
use crate::jobs::job_manager::{JobManager, JobRegistrationInput};
use crate::platform::file_access::{remove_dir_within, remove_file_within};
use crate::services::pdf_service::PdfService;
use crate::services::project_store::ProjectStore;
use crate::services::student_scan_service::{
    persisted_dependency_jobs, scan_submission_dependencies_with_jobs,
};
use crate::services::workflow_engine;

pub fn import_document(
    project_store: &ProjectStore,
    _pdf_service: &dyn PdfService,
    project_id: &str,
    source_path: &str,
    role: DocumentRole,
) -> Result<Document, AppError> {
    import_document_with_job::<tauri::Wry>(
        project_store,
        _pdf_service,
        project_id,
        source_path,
        role,
        None,
        None,
    )
}

pub fn import_document_with_job<R: tauri::Runtime>(
    project_store: &ProjectStore,
    _pdf_service: &dyn PdfService,
    project_id: &str,
    source_path: &str,
    role: DocumentRole,
    job_manager: Option<(&JobManager, &tauri::AppHandle<R>)>,
    correlation_id: Option<String>,
) -> Result<Document, AppError> {
    let source = fs::canonicalize(Path::new(source_path)).map_err(|error| AppError {
        code: AppErrorCode::DocumentImportFailed,
        message: "Seçilen kaynak dosya okunamadı.".to_string(),
        recoverable: true,
        suggested_action: Some("Geçerli bir PDF dosyası seçin.".to_string()),
        technical_details: Some(error.to_string()),
        correlation_id: correlation_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string()),
    })?;

    if !source.is_file() {
        return Err(AppError {
            code: AppErrorCode::DocumentImportFailed,
            message: "Source file does not exist or is not a file.".to_string(),
            recoverable: true,
            suggested_action: Some("Select a valid PDF file.".to_string()),
            technical_details: Some(format!("Invalid path: {}", source_path)),
            correlation_id: correlation_id
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
        });
    }

    let file_name = source
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document.pdf")
        .to_string();

    let mut project = project_store.get_project_snapshot(project_id.to_string())?;
    let trusted_root = project_store.trusted_project_root(project_id)?;

    let (job_id, cancel_token) = if let Some((jm, app)) = job_manager {
        let reg = jm.register_or_get_active_job(
            app,
            JobRegistrationInput {
                project_id: project_id.to_string(),
                project_root_path: Some(project.root_path.clone()),
                kind: JobKind::DocumentImport,
                display_label: Some(format!("Belge Yükleme: {file_name}")),
                total: 100,
                message: format!("{file_name} kopyalanıyor"),
                correlation_id: correlation_id.clone(),
                idempotency_key: Some(format!(
                    "doc_import:{}:{:?}:{}",
                    project_id,
                    role,
                    source.to_string_lossy()
                )),
                duplicate_policy: DuplicatePolicy::ReturnExisting,
                cancellable: true,
                retry_of_job_id: None,
            },
        )?;
        let _ = jm.set_running(app, &reg.snapshot.id);
        let _ = jm.update_progress(
            app,
            &reg.snapshot.id,
            10,
            100,
            format!("{file_name} hazırlanıyor"),
        );
        (Some(reg.snapshot.id), Some(reg.cancellation_token))
    } else {
        (None, None)
    };

    if let Some(ref token) = cancel_token {
        if token.is_cancelled() {
            if let (Some(ref id), Some((jm, app))) = (&job_id, job_manager) {
                let _ = jm.mark_cancelled(app, id);
            }
            return Err(AppError {
                code: AppErrorCode::DocumentImportFailed,
                message: "Belge import işlemi iptal edildi.".to_string(),
                recoverable: true,
                suggested_action: None,
                technical_details: None,
                correlation_id: correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            });
        }
    }

    let document_id = Uuid::new_v4().to_string();
    let dest_file_name = format!("{}_{}", document_id, safe_file_name(&file_name));
    let managed_path = trusted_root.managed(&format!("documents/{dest_file_name}"))?;
    let dest_path = trusted_root.prepare_write_target(&managed_path)?;

    if let Some((jm, app)) = job_manager {
        if let Some(ref id) = job_id {
            let _ = jm.update_progress(app, id, 30, 100, format!("{file_name} kopyalanıyor"));
        }
    }

    let copy_result = copy_external_file(&source, &dest_path, cancel_token.as_ref());
    if let Err(e) = copy_result {
        if e.kind() == io::ErrorKind::Interrupted {
            if let (Some(ref id), Some((jm, app))) = (&job_id, job_manager) {
                let _ = jm.mark_cancelled(app, id);
            }
            return Err(AppError {
                code: AppErrorCode::DocumentImportFailed,
                message: "Belge kopyalama kullanıcı tarafından iptal edildi.".to_string(),
                recoverable: true,
                suggested_action: None,
                technical_details: Some(e.to_string()),
                correlation_id: correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            });
        }

        let err = AppError {
            code: AppErrorCode::DocumentImportFailed,
            message: "Failed to copy document to project folder.".to_string(),
            recoverable: true,
            suggested_action: Some("Check disk space and permissions.".to_string()),
            technical_details: Some(e.to_string()),
            correlation_id: correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string()),
        };
        if let (Some(ref id), Some((jm, app))) = (&job_id, job_manager) {
            let _ = jm.fail(app, id, err.clone());
        }
        return Err(err);
    }

    if let Some(ref token) = cancel_token {
        if token.is_cancelled() {
            let _ = remove_file_within(trusted_root.root(), &dest_path);
            if let (Some(ref id), Some((jm, app))) = (&job_id, job_manager) {
                let _ = jm.mark_cancelled(app, id);
            }
            return Err(AppError {
                code: AppErrorCode::DocumentImportFailed,
                message: "Belge import işlemi iptal edildi.".to_string(),
                recoverable: true,
                suggested_action: None,
                technical_details: None,
                correlation_id: correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            });
        }
    }

    if let Some((jm, app)) = job_manager {
        if let Some(ref id) = job_id {
            let _ = jm.update_progress(app, id, 80, 100, format!("{file_name} kaydoluyor"));
        }
    }

    let stored_path = managed_path.as_str().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let page_count = 0;

    let document = Document {
        id: document_id.clone(),
        role,
        file_name,
        stored_path,
        page_count,
        added_at: now,
        checksum: None,
        preview: None,
    };

    project.documents.push(document.clone());
    project.workflow = workflow_engine::evaluate_workflow(&project);

    if let Err(error) = project_store.commit_snapshot_cas(&project).map(|_| ()) {
        let _ = remove_file_within(trusted_root.root(), &dest_path);
        if let (Some(ref id), Some((jm, app))) = (&job_id, job_manager) {
            let _ = jm.fail(app, id, error.clone());
        }
        return Err(error);
    }

    if let (Some(ref id), Some((jm, app))) = (&job_id, job_manager) {
        let _ = jm.succeed(app, id, serde_json::to_value(&document).ok());
    }

    Ok(document)
}

pub fn remove_document(
    project_store: &ProjectStore,
    project_id: &str,
    document_id: &str,
) -> Result<(), AppError> {
    let mut project = project_store.get_project_snapshot(project_id.to_string())?;

    let doc_index = project.documents.iter().position(|d| d.id == document_id);
    if let Some(index) = doc_index {
        let doc = project.documents[index].clone();
        let trusted_root = project_store.trusted_project_root(project_id)?;
        let document_path = doc.resolve_path_with_root(&trusted_root)?;
        let removed_submission_ids = project
            .student_submissions
            .iter()
            .filter(|submission| submission.document_id == doc.id)
            .map(|submission| submission.id.clone())
            .collect::<Vec<_>>();
        let has_dependent_results = project
            .student_answer_ocr_records
            .iter()
            .any(|record| removed_submission_ids.contains(&record.submission_id));
        let dependency_scan = scan_submission_dependencies_with_jobs(
            &project,
            &removed_submission_ids,
            &persisted_dependency_jobs(&project)?,
        );
        if has_dependent_results || dependency_scan.is_blocked() {
            return Err(AppError {
                code: AppErrorCode::WorkflowBlocked,
                message: "Belge, kaydedilmiş OCR veya puan sonuçları bulunduğu için silinemez."
                    .to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Sonuçları koruyun veya ilgili öğrenci paketini kontrollü biçimde sıfırlayın."
                        .to_string(),
                ),
                technical_details: Some(format!(
                    "document_id={}; dependent_submissions={}",
                    doc.id,
                    removed_submission_ids.len()
                )),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        project.documents.remove(index);
        let preview_base = trusted_root.root().join("cache").join("page_previews");
        let preview_relative = trusted_root.managed(&format!("cache/page_previews/{}", doc.id))?;
        let preview_dir = trusted_root.root().join(preview_relative.as_path());

        if matches!(doc.role, DocumentRole::StudentScan) {
            project
                .student_submissions
                .retain(|submission| submission.document_id != doc.id);
            project
                .student_scan_batches
                .retain(|batch| batch.document_id != doc.id);
            if project.student_scan_document_id.as_deref() == Some(doc.id.as_str()) {
                project.student_scan_document_id = None;
            }
            project.student_grouping_mode = None;
            project.student_pages_per_student = None;
            project.student_grouping_complete_at = None;
        }

        project.workflow = workflow_engine::evaluate_workflow(&project);
        project_store.commit_snapshot_cas(&project).map(|_| ())?;
        if let Err(error) = remove_file_within(trusted_root.root(), &document_path) {
            log::warn!(
                "Proje kaydı güncellendi ancak belge artığı güvenle silinemedi: document_id={}; error={error}",
                doc.id
            );
        }
        if let Err(error) = remove_dir_within(&preview_base, &preview_dir) {
            log::warn!(
                "Proje kaydı güncellendi ancak önizleme artığı güvenle silinemedi: document_id={}; error={error}",
                doc.id
            );
        }
        let preview_outputs_base = trusted_root.root().join("outputs").join("previews");
        if let Ok(managed) = trusted_root.managed(&format!("outputs/previews/{}", doc.id)) {
            let preview_output_dir = trusted_root.root().join(managed.as_path());
            if let Err(error) = remove_dir_within(&preview_outputs_base, &preview_output_dir) {
                log::warn!(
                    "Proje kaydı güncellendi ancak immutable önizleme artığı silinemedi: document_id={}; error={error}",
                    doc.id
                );
            }
        }
        Ok(())
    } else {
        Err(AppError {
            code: AppErrorCode::DocumentImportFailed,
            message: "Document not found in project.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: None,
            correlation_id: Uuid::new_v4().to_string(),
        })
    }
}

fn safe_file_name(file_name: &str) -> String {
    let candidate = Path::new(file_name)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("document.pdf");
    let sanitized = candidate
        .chars()
        .map(|character| match character {
            '/' | '\\' | '\0' => '_',
            _ => character,
        })
        .collect::<String>();
    if sanitized.trim().is_empty() || sanitized == "." || sanitized == ".." {
        "document.pdf".to_string()
    } else {
        sanitized
    }
}

fn copy_external_file(
    source: &Path,
    destination: &Path,
    cancel_token: Option<&CancellationToken>,
) -> io::Result<()> {
    let mut input = fs::File::open(source)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let result = (|| {
        let mut buffer = [0_u8; 128 * 1024];
        loop {
            if let Some(token) = cancel_token {
                if token.is_cancelled() {
                    return Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "Belge kopyalama iptal edildi.",
                    ));
                }
            }
            let read = input.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            if let Some(token) = cancel_token {
                if token.is_cancelled() {
                    return Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "Belge kopyalama iptal edildi.",
                    ));
                }
            }
            output.write_all(&buffer[..read])?;
        }
        output.sync_all()
    })();
    if result.is_err() {
        drop(output);
        let _ = fs::remove_file(destination);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::document::DocumentRole;
    use crate::services::pdf_service::PdfService;
    use crate::services::project_store::ProjectStore;
    use std::path::{Path, PathBuf};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    struct StubPdfService {
        called: Arc<AtomicBool>,
        page_count: u32,
    }

    impl PdfService for StubPdfService {
        fn page_count(&self, _pdf_path: &Path) -> Result<u32, AppError> {
            self.called.store(true, Ordering::SeqCst);
            Ok(self.page_count)
        }

        fn render_pages(
            &self,
            _pdf_path: &Path,
            _output_dir: &Path,
            _pages: &[u32],
        ) -> Result<Vec<PathBuf>, AppError> {
            Ok(vec![])
        }

        fn render_all_pages(
            &self,
            _pdf_path: &Path,
            _output_dir: &Path,
        ) -> Result<Vec<PathBuf>, AppError> {
            Ok(vec![])
        }

        fn get_renderer_status(
            &self,
        ) -> Result<crate::services::pdf_service::PdfRendererStatus, AppError> {
            Ok(crate::services::pdf_service::PdfRendererStatus {
                available: true,
                backend: "poppler".to_string(),
                pdfinfo_path: Some("/fake/pdfinfo".to_string()),
                pdftoppm_path: Some("/fake/pdftoppm".to_string()),
                searched_paths: vec![],
                path_env: None,
                install_hint: None,
                warnings: vec![],
            })
        }
    }

    fn temp_project_root() -> String {
        let root = std::env::temp_dir().join(format!("rubrika-v3-doc-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        root.to_string_lossy().to_string()
    }

    fn create_source_file(path: &Path, content: &[u8]) {
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn answer_key_json_import_does_not_call_page_count() {
        let root = temp_project_root();
        let store = ProjectStore::new();
        let project = store
            .create_project("Project".to_string(), root.clone())
            .expect("project");

        let source =
            std::env::temp_dir().join(format!("rubrika-v3-rubric-{}.json", Uuid::new_v4()));
        create_source_file(&source, br#"{"rubric":true}"#);

        let called = Arc::new(AtomicBool::new(false));
        let service = StubPdfService {
            called: called.clone(),
            page_count: 7,
        };

        let document = import_document(
            &store,
            &service,
            &project.id,
            source.to_string_lossy().as_ref(),
            DocumentRole::AnswerKey,
        )
        .expect("json import");

        assert_eq!(document.page_count, 0);
        assert!(!called.load(Ordering::SeqCst));
        assert!(document.stored_path.starts_with("documents/"));
        assert!(!document.stored_path.starts_with('/'));
    }

    #[test]
    fn exam_source_import_does_not_call_page_count() {
        let root = temp_project_root();
        let store = ProjectStore::new();
        let project = store
            .create_project("Project".to_string(), root.clone())
            .expect("project");

        let source = std::env::temp_dir().join(format!("rubrika-v3-exam-{}.pdf", Uuid::new_v4()));
        create_source_file(
            &source,
            b"%PDF-1.4\n1 0 obj\n<<>>\nendobj\ntrailer\n<<>>\n%%EOF",
        );

        let called = Arc::new(AtomicBool::new(false));
        let service = StubPdfService {
            called: called.clone(),
            page_count: 3,
        };

        let document = import_document(
            &store,
            &service,
            &project.id,
            source.to_string_lossy().as_ref(),
            DocumentRole::ExamSource,
        )
        .expect("pdf import");

        assert_eq!(document.page_count, 0);
        assert!(!called.load(Ordering::SeqCst));
        assert!(document.stored_path.starts_with("documents/"));
        assert!(!document.stored_path.starts_with('/'));
    }

    #[test]
    fn proof_15_document_import_cancel_never_activates_partial_file() {
        use crate::domain::job::JobStatus;

        let root = temp_project_root();
        let store = ProjectStore::new();
        let project = store
            .create_project("proj_p15".to_string(), root.clone())
            .expect("project");

        let source = std::env::temp_dir().join(format!("rubrika-v3-p15-{}.pdf", Uuid::new_v4()));
        let source_content = b"%PDF-1.4\n%Proof 15 document import test content";
        create_source_file(&source, source_content);

        let called = Arc::new(AtomicBool::new(false));
        let pdf_service = StubPdfService {
            called: called.clone(),
            page_count: 1,
        };

        let jm = JobManager::new();
        let app = tauri::test::mock_app();
        let handle = app.handle();

        let canonical_source = std::fs::canonicalize(&source).unwrap_or(source.clone());
        let idempotency_key = format!(
            "doc_import:{}:{:?}:{}",
            project.id,
            DocumentRole::ExamSource,
            canonical_source.to_string_lossy()
        );

        let reg = jm
            .register_or_get_active_job(
                handle,
                JobRegistrationInput {
                    project_id: project.id.clone(),
                    project_root_path: Some(project.root_path.clone()),
                    kind: JobKind::DocumentImport,
                    display_label: Some("Import Document".into()),
                    total: 100,
                    message: "Importing".into(),
                    correlation_id: Some("corr-p15".into()),
                    idempotency_key: Some(idempotency_key),
                    duplicate_policy: DuplicatePolicy::ReturnExisting,
                    cancellable: true,
                    retry_of_job_id: None,
                },
            )
            .unwrap();

        jm.set_running(handle, &reg.snapshot.id).unwrap();
        reg.cancellation_token.cancel();

        let res = import_document_with_job(
            &store,
            &pdf_service,
            &project.id,
            source.to_string_lossy().as_ref(),
            DocumentRole::ExamSource,
            Some((&jm, handle)),
            Some("corr-p15".into()),
        );

        assert!(res.is_err());
        assert_eq!(res.unwrap_err().code, AppErrorCode::DocumentImportFailed);

        let snap = jm.get_job_snapshot(&reg.snapshot.id).unwrap();
        assert_eq!(snap.status, JobStatus::Cancelled);

        // Verify project documents list does NOT contain partial document
        let updated_project = store.get_project_snapshot(project.id).unwrap();
        assert_eq!(updated_project.documents.len(), 0);

        // Verify source file content is untouched
        let source_read = std::fs::read(&source).unwrap();
        assert_eq!(source_read, source_content);
    }
}

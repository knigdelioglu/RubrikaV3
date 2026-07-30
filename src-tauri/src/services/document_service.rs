use std::path::Path;
use uuid::Uuid;

use crate::domain::document::{Document, DocumentRole};
use crate::domain::errors::{AppError, AppErrorCode};
use crate::platform::file_access::{remove_dir_within, remove_file_within};
use crate::services::pdf_service::PdfService;
use crate::services::project_store::ProjectStore;
use crate::services::workflow_engine;

pub fn import_document(
    project_store: &ProjectStore,
    _pdf_service: &dyn PdfService,
    project_id: &str,
    source_path: &str,
    role: DocumentRole,
) -> Result<Document, AppError> {
    let source = Path::new(source_path);
    if !source.exists() || !source.is_file() {
        return Err(AppError {
            code: AppErrorCode::DocumentImportFailed,
            message: "Source file does not exist or is not a file.".to_string(),
            recoverable: true,
            suggested_action: Some("Select a valid PDF file.".to_string()),
            technical_details: Some(format!("Invalid path: {}", source_path)),
            correlation_id: Uuid::new_v4().to_string(),
        });
    }

    let file_name = source
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document.pdf")
        .to_string();

    let mut project = project_store.get_project_snapshot(project_id.to_string())?;

    let document_id = Uuid::new_v4().to_string();
    let dest_file_name = format!("{}_{}", document_id, file_name);
    let dest_path = Path::new(&project.root_path)
        .join("documents")
        .join(&dest_file_name);

    std::fs::copy(source, &dest_path).map_err(|e| AppError {
        code: AppErrorCode::DocumentImportFailed,
        message: "Failed to copy document to project folder.".to_string(),
        recoverable: true,
        suggested_action: Some("Check disk space and permissions.".to_string()),
        technical_details: Some(e.to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    })?;

    let stored_path = dest_path.to_string_lossy().to_string();
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

    // Update workflow
    project.workflow = workflow_engine::evaluate_workflow(&project);

    if let Err(error) = project_store.save_project(&project) {
        let documents_dir = Path::new(&project.root_path).join("documents");
        let _ = remove_file_within(&documents_dir, &dest_path);
        return Err(error);
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
        let removed_submission_ids = project
            .student_submissions
            .iter()
            .filter(|submission| submission.document_id == doc.id)
            .map(|submission| submission.id.clone())
            .collect::<Vec<_>>();
        let has_dependent_results = project
            .student_answer_ocr_records
            .iter()
            .any(|record| removed_submission_ids.contains(&record.submission_id))
            || project
                .scoring_records
                .iter()
                .any(|record| removed_submission_ids.contains(&record.submission_id));
        if has_dependent_results {
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
        let preview_base = Path::new(&project.root_path)
            .join("cache")
            .join("page_previews");
        let preview_dir = preview_base.join(&doc.id);

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
        project_store.save_project(&project)?;
        let documents_dir = Path::new(&project.root_path).join("documents");
        if let Err(error) = remove_file_within(&documents_dir, Path::new(&doc.stored_path)) {
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
    }
}

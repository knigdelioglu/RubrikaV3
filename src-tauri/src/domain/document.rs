use crate::domain::errors::{AppError, AppErrorCode};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PdfPreviewStatus {
    Missing,
    Queued,
    Running,
    Ready,
    Failed,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PdfPreviewState {
    pub status: PdfPreviewStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rendered_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PdfPagePreview {
    pub document_id: String,
    pub page_number: u32,
    pub image_path: String,
    pub width: u32,
    pub height: u32,
    pub rendered_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentRole {
    StudentScan,
    ExamSource,
    AnswerKey,
    Rubric,
    Export,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: String,
    pub role: DocumentRole,
    pub file_name: String,
    pub stored_path: String,
    pub page_count: u32,
    pub added_at: String,
    pub checksum: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<PdfPreviewState>,
}

impl Document {
    pub fn resolve_path(&self, project_root: &str) -> Result<PathBuf, AppError> {
        let path = Path::new(&self.stored_path);
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            let direct = Path::new(project_root).join(path);
            if direct.exists() && direct.is_file() {
                direct
            } else {
                Path::new(project_root).join("documents").join(path)
            }
        };

        if !resolved.exists() || !resolved.is_file() {
            return Err(AppError {
                code: AppErrorCode::PdfDocumentNotFound,
                message: format!("Belge dosyası bulunamadı: {}", self.file_name),
                recoverable: true,
                suggested_action: Some("Lütfen belgeyi yeniden yükleyin.".to_string()),
                technical_details: Some(format!(
                    "stored_path={:?}, resolved={:?}, project_root={:?}",
                    self.stored_path, resolved, project_root
                )),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn temp_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("rubrika-test-doc-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn test_resolve_path_absolute() {
        let root = temp_root();
        let file_path = root.join("exam.pdf");
        File::create(&file_path).unwrap();

        let doc = Document {
            id: "d1".to_string(),
            role: DocumentRole::ExamSource,
            file_name: "exam.pdf".to_string(),
            stored_path: file_path.to_string_lossy().to_string(),
            page_count: 1,
            added_at: "now".to_string(),
            checksum: None,
            preview: None,
        };

        let resolved = doc.resolve_path("/nonexistent_root").unwrap();
        assert_eq!(resolved, file_path);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_resolve_path_relative_direct() {
        let root = temp_root();
        let file_path = root.join("exam.pdf");
        File::create(&file_path).unwrap();

        let doc = Document {
            id: "d1".to_string(),
            role: DocumentRole::ExamSource,
            file_name: "exam.pdf".to_string(),
            stored_path: "exam.pdf".to_string(),
            page_count: 1,
            added_at: "now".to_string(),
            checksum: None,
            preview: None,
        };

        let resolved = doc.resolve_path(&root.to_string_lossy()).unwrap();
        assert_eq!(resolved, file_path);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_resolve_path_relative_documents() {
        let root = temp_root();
        let docs_dir = root.join("documents");
        std::fs::create_dir_all(&docs_dir).unwrap();
        let file_path = docs_dir.join("exam.pdf");
        File::create(&file_path).unwrap();

        let doc = Document {
            id: "d1".to_string(),
            role: DocumentRole::ExamSource,
            file_name: "exam.pdf".to_string(),
            stored_path: "exam.pdf".to_string(),
            page_count: 1,
            added_at: "now".to_string(),
            checksum: None,
            preview: None,
        };

        let resolved = doc.resolve_path(&root.to_string_lossy()).unwrap();
        assert_eq!(resolved, file_path);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_resolve_path_not_found() {
        let doc = Document {
            id: "d1".to_string(),
            role: DocumentRole::ExamSource,
            file_name: "exam.pdf".to_string(),
            stored_path: "exam.pdf".to_string(),
            page_count: 1,
            added_at: "now".to_string(),
            checksum: None,
            preview: None,
        };

        let result = doc.resolve_path("/nonexistent_root");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, AppErrorCode::PdfDocumentNotFound);
    }
}

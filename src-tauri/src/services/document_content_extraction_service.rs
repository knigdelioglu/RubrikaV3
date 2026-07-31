use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::model::{ModelInputImage, ModelInputImageKind};
use crate::platform::project_paths::TrustedProjectRoot;
use crate::services::model_input_image_service::{ModelInputBatchMetadata, ModelInputImageService};

const MIN_NON_WHITESPACE_LENGTH: usize = 200;

#[derive(Clone)]
pub struct DocumentContentExtractionService {
    pub model_input_image_service: Arc<ModelInputImageService>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentContentKind {
    ExamSource,
    Rubric,
    AnswerKey,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentContentExtractionMethod {
    PdfToText,
    VisionFallbackPrepared,
    Cached,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextQualitySummary {
    pub enough_text: bool,
    pub detected_question_numbers: Vec<u32>,
    pub missing_question_numbers: Vec<u32>,
    pub likely_scanned_pdf: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DocumentContentExtractionRequest {
    pub project_id: String,
    pub project_root: PathBuf,
    pub document_id: String,
    pub document_path: PathBuf,
    pub kind: DocumentContentKind,
    pub expected_question_count: Option<u32>,
    pub force_refresh: bool,
    pub vision_sources: Vec<(u32, PathBuf)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentContentExtractionResult {
    pub document_id: String,
    pub kind: DocumentContentKind,
    pub method: DocumentContentExtractionMethod,
    pub raw_text: Option<String>,
    pub normalized_text: Option<String>,
    pub raw_text_length: usize,
    pub non_whitespace_length: usize,
    pub normalized_text_length: usize,
    pub page_count: Option<u32>,
    pub text_quality: TextQualitySummary,
    pub vision_fallback_needed: bool,
    #[serde(default)]
    pub ignored_question_numbers: Vec<u32>,
    #[serde(default)]
    pub model_input_images: Vec<ModelInputImage>,
    pub artifact_dir: PathBuf,
    pub raw_text_path: Option<PathBuf>,
    pub normalized_text_path: Option<PathBuf>,
    pub pdftotext_stderr_path: Option<PathBuf>,
    pub model_input_manifest_path: Option<PathBuf>,
    pub metadata_path: PathBuf,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DocumentContentCacheMetadata {
    project_id: String,
    document_id: String,
    kind: DocumentContentKind,
    method: DocumentContentExtractionMethod,
    source_file_size: u64,
    source_modified_at: Option<String>,
    expected_question_count: Option<u32>,
    raw_text_length: usize,
    non_whitespace_length: usize,
    normalized_text_length: usize,
    page_count: Option<u32>,
    enough_text: bool,
    vision_fallback_needed: bool,
    detected_question_numbers: Vec<u32>,
    missing_question_numbers: Vec<u32>,
    #[serde(default)]
    ignored_question_numbers: Vec<u32>,
    likely_scanned_pdf: bool,
    reason: Option<String>,
    warnings: Vec<String>,
    raw_text_path: Option<String>,
    normalized_text_path: Option<String>,
    pdftotext_stderr_path: Option<String>,
    model_input_manifest_path: Option<String>,
    artifact_dir: String,
    updated_at: String,
}

impl DocumentContentExtractionService {
    pub fn new(model_input_image_service: Arc<ModelInputImageService>) -> Self {
        Self {
            model_input_image_service,
        }
    }

    pub fn extract(
        &self,
        request: DocumentContentExtractionRequest,
    ) -> Result<DocumentContentExtractionResult, AppError> {
        let trusted_root =
            TrustedProjectRoot::from_canonical_root(request.project_root.clone(), false)?;
        let artifact_relative =
            trusted_root.managed(&format!("cache/document_content/{}", request.document_id))?;
        let artifact_dir = trusted_root.root().join(artifact_relative.as_path());
        let metadata_path = artifact_dir.join("content_metadata.json");
        if !request.force_refresh {
            if let Some(cached) = self.try_load_cached(&request, &artifact_dir, &metadata_path)? {
                return Ok(cached);
            }
        }

        trusted_root.ensure_managed_directory(&artifact_dir)?;

        let source_metadata = std::fs::metadata(&request.document_path).map_err(|error| {
            app_error(
                AppErrorCode::FileReadFailed,
                "PDF dosyası okunamadı.",
                Some(error.to_string()),
                Some("Check the source PDF path.".to_string()),
            )
        })?;
        let source_file_size = source_metadata.len();
        let source_modified_at = source_metadata.modified().ok().map(system_time_to_rfc3339);

        let raw_text_path = artifact_dir.join("raw_text.txt");
        let normalized_text_path = artifact_dir.join("normalized_text.txt");
        let stderr_path = artifact_dir.join("pdftotext_stderr.txt");
        let mut warnings = Vec::new();
        let output = Command::new("pdftotext")
            .arg("-raw")
            .arg(&request.document_path)
            .arg("-")
            .output();
        let raw_text = match output {
            Ok(output) if output.status.success() => {
                String::from_utf8_lossy(&output.stdout).to_string()
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let _ = atomic_write_managed(&trusted_root, &stderr_path, &stderr);
                warnings.push("pdftotext_failed".to_string());
                warnings.push("vision_fallback_recommended".to_string());
                String::new()
            }
            Err(error) => {
                let stderr = error.to_string();
                let _ = atomic_write_managed(&trusted_root, &stderr_path, &stderr);
                warnings.push("pdftotext_unavailable".to_string());
                warnings.push("vision_fallback_recommended".to_string());
                String::new()
            }
        };
        let normalized_text = normalize_text(&raw_text);
        let raw_text_length = raw_text.len();
        let non_whitespace_length = raw_text.chars().filter(|c| !c.is_whitespace()).count();
        let normalized_text_length = normalized_text.len();
        let markers = detect_question_markers(&raw_text);
        let (markers, ignored_question_numbers) =
            clamp_question_markers(markers, request.expected_question_count);
        let detected_question_numbers = markers.keys().copied().collect::<Vec<_>>();
        let missing_question_numbers = request
            .expected_question_count
            .map(|expected| missing_numbers(expected, &detected_question_numbers))
            .unwrap_or_default();
        let enough_text = non_whitespace_length >= MIN_NON_WHITESPACE_LENGTH;
        let likely_scanned_pdf = !enough_text || raw_text.trim().is_empty();
        let vision_fallback_needed = match request.kind {
            DocumentContentKind::Rubric | DocumentContentKind::AnswerKey => !enough_text,
            _ => !enough_text || !missing_question_numbers.is_empty(),
        };
        if !enough_text {
            warnings.push("pdf_text_below_threshold".to_string());
        }
        if !missing_question_numbers.is_empty() {
            match request.kind {
                DocumentContentKind::Rubric | DocumentContentKind::AnswerKey => {
                    warnings.push("question_markers_incomplete".to_string());
                }
                _ => {
                    warnings.push(format!(
                        "question_coverage_incomplete missing={:?}",
                        missing_question_numbers
                    ));
                }
            }
        }
        if likely_scanned_pdf {
            warnings.push("likely_scanned_pdf".to_string());
        }

        atomic_write_managed(&trusted_root, &raw_text_path, &raw_text)?;
        atomic_write_managed(&trusted_root, &normalized_text_path, &normalized_text)?;

        let mut model_input_images = Vec::new();
        let mut model_input_manifest_path = None;
        if vision_fallback_needed {
            if request.vision_sources.is_empty() {
                warnings.push("vision_sources_missing".to_string());
            } else {
                let model_input_kind = model_input_kind_for(&request.kind);
                let manifest_path = ModelInputImageService::manifest_path(
                    &request.project_root,
                    &model_input_kind,
                    &request.document_id,
                )?;
                if !manifest_path.exists() {
                    model_input_images = self.model_input_image_service.prepare_inputs(
                        &request.project_root,
                        model_input_kind,
                        &request.document_id,
                        &request.vision_sources,
                    )?;
                } else if let Ok(manifest) = load_model_input_manifest(&manifest_path) {
                    ModelInputImageService::validate_manifest_paths(&trusted_root, &manifest)?;
                    model_input_images = manifest.images;
                }
                model_input_manifest_path = Some(manifest_path);
            }
        }

        let method = if vision_fallback_needed && !model_input_images.is_empty() {
            DocumentContentExtractionMethod::VisionFallbackPrepared
        } else {
            DocumentContentExtractionMethod::PdfToText
        };
        let page_count = if request.vision_sources.is_empty() {
            None
        } else {
            Some(request.vision_sources.len() as u32)
        };

        let result = DocumentContentExtractionResult {
            document_id: request.document_id.clone(),
            kind: request.kind.clone(),
            method: method.clone(),
            raw_text: Some(raw_text.clone()),
            normalized_text: Some(normalized_text.clone()),
            raw_text_length,
            non_whitespace_length,
            normalized_text_length,
            page_count,
            text_quality: TextQualitySummary {
                enough_text,
                detected_question_numbers: detected_question_numbers.clone(),
                missing_question_numbers: missing_question_numbers.clone(),
                likely_scanned_pdf,
                reason: if enough_text {
                    None
                } else {
                    Some("raw_text_below_threshold".to_string())
                },
            },
            vision_fallback_needed,
            ignored_question_numbers: ignored_question_numbers.clone(),
            model_input_images: model_input_images.clone(),
            artifact_dir: artifact_dir.clone(),
            raw_text_path: Some(raw_text_path.clone()),
            normalized_text_path: Some(normalized_text_path.clone()),
            pdftotext_stderr_path: Some(stderr_path.clone()),
            model_input_manifest_path: model_input_manifest_path.clone(),
            metadata_path: metadata_path.clone(),
            warnings: warnings.clone(),
        };

        let metadata = DocumentContentCacheMetadata {
            project_id: request.project_id,
            document_id: request.document_id,
            kind: request.kind,
            method,
            source_file_size,
            source_modified_at,
            expected_question_count: request.expected_question_count,
            raw_text_length,
            non_whitespace_length,
            normalized_text_length,
            page_count,
            enough_text,
            vision_fallback_needed,
            detected_question_numbers,
            missing_question_numbers,
            ignored_question_numbers,
            likely_scanned_pdf,
            reason: result.text_quality.reason.clone(),
            warnings,
            raw_text_path: Some(raw_text_path.to_string_lossy().to_string()),
            normalized_text_path: Some(normalized_text_path.to_string_lossy().to_string()),
            pdftotext_stderr_path: Some(stderr_path.to_string_lossy().to_string()),
            model_input_manifest_path: model_input_manifest_path
                .as_ref()
                .map(|path| path.to_string_lossy().to_string()),
            artifact_dir: artifact_dir.to_string_lossy().to_string(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        atomic_write_managed(
            &trusted_root,
            &metadata_path,
            &serde_json::to_string_pretty(&metadata).map_err(|error| {
                app_error(
                    AppErrorCode::ProjectSaveFailed,
                    "Belge içerik metadata yazılamadı.",
                    Some(error.to_string()),
                    Some("Check project cache permissions.".to_string()),
                )
            })?,
        )
        .map_err(|mut error| {
            error.message = "Belge içerik metadata yazılamadı.".to_string();
            error
        })?;

        Ok(result)
    }

    fn try_load_cached(
        &self,
        request: &DocumentContentExtractionRequest,
        artifact_dir: &Path,
        metadata_path: &Path,
    ) -> Result<Option<DocumentContentExtractionResult>, AppError> {
        if !metadata_path.exists() {
            return Ok(None);
        }
        let trusted_root =
            TrustedProjectRoot::from_canonical_root(request.project_root.clone(), false)?;
        let metadata_managed = trusted_root.managed_for_path(metadata_path)?;
        let metadata_path = trusted_root.resolve_existing_file(&metadata_managed)?;
        let metadata: DocumentContentCacheMetadata = match std::fs::read_to_string(&metadata_path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(metadata) => metadata,
                Err(_) => return Ok(None),
            },
            Err(_) => return Ok(None),
        };
        let source_metadata = std::fs::metadata(&request.document_path).map_err(|error| {
            app_error(
                AppErrorCode::FileReadFailed,
                "PDF dosyası okunamadı.",
                Some(error.to_string()),
                Some("Check the source PDF path.".to_string()),
            )
        })?;
        if metadata.source_file_size != source_metadata.len() {
            return Ok(None);
        }
        if let (Some(saved_modified), Some(current_modified)) = (
            metadata.source_modified_at.as_deref(),
            source_metadata.modified().ok().map(system_time_to_rfc3339),
        ) {
            if saved_modified != current_modified {
                return Ok(None);
            }
        }

        let raw_text =
            read_optional_managed_text(&trusted_root, &artifact_dir.join("raw_text.txt"));
        let normalized_text =
            read_optional_managed_text(&trusted_root, &artifact_dir.join("normalized_text.txt"));
        let mut model_input_manifest_path = metadata
            .model_input_manifest_path
            .as_ref()
            .map(PathBuf::from);
        if let Some(path) = &model_input_manifest_path {
            let trusted_root =
                TrustedProjectRoot::from_canonical_root(request.project_root.clone(), false)?;
            let managed = trusted_root.adapt_legacy_document_path(&path.to_string_lossy())?;
            let safe_path = trusted_root.resolve_existing_file(&managed)?;
            model_input_manifest_path = Some(safe_path);
        }
        let mut model_input_images = if metadata.vision_fallback_needed {
            if let Some(path) = &model_input_manifest_path {
                load_model_input_manifest(path)
                    .and_then(|manifest| {
                        ModelInputImageService::validate_manifest_paths(&trusted_root, &manifest)
                            .map(|_| manifest)
                    })
                    .map(|manifest| manifest.images)
                    .unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        let mut warnings = metadata.warnings.clone();
        if metadata.vision_fallback_needed
            && model_input_images.is_empty()
            && !request.vision_sources.is_empty()
        {
            let model_input_kind = model_input_kind_for(&metadata.kind);
            let manifest_path = ModelInputImageService::manifest_path(
                &request.project_root,
                &model_input_kind,
                &metadata.document_id,
            )?;
            model_input_images = self.model_input_image_service.prepare_inputs(
                &request.project_root,
                model_input_kind,
                &metadata.document_id,
                &request.vision_sources,
            )?;
            model_input_manifest_path = Some(manifest_path);
            warnings.push("vision_inputs_rebuilt_from_cache".to_string());
        }
        let vision_fallback_needed = match metadata.kind {
            DocumentContentKind::Rubric | DocumentContentKind::AnswerKey => !metadata.enough_text,
            _ => metadata.vision_fallback_needed,
        };
        Ok(Some(DocumentContentExtractionResult {
            document_id: metadata.document_id,
            kind: metadata.kind,
            method: DocumentContentExtractionMethod::Cached,
            raw_text,
            normalized_text,
            raw_text_length: metadata.raw_text_length,
            non_whitespace_length: metadata.non_whitespace_length,
            normalized_text_length: metadata.normalized_text_length,
            page_count: metadata.page_count,
            text_quality: TextQualitySummary {
                enough_text: metadata.enough_text,
                detected_question_numbers: metadata.detected_question_numbers,
                missing_question_numbers: metadata.missing_question_numbers,
                likely_scanned_pdf: metadata.likely_scanned_pdf,
                reason: metadata.reason,
            },
            vision_fallback_needed,
            ignored_question_numbers: metadata.ignored_question_numbers,
            model_input_images,
            artifact_dir: artifact_dir.to_path_buf(),
            raw_text_path: metadata.raw_text_path.map(PathBuf::from),
            normalized_text_path: metadata.normalized_text_path.map(PathBuf::from),
            pdftotext_stderr_path: metadata.pdftotext_stderr_path.map(PathBuf::from),
            model_input_manifest_path,
            metadata_path: metadata_path.to_path_buf(),
            warnings,
        }))
    }
}

pub fn normalize_question_detection_text(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\u{c}', "\n\n")
}

pub fn normalize_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn detect_question_markers(text: &str) -> BTreeMap<u32, usize> {
    let normalized = normalize_question_detection_text(text);
    let mut markers = BTreeMap::new();
    let mut offset = 0usize;
    for line in normalized.lines() {
        let trimmed = line.trim_start();
        if let Some((number, start_in_line)) = detect_marker_in_line(trimmed) {
            markers
                .entry(number)
                .or_insert(offset + line.len() - trimmed.len() + start_in_line);
        }
        offset += line.len() + 1;
    }
    markers
}

pub fn clamp_question_markers(
    markers: BTreeMap<u32, usize>,
    expected_question_count: Option<u32>,
) -> (BTreeMap<u32, usize>, Vec<u32>) {
    let Some(expected_question_count) = expected_question_count else {
        return (markers, Vec::new());
    };

    let mut filtered = BTreeMap::new();
    let mut ignored = Vec::new();
    for (number, offset) in markers {
        if number <= expected_question_count {
            filtered.entry(number).or_insert(offset);
        } else {
            ignored.push(number);
        }
    }
    (filtered, ignored)
}

pub fn missing_numbers(expected_count: u32, detected_numbers: &[u32]) -> Vec<u32> {
    let detected = detected_numbers
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    (1..=expected_count)
        .filter(|number| !detected.contains(number))
        .collect()
}

fn detect_marker_in_line(line: &str) -> Option<(u32, usize)> {
    let trimmed = line.trim_start();
    let offset = line.len().saturating_sub(trimmed.len());
    let lower = trimmed.to_lowercase();
    let mut start = if lower.starts_with("soru") {
        "soru".len()
    } else if lower.starts_with("question") {
        "question".len()
    } else if trimmed.starts_with('s') || trimmed.starts_with('S') {
        1
    } else {
        0
    };

    while let Some(ch) = trimmed[start..].chars().next() {
        if ch.is_whitespace() || matches!(ch, '.' | ')' | '-' | ':' | '(' | '[' | ']') {
            start += ch.len_utf8();
        } else {
            break;
        }
    }

    let mut digits = String::new();
    for ch in trimmed[start..].chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else {
            break;
        }
    }

    if digits.is_empty() {
        return None;
    }

    digits.parse::<u32>().ok().map(|number| (number, offset))
}

fn read_optional_text(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

fn read_optional_managed_text(trusted_root: &TrustedProjectRoot, path: &Path) -> Option<String> {
    let managed = trusted_root.managed_for_path(path).ok()?;
    let safe_path = trusted_root.resolve_existing_file(&managed).ok()?;
    read_optional_text(&safe_path)
}

fn atomic_write_managed(
    trusted_root: &TrustedProjectRoot,
    path: &Path,
    content: &str,
) -> Result<(), AppError> {
    let managed = trusted_root.managed_for_path(path)?;
    trusted_root.atomic_write(&managed, content)
}

fn load_model_input_manifest(path: &Path) -> Result<ModelInputBatchMetadata, AppError> {
    let content = std::fs::read_to_string(path).map_err(|error| {
        app_error(
            AppErrorCode::FileReadFailed,
            "Model input manifest okunamadı.",
            Some(error.to_string()),
            Some("Rebuild the model input cache.".to_string()),
        )
    })?;
    serde_json::from_str(&content).map_err(|error| {
        app_error(
            AppErrorCode::ProjectLoadFailed,
            "Model input manifest bozuk.",
            Some(error.to_string()),
            Some("Rebuild the model input cache.".to_string()),
        )
    })
}

fn model_input_kind_for(kind: &DocumentContentKind) -> ModelInputImageKind {
    match kind {
        DocumentContentKind::ExamSource => ModelInputImageKind::QuestionText,
        DocumentContentKind::Rubric | DocumentContentKind::AnswerKey => ModelInputImageKind::Rubric,
    }
}

fn system_time_to_rfc3339(time: std::time::SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Utc> = time.into();
    datetime.to_rfc3339()
}

fn app_error(
    code: AppErrorCode,
    message: &str,
    technical_details: Option<String>,
    suggested_action: Option<String>,
) -> AppError {
    AppError {
        code,
        message: message.to_string(),
        recoverable: true,
        suggested_action,
        technical_details,
        correlation_id: Uuid::new_v4().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_text_collapses_whitespace() {
        assert_eq!(normalize_text("a  b\nc\t d"), "a b c d");
    }

    #[test]
    fn detect_question_markers_finds_common_prefixes() {
        let markers = detect_question_markers("Soru 1.\n2)\n3 -\nQuestion 4");
        assert_eq!(
            markers.keys().copied().collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
    }

    #[test]
    fn missing_numbers_reports_coverage_gaps() {
        assert_eq!(missing_numbers(6, &[1, 2, 3, 4, 5]), vec![6]);
    }

    #[test]
    fn test_try_load_cached_rubric_loosened_fallback() {
        let root =
            std::env::temp_dir().join(format!("rubrika-test-cache-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let pdf_path = root.join("rubric.pdf");
        std::fs::write(&pdf_path, "dummy pdf content").unwrap();

        let metadata_dir = root.join("cache").join("document_content").join("d1");
        std::fs::create_dir_all(&metadata_dir).unwrap();

        let metadata = DocumentContentCacheMetadata {
            project_id: "p1".to_string(),
            document_id: "d1".to_string(),
            kind: DocumentContentKind::Rubric,
            method: DocumentContentExtractionMethod::PdfToText,
            source_file_size: 17,
            source_modified_at: std::fs::metadata(&pdf_path)
                .unwrap()
                .modified()
                .ok()
                .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339()),
            expected_question_count: Some(5),
            raw_text_length: 300,
            non_whitespace_length: 250,
            normalized_text_length: 300,
            page_count: Some(1),
            enough_text: true,
            vision_fallback_needed: true,
            detected_question_numbers: vec![1, 2, 3, 4],
            missing_question_numbers: vec![5],
            ignored_question_numbers: vec![],
            likely_scanned_pdf: false,
            reason: None,
            warnings: vec![],
            raw_text_path: None,
            normalized_text_path: None,
            pdftotext_stderr_path: None,
            model_input_manifest_path: None,
            artifact_dir: metadata_dir.to_string_lossy().to_string(),
            updated_at: "".to_string(),
        };

        let metadata_path = metadata_dir.join("content_metadata.json");
        std::fs::write(&metadata_path, serde_json::to_string(&metadata).unwrap()).unwrap();

        std::fs::write(metadata_dir.join("raw_text.txt"), "Soru 1. ".repeat(40)).unwrap();
        std::fs::write(
            metadata_dir.join("normalized_text.txt"),
            "Soru 1. ".repeat(40),
        )
        .unwrap();

        let image_service = std::sync::Arc::new(ModelInputImageService::default());
        let service = DocumentContentExtractionService::new(image_service);

        let req = DocumentContentExtractionRequest {
            project_id: "p1".to_string(),
            project_root: root.clone(),
            document_id: "d1".to_string(),
            document_path: pdf_path.clone(),
            kind: DocumentContentKind::Rubric,
            expected_question_count: Some(5),
            force_refresh: false,
            vision_sources: vec![],
        };

        let result = service.extract(req).unwrap();
        assert!(!result.vision_fallback_needed);

        let _ = std::fs::remove_dir_all(&root);
    }
}

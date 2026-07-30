use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use image::GenericImageView;
use uuid::Uuid;

use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::project::Project;
use crate::domain::student::{
    StudentAnswerCropTemplateItem, StudentAnswerOcrCropBBox, StudentAnswerOcrRenderDiagnostics,
    StudentIdentityCropTemplate, StudentSubmission,
};
use crate::services::pdf_preview_service::PdfPreviewService;
use crate::services::project_store::ProjectStore;
use crate::services::workflow_engine;

#[derive(Clone)]
pub struct StudentAnswerCropService {
    project_store: ProjectStore,
    pdf_preview_service: Arc<PdfPreviewService>,
}

pub struct StudentAnswerCropArtifacts {
    pub model_input_images: Vec<(u32, PathBuf)>,
    pub source_page_numbers: Vec<u32>,
    pub render_diagnostics: StudentAnswerOcrRenderDiagnostics,
    pub layout_hint: String,
}

pub struct StudentIdentityCropArtifacts {
    pub model_input_images: Vec<(u32, PathBuf)>,
    pub source_page_numbers: Vec<u32>,
    pub crop_refs: Vec<String>,
}

pub struct StudentAnswerIssueCropArtifacts {
    pub model_input_image: PathBuf,
    pub crop_created: bool,
    pub crop_bbox: Option<StudentAnswerOcrCropBBox>,
}

impl StudentAnswerCropService {
    pub fn new(project_store: ProjectStore, pdf_preview_service: Arc<PdfPreviewService>) -> Self {
        Self {
            project_store,
            pdf_preview_service,
        }
    }

    pub fn prepare_source_artifacts(
        &self,
        project_id: &str,
        project_root: &str,
        document_id: &str,
        submission: &StudentSubmission,
        question: &crate::domain::question::Question,
    ) -> Result<StudentAnswerCropArtifacts, AppError> {
        let project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        let previews = self
            .pdf_preview_service
            .require_ready_page_previews(&project.id, document_id)?;
        let preview_by_page: BTreeMap<u32, PathBuf> = previews
            .iter()
            .map(|preview| (preview.page_number, PathBuf::from(&preview.image_path)))
            .collect();
        let template_item = project
            .student_answer_crop_template
            .items
            .iter()
            .find(|item| item.question_id == question.id)
            .cloned();

        self.build_sources(
            project_root,
            document_id,
            submission,
            question,
            &preview_by_page,
            template_item.as_ref(),
        )
    }

    pub fn save_template(
        &self,
        project_id: &str,
        mut items: Vec<StudentAnswerCropTemplateItem>,
    ) -> Result<Project, AppError> {
        let mut project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        items.sort_by_key(|item| item.question_number);
        let template_changed = project.student_answer_crop_template.items != items;
        project.student_answer_crop_template.items = items;
        project.student_answer_crop_template.updated_at = Some(chrono::Utc::now().to_rfc3339());
        if template_changed {
            project.student_answer_ocr_records.clear();
        }
        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store.save_project(&project)?;
        Ok(project)
    }

    pub fn save_identity_template(
        &self,
        project_id: &str,
        mut template: StudentIdentityCropTemplate,
    ) -> Result<Project, AppError> {
        let mut project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        template.label = if template.label.trim().is_empty() {
            "identity_header".to_string()
        } else {
            template.label.trim().to_string()
        };
        template.updated_at = Some(chrono::Utc::now().to_rfc3339());
        project.student_identity_crop_template = Some(template);
        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store.save_project(&project)?;
        Ok(project)
    }

    pub fn prepare_identity_artifacts(
        &self,
        project_id: &str,
        project_root: &str,
        document_id: &str,
        submission: &StudentSubmission,
    ) -> Result<StudentIdentityCropArtifacts, AppError> {
        let project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        let template = project
            .student_identity_crop_template
            .as_ref()
            .ok_or_else(|| AppError {
                code: AppErrorCode::CropRegionMissing,
                message: "Kimlik alanı crop şablonu eksik.".to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Önce Crop Şablonu sayfasında kimlik alanını seçin.".to_string(),
                ),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        let previews = self
            .pdf_preview_service
            .require_ready_page_previews(&project.id, document_id)?;
        let page_number = submission
            .page_numbers
            .get(template.page_index_within_submission as usize)
            .copied()
            .ok_or_else(|| AppError {
                code: AppErrorCode::CropRegionMissing,
                message: "Kimlik crop sayfası öğrenci grubunda bulunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Kimlik crop şablonunu kontrol edin.".to_string()),
                technical_details: Some(format!("submission_id={}", submission.id)),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        let preview_path = previews
            .iter()
            .find(|preview| preview.page_number == page_number)
            .map(|preview| PathBuf::from(&preview.image_path))
            .ok_or_else(|| AppError {
                code: AppErrorCode::PdfRenderFailed,
                message: "Kimlik OCR sayfa önizlemesi bulunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Öğrenci PDF önizlemelerini yeniden oluşturun.".to_string()),
                technical_details: Some(format!("page_number={page_number}")),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        let crop_render = self.crop_identity_preview_image(
            project_root,
            document_id,
            &submission.id,
            page_number,
            &preview_path,
            template,
        )?;
        Ok(StudentIdentityCropArtifacts {
            model_input_images: vec![(page_number, crop_render.path.clone())],
            source_page_numbers: vec![page_number],
            crop_refs: vec![crop_render.path.to_string_lossy().to_string()],
        })
    }

    pub fn crop_issue_region(
        &self,
        output_dir: &Path,
        output_name: &str,
        base_image_path: &Path,
        highlight_region: Option<&StudentAnswerOcrCropBBox>,
    ) -> Result<StudentAnswerIssueCropArtifacts, AppError> {
        if highlight_region.is_none() {
            return Ok(StudentAnswerIssueCropArtifacts {
                model_input_image: base_image_path.to_path_buf(),
                crop_created: false,
                crop_bbox: None,
            });
        }

        let Some(highlight_region) = highlight_region else {
            return Ok(StudentAnswerIssueCropArtifacts {
                model_input_image: base_image_path.to_path_buf(),
                crop_created: false,
                crop_bbox: None,
            });
        };
        let image = image::open(base_image_path).map_err(|error| AppError {
            code: AppErrorCode::PdfRenderFailed,
            message: "OCR issue crop görüntüsü açılamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("Kaynak crop görüntüsünü kontrol edin.".to_string()),
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        let (width, height) = image.dimensions();
        let dummy_template = StudentAnswerCropTemplateItem {
            question_id: "ocr_issue".to_string(),
            question_number: 0,
            page_index_within_submission: highlight_region.page_index,
            bbox: highlight_region.clone(),
            label: Some("ocr_issue".to_string()),
            note: None,
        };
        let (x, y, crop_width, crop_height, _, _) = crop_rect(&dummy_template, width, height);
        std::fs::create_dir_all(output_dir).map_err(|error| AppError {
            code: AppErrorCode::FileWriteFailed,
            message: "OCR issue crop dizini oluşturulamadı.".to_string(),
            recoverable: false,
            suggested_action: Some("Disk izinlerini kontrol edin.".to_string()),
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        let cropped = image.crop_imm(x, y, crop_width, crop_height);
        let crop_path = output_dir.join(output_name);
        cropped.save(&crop_path).map_err(|error| AppError {
            code: AppErrorCode::PdfRenderFailed,
            message: "OCR issue crop görüntüsü kaydedilemedi.".to_string(),
            recoverable: true,
            suggested_action: Some("Kaynak crop görüntüsünü kontrol edin.".to_string()),
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        Ok(StudentAnswerIssueCropArtifacts {
            model_input_image: crop_path,
            crop_created: true,
            crop_bbox: Some(highlight_region.clone()),
        })
    }

    fn build_sources(
        &self,
        project_root: &str,
        document_id: &str,
        submission: &StudentSubmission,
        question: &crate::domain::question::Question,
        preview_by_page: &BTreeMap<u32, PathBuf>,
        template_item: Option<&StudentAnswerCropTemplateItem>,
    ) -> Result<StudentAnswerCropArtifacts, AppError> {
        let full_page_preview_refs = submission
            .page_numbers
            .iter()
            .filter_map(|page_number| {
                preview_by_page
                    .get(page_number)
                    .map(|path| path.to_string_lossy().to_string())
            })
            .collect::<Vec<_>>();
        let rendered_page_preview_exists = !full_page_preview_refs.is_empty();
        let source_page_count = Some(submission.page_numbers.len() as u32);

        let mut render_diagnostics = StudentAnswerOcrRenderDiagnostics {
            crop_refs: vec![],
            full_page_preview_refs: full_page_preview_refs.clone(),
            crop_bbox: None,
            crop_width: None,
            crop_height: None,
            source_page_count,
            answer_region_source: None,
            question_region_start: submission.page_numbers.first().copied(),
            question_region_end: submission.page_numbers.last().copied(),
            next_question_anchor: question
                .number
                .checked_add(1)
                .map(|next| format!("q{next}")),
            crop_was_clamped: false,
            crop_margin_applied: false,
            rendered_crop_exists: false,
            rendered_page_preview_exists,
            crop_missing: false,
            page_preview_missing: !rendered_page_preview_exists,
            partial_answer_suspected: false,
            printed_text_mixed: false,
            printed_question_leak_detected: false,
        };

        let mut source_refs: Vec<(u32, PathBuf)> = Vec::new();
        let source_pages = submission.page_numbers.clone();
        let mut layout_hint = "full page fallback".to_string();

        if let Some(crop_template) = template_item {
            if let Some(page_number) = submission
                .page_numbers
                .get(crop_template.page_index_within_submission as usize)
                .copied()
            {
                if let Some(preview_path) = preview_by_page.get(&page_number).cloned() {
                    let crop_render = self.crop_preview_image(
                        project_root,
                        document_id,
                        submission.id.as_str(),
                        question.number,
                        page_number,
                        &preview_path,
                        crop_template,
                    )?;
                    render_diagnostics
                        .crop_refs
                        .push(crop_render.path.to_string_lossy().to_string());
                    render_diagnostics.crop_bbox = Some(crop_render.bbox);
                    render_diagnostics.crop_width = Some(crop_render.crop_width);
                    render_diagnostics.crop_height = Some(crop_render.crop_height);
                    render_diagnostics.rendered_crop_exists = true;
                    render_diagnostics.crop_was_clamped = crop_render.crop_was_clamped;
                    render_diagnostics.crop_margin_applied = crop_render.crop_margin_applied;
                    render_diagnostics.answer_region_source = Some("manual_template".to_string());
                    layout_hint = format!(
                        "crop page {} on submission page {}",
                        question.number, page_number
                    );
                    source_refs.push((page_number, crop_render.path));
                    if submission.page_numbers.len() > 1 || crop_render.crop_was_clamped {
                        render_diagnostics.partial_answer_suspected = true;
                    }
                } else {
                    render_diagnostics.crop_missing = true;
                    render_diagnostics.answer_region_source = Some("crop_missing".to_string());
                    source_refs.extend(submission.page_numbers.iter().filter_map(|page_number| {
                        preview_by_page
                            .get(page_number)
                            .cloned()
                            .map(|path| (*page_number, path))
                    }));
                }
            } else {
                render_diagnostics.crop_missing = true;
                render_diagnostics.answer_region_source = Some("crop_page_missing".to_string());
                source_refs.extend(submission.page_numbers.iter().filter_map(|page_number| {
                    preview_by_page
                        .get(page_number)
                        .cloned()
                        .map(|path| (*page_number, path))
                }));
            }
        } else {
            source_refs.extend(submission.page_numbers.iter().filter_map(|page_number| {
                preview_by_page
                    .get(page_number)
                    .cloned()
                    .map(|path| (*page_number, path))
            }));
            render_diagnostics.answer_region_source = Some("full_page_fallback".to_string());
            render_diagnostics.partial_answer_suspected = true;
        }

        if source_refs.is_empty() {
            return Err(AppError {
                code: AppErrorCode::PdfRenderFailed,
                message: "Öğrenci cevap sayfası bulunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Önizleme cache'ini yeniden oluşturun.".to_string()),
                technical_details: Some(format!("submission_id={}", submission.id)),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        render_diagnostics.rendered_page_preview_exists =
            !render_diagnostics.full_page_preview_refs.is_empty();
        render_diagnostics.page_preview_missing = !render_diagnostics.rendered_page_preview_exists;
        if render_diagnostics.crop_missing || render_diagnostics.partial_answer_suspected {
            layout_hint = format!("{layout_hint} (review_needed)");
        }
        if render_diagnostics.answer_region_source.as_deref() == Some("manual_template") {
            // Manual template crops are the authoritative OCR input for this question.
        } else if render_diagnostics.crop_missing {
            render_diagnostics.answer_region_source = Some("crop_missing".to_string());
        } else if render_diagnostics.partial_answer_suspected {
            render_diagnostics.answer_region_source =
                Some("full_page_fallback_review_required".to_string());
        }

        Ok(StudentAnswerCropArtifacts {
            model_input_images: source_refs,
            source_page_numbers: source_pages,
            render_diagnostics,
            layout_hint,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn crop_preview_image(
        &self,
        project_root: &str,
        document_id: &str,
        submission_id: &str,
        question_number: u32,
        page_number: u32,
        preview_path: &Path,
        crop_template: &StudentAnswerCropTemplateItem,
    ) -> Result<CropRenderInfo, AppError> {
        let image = image::open(preview_path).map_err(|error| AppError {
            code: AppErrorCode::PdfRenderFailed,
            message: "Önizleme görüntüsü açılamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("Önizleme cache'ini yeniden oluşturun.".to_string()),
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        let (width, height) = image.dimensions();
        let (x, y, crop_width, crop_height, crop_was_clamped, crop_margin_applied) =
            crop_rect(crop_template, width, height);
        let cropped = image.crop_imm(x, y, crop_width, crop_height);
        let crop_dir = Path::new(project_root)
            .join("crops")
            .join("student_answer_ocr")
            .join(document_id)
            .join(submission_id);
        std::fs::create_dir_all(&crop_dir).map_err(|error| AppError {
            code: AppErrorCode::FileWriteFailed,
            message: "Crop dizini oluşturulamadı.".to_string(),
            recoverable: false,
            suggested_action: Some("Disk izinlerini kontrol edin.".to_string()),
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        let crop_path = crop_dir.join(format!("q{question_number}_p{page_number}.png"));
        cropped.save(&crop_path).map_err(|error| AppError {
            code: AppErrorCode::PdfRenderFailed,
            message: "Crop görüntüsü kaydedilemedi.".to_string(),
            recoverable: true,
            suggested_action: Some("Önizleme cache'ini yeniden oluşturun.".to_string()),
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        let normalized_bbox = StudentAnswerOcrCropBBox {
            x: x as f32 / width.max(1) as f32,
            y: y as f32 / height.max(1) as f32,
            width: crop_width as f32 / width.max(1) as f32,
            height: crop_height as f32 / height.max(1) as f32,
            page_index: crop_template.page_index_within_submission,
        };
        Ok(CropRenderInfo {
            path: crop_path,
            bbox: normalized_bbox,
            crop_width,
            crop_height,
            crop_was_clamped,
            crop_margin_applied,
        })
    }

    fn crop_identity_preview_image(
        &self,
        project_root: &str,
        document_id: &str,
        submission_id: &str,
        page_number: u32,
        preview_path: &Path,
        crop_template: &StudentIdentityCropTemplate,
    ) -> Result<CropRenderInfo, AppError> {
        let item = StudentAnswerCropTemplateItem {
            question_id: "identity_header".to_string(),
            question_number: 0,
            page_index_within_submission: crop_template.page_index_within_submission,
            bbox: crop_template.bbox.clone(),
            label: Some(crop_template.label.clone()),
            note: crop_template.note.clone(),
        };
        let image = image::open(preview_path).map_err(|error| AppError {
            code: AppErrorCode::PdfRenderFailed,
            message: "Kimlik önizleme görüntüsü açılamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("Önizleme cache'ini yeniden oluşturun.".to_string()),
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        let (width, height) = image.dimensions();
        let (x, y, crop_width, crop_height, crop_was_clamped, crop_margin_applied) =
            crop_rect(&item, width, height);
        let cropped = image.crop_imm(x, y, crop_width, crop_height);
        let crop_dir = Path::new(project_root)
            .join("crops")
            .join("student_identity_ocr")
            .join(document_id)
            .join(submission_id);
        std::fs::create_dir_all(&crop_dir).map_err(|error| AppError {
            code: AppErrorCode::FileWriteFailed,
            message: "Kimlik crop dizini oluşturulamadı.".to_string(),
            recoverable: false,
            suggested_action: Some("Disk izinlerini kontrol edin.".to_string()),
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        let crop_path = crop_dir.join(format!("identity_p{page_number}.png"));
        cropped.save(&crop_path).map_err(|error| AppError {
            code: AppErrorCode::PdfRenderFailed,
            message: "Kimlik crop görüntüsü kaydedilemedi.".to_string(),
            recoverable: true,
            suggested_action: Some("Önizleme cache'ini yeniden oluşturun.".to_string()),
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        Ok(CropRenderInfo {
            path: crop_path,
            bbox: item.bbox,
            crop_width,
            crop_height,
            crop_was_clamped,
            crop_margin_applied,
        })
    }
}

struct CropRenderInfo {
    path: PathBuf,
    bbox: StudentAnswerOcrCropBBox,
    crop_width: u32,
    crop_height: u32,
    crop_was_clamped: bool,
    crop_margin_applied: bool,
}

pub(crate) fn crop_rect(
    crop_template: &StudentAnswerCropTemplateItem,
    width: u32,
    height: u32,
) -> (u32, u32, u32, u32, bool, bool) {
    let bbox = &crop_template.bbox;
    let normalized = bbox.x <= 1.0 && bbox.y <= 1.0 && bbox.width <= 1.0 && bbox.height <= 1.0;
    let margin_applied = true;
    let margin = if normalized { 0.04 } else { 12.0 };
    let mut x = if normalized {
        (bbox.x * width as f32).round() as u32
    } else {
        bbox.x.round() as u32
    };
    let mut y = if normalized {
        (bbox.y * height as f32).round() as u32
    } else {
        bbox.y.round() as u32
    };
    let mut w = if normalized {
        (bbox.width * width as f32).round() as u32
    } else {
        bbox.width.round() as u32
    };
    let mut h = if normalized {
        (bbox.height * height as f32).round() as u32
    } else {
        bbox.height.round() as u32
    };
    if normalized {
        let margin_px_x = (margin * width as f32).round() as i32;
        let margin_px_y = (margin * height as f32).round() as i32;
        let x_i = x as i32 - margin_px_x;
        let y_i = y as i32 - margin_px_y;
        let w_i = w as i32 + margin_px_x * 2;
        let h_i = h as i32 + margin_px_y * 2;
        x = x_i.max(0) as u32;
        y = y_i.max(0) as u32;
        w = w_i.max(1) as u32;
        h = h_i.max(1) as u32;
    } else {
        let margin_px = margin.round() as u32;
        x = x.saturating_sub(margin_px);
        y = y.saturating_sub(margin_px);
        w = w.saturating_add(margin_px.saturating_mul(2));
        h = h.saturating_add(margin_px.saturating_mul(2));
    }
    let mut crop_was_clamped = false;
    if x >= width {
        x = width.saturating_sub(1);
        crop_was_clamped = true;
    }
    if y >= height {
        y = height.saturating_sub(1);
        crop_was_clamped = true;
    }
    if x + w > width {
        w = width.saturating_sub(x);
        crop_was_clamped = true;
    }
    if y + h > height {
        h = height.saturating_sub(y);
        crop_was_clamped = true;
    }
    (x, y, w.max(1), h.max(1), crop_was_clamped, margin_applied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::student::{StudentAnswerOcrRecord, StudentAnswerOcrStatus};
    use crate::domain::workflow::{WorkflowSnapshot, WorkflowStage};
    use crate::jobs::job_manager::JobManager;
    use crate::services::pdf_service::SystemPdfService;
    use uuid::Uuid;

    fn temp_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("rubrika-crop-template-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn crop_item(question_id: &str, question_number: u32, x: f32) -> StudentAnswerCropTemplateItem {
        StudentAnswerCropTemplateItem {
            question_id: question_id.to_string(),
            question_number,
            page_index_within_submission: 0,
            bbox: StudentAnswerOcrCropBBox {
                x,
                y: 0.1,
                width: 0.2,
                height: 0.2,
                page_index: 0,
            },
            label: None,
            note: None,
        }
    }

    fn ocr_record() -> StudentAnswerOcrRecord {
        StudentAnswerOcrRecord {
            id: Uuid::new_v4().to_string(),
            submission_id: "submission-1".to_string(),
            question_id: "q1".to_string(),
            question_number: 1,
            source_page_numbers: vec![1],
            source_image_refs: vec!["old.png".to_string()],
            crop_refs: vec!["old-crop.png".to_string()],
            full_page_preview_refs: vec!["old-page.png".to_string()],
            answer_text: "old".to_string(),
            structured_answer: None,
            confidence: Some(0.8),
            status: StudentAnswerOcrStatus::TeacherApproved,
            needs_review: false,
            review_reasons: vec![],
            warnings: vec![],
            model_name: Some("gemma".to_string()),
            prompt_version: "student_answer_ocr_v1".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            teacher_corrected_text: None,
            teacher_reviewed_at: Some(chrono::Utc::now()),
            parse_diagnostics: None,
            render_diagnostics: None,
            ..Default::default()
        }
    }

    fn service(project_store: ProjectStore) -> StudentAnswerCropService {
        StudentAnswerCropService::new(
            project_store.clone(),
            Arc::new(PdfPreviewService::new(
                project_store,
                Arc::new(SystemPdfService),
                Arc::new(JobManager::new()),
            )),
        )
    }

    #[test]
    fn save_template_invalidates_existing_ocr_when_crop_changes() {
        let root = temp_root();
        let project_store = ProjectStore::new();
        let mut project = project_store
            .create_project("Crop".to_string(), root.to_string_lossy().to_string())
            .unwrap();
        project.student_answer_crop_template.items = vec![crop_item("q1", 1, 0.1)];
        project.student_answer_ocr_records = vec![ocr_record()];
        project.workflow = WorkflowSnapshot {
            current_stage: WorkflowStage::StudentAnswerOcrReadyForScoring,
            current_stage_label: "stale".to_string(),
            blocking_reasons: vec![],
            next_actions: vec![],
            summary: Default::default(),
        };
        project_store.save_project(&project).unwrap();

        let updated = service(project_store)
            .save_template(&project.id, vec![crop_item("q1", 1, 0.3)])
            .unwrap();

        assert!(updated.student_answer_ocr_records.is_empty());
        assert_ne!(
            updated.workflow.current_stage,
            WorkflowStage::StudentAnswerOcrReadyForScoring
        );
        let _ = std::fs::remove_dir_all(root);
    }
}

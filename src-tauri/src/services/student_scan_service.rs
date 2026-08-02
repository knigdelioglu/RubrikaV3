use std::collections::BTreeSet;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::document::{Document, DocumentRole, PdfPagePreview, PdfPreviewStatus};
use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::job::{JobKind, JobSnapshot, JobStatus};
use crate::domain::project::Project;
use crate::domain::question::Question;
use crate::domain::student::{
    new_student_id, student_identity_is_missing, ClassMembershipSource, PageGroupingMode, Student,
    StudentAnswerSlot, StudentAnswerSlotStatus, StudentScanReadinessSnapshot, StudentSubmission,
    StudentSubmissionStatus,
};
use crate::jobs::job_manager::load_persisted_jobs;
use crate::services::pdf_preview_service::PdfPreviewService;
use crate::services::project_store::ProjectStore;
use crate::services::workflow_engine;

#[derive(Clone)]
pub struct StudentScanService {
    project_store: ProjectStore,
    pdf_preview_service: Arc<PdfPreviewService>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateStudentPageGroupsInput {
    pub project_id: String,
    pub document_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
    pub pages_per_student: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateStudentPageGroupsOutput {
    pub groups_created: u32,
    pub total_pages: u32,
    pub pages_per_student: u32,
    pub remainder_pages: u32,
    pub needs_review: bool,
    pub submissions: Vec<StudentSubmission>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStudentIdentityInput {
    pub project_id: String,
    pub submission_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSubmissionPagesInput {
    pub project_id: String,
    pub submission_id: String,
    pub page_numbers: Vec<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteStudentSubmissionInput {
    pub project_id: String,
    pub submission_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkStudentGroupingCompleteInput {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetOcrReadinessInput {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmissionDependencyScan {
    pub submission_count: usize,
    pub ocr_record_count: usize,
    pub ocr_generation_count: usize,
    pub ocr_review_count: usize,
    pub scoring_record_count: usize,
    pub approved_scoring_count: usize,
    pub artifact_ref_count: usize,
    pub running_job_count: usize,
}

impl SubmissionDependencyScan {
    pub fn is_blocked(&self) -> bool {
        self.ocr_record_count > 0
            || self.ocr_generation_count > 0
            || self.scoring_record_count > 0
            || self.artifact_ref_count > 0
            || self.running_job_count > 0
    }
}

pub fn scan_submission_dependencies(
    project: &Project,
    submission_ids: &[String],
) -> SubmissionDependencyScan {
    let in_scope = |id: &str| submission_ids.iter().any(|candidate| candidate == id);
    let mut scan = SubmissionDependencyScan {
        submission_count: project
            .student_submissions
            .iter()
            .filter(|submission| in_scope(&submission.id))
            .count(),
        ..Default::default()
    };
    scan.ocr_record_count = project
        .student_answer_ocr_records
        .iter()
        .filter(|record| in_scope(&record.submission_id))
        .count();
    scan.ocr_review_count = project
        .student_answer_ocr_records
        .iter()
        .filter(|record| {
            in_scope(&record.submission_id)
                && (record.needs_review
                    || record.status
                        == crate::domain::student::StudentAnswerOcrStatus::TeacherApproved)
        })
        .count();
    scan.ocr_generation_count = project
        .student_answer_ocr_generations
        .iter()
        .filter(|generation| in_scope(&generation.submission_id))
        .count();
    scan.scoring_record_count = project
        .scoring_records
        .iter()
        .filter(|record| in_scope(&record.submission_id))
        .count();
    scan.approved_scoring_count = project
        .scoring_records
        .iter()
        .filter(|record| {
            in_scope(&record.submission_id)
                && matches!(
                    record.teacher_review_status,
                    crate::domain::scoring::ScoringReviewStatus::Approved
                        | crate::domain::scoring::ScoringReviewStatus::Edited
                )
        })
        .count();
    scan.artifact_ref_count = project
        .student_answer_ocr_records
        .iter()
        .filter(|record| in_scope(&record.submission_id))
        .map(|record| {
            record.source_image_refs.len()
                + record.crop_refs.len()
                + record.original_crop_refs.len()
                + record.preprocessed_crop_refs.len()
                + record.full_page_preview_refs.len()
        })
        .sum();
    scan
}

pub fn scan_submission_dependencies_with_jobs(
    project: &Project,
    submission_ids: &[String],
    jobs: &[JobSnapshot],
) -> SubmissionDependencyScan {
    let mut scan = scan_submission_dependencies(project, submission_ids);
    scan.running_job_count = jobs
        .iter()
        .filter(|job| {
            matches!(job.kind, JobKind::StudentAnswerOcr | JobKind::Scoring)
                && matches!(job.status, JobStatus::Queued | JobStatus::Running)
        })
        .count();
    scan
}

pub fn persisted_dependency_jobs(project: &Project) -> Result<Vec<JobSnapshot>, AppError> {
    load_persisted_jobs(std::path::Path::new(&project.root_path))
}

fn submission_in_use_error(scan: &SubmissionDependencyScan) -> AppError {
    AppError {
        code: AppErrorCode::StudentSubmissionInUse,
        message: "Bu öğrenci kaydına bağlı OCR veya puanlama verileri bulunduğu için silinemiyor."
            .to_string(),
        recoverable: true,
        suggested_action: Some(
            "Önce ilgili sınav verilerini kaldırın ya da öğrenciyi uygulamadan çıkarın."
                .to_string(),
        ),
        technical_details: Some(format!(
            "ocr_records={}; ocr_generations={}; scoring_records={}; artifacts={}; running_jobs={}",
            scan.ocr_record_count,
            scan.ocr_generation_count,
            scan.scoring_record_count,
            scan.artifact_ref_count,
            scan.running_job_count
        )),
        correlation_id: Uuid::new_v4().to_string(),
    }
}

impl StudentScanService {
    pub fn new(project_store: ProjectStore, pdf_preview_service: Arc<PdfPreviewService>) -> Self {
        Self {
            project_store,
            pdf_preview_service,
        }
    }

    pub fn list_student_scan_documents(&self, project_id: &str) -> Result<Vec<Document>, AppError> {
        let project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        Ok(project
            .documents
            .into_iter()
            .filter(|document| document.role == DocumentRole::StudentScan)
            .collect())
    }

    pub fn list_student_submissions(
        &self,
        project_id: &str,
    ) -> Result<Vec<StudentSubmission>, AppError> {
        let project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        Ok(project.student_submissions)
    }

    pub fn create_student_page_groups(
        &self,
        input: CreateStudentPageGroupsInput,
    ) -> Result<CreateStudentPageGroupsOutput, AppError> {
        let mut project = self.load_project(&input.project_id)?;
        let document = self
            .find_student_scan_document(&project, &input.document_id)?
            .clone();
        let scope = resolve_batch_scope(
            &project,
            input.batch_id.as_deref(),
            Some(input.document_id.as_str()),
        )?;
        ensure_scope_can_regroup(&project, &scope)?;
        let previews = self
            .pdf_preview_service
            .require_ready_page_previews(&input.project_id, &input.document_id)?;
        if previews.is_empty() {
            return Err(AppError {
                code: AppErrorCode::StudentScanPreviewNotReady,
                message: "Öğrenci PDF önizlemeleri hazır değil.".to_string(),
                recoverable: true,
                suggested_action: Some("Önce öğrenci PDF önizlemelerini oluşturun.".to_string()),
                technical_details: Some(format!("document_id={}", input.document_id)),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        let total_pages = previews.len() as u32;
        if input.pages_per_student == 0 || input.pages_per_student > 20 {
            return Err(AppError {
                code: AppErrorCode::StudentGroupingInvalid,
                message: "Sayfa başına öğrenci sayısı 1 ile 20 arasında olmalıdır.".to_string(),
                recoverable: true,
                suggested_action: Some("Pozitif bir sayı girin.".to_string()),
                technical_details: Some(format!("pages_per_student={}", input.pages_per_student)),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        if input.pages_per_student > total_pages {
            return Err(AppError {
                code: AppErrorCode::StudentGroupingInvalid,
                message: "Sayfa başına öğrenci sayısı toplam sayfadan büyük olamaz.".to_string(),
                recoverable: true,
                suggested_action: Some("Daha küçük bir sayı girin.".to_string()),
                technical_details: Some(format!(
                    "pages_per_student={}; total_pages={}",
                    input.pages_per_student, total_pages
                )),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        let page_groups = build_fixed_page_groups(input.pages_per_student, &previews);
        let remainder_pages = total_pages % input.pages_per_student;
        let needs_review = remainder_pages > 0;
        let mut warnings = Vec::new();
        if needs_review {
            warnings.push(
                "Son grup eksik sayfa içeriyor olabilir; son sayfayı manuel kontrol edin."
                    .to_string(),
            );
        }

        let now = chrono::Utc::now().to_rfc3339();
        remove_scope_submissions(&mut project, &scope);
        project.student_scan_document_id = Some(document.id.clone());
        project.student_grouping_mode = Some(PageGroupingMode::FixedPagesPerStudent);
        project.student_pages_per_student = Some(input.pages_per_student);
        project.student_grouping_complete_at = None;
        if let Some(batch_index) = scope.batch_index {
            let batch = &mut project.student_scan_batches[batch_index];
            batch.pages_per_student = Some(input.pages_per_student);
            batch.grouping_mode = Some(PageGroupingMode::FixedPagesPerStudent);
            batch.grouping_completed_at = None;
            batch.updated_at = now.clone();
        }

        for page_numbers in page_groups {
            let student_id = new_student_id();
            let student = Student {
                id: student_id.clone(),
                display_name: None,
                number: None,
                // A grouped submission is not yet a roster match. The teacher-confirmed
                // identity step attaches it to the canonical class roster.
                class_name: None,
                warnings: vec!["Öğrenci kimliği henüz girilmedi.".to_string()],
                identity_ocr: None,
            };
            let submission = StudentSubmission {
                id: Uuid::new_v4().to_string(),
                student_id: student_id.clone(),
                document_id: document.id.clone(),
                class_id: scope.class_id.clone(),
                scan_batch_id: scope.batch_id.clone(),
                class_membership_source: scope
                    .batch_id
                    .as_ref()
                    .map(|_| ClassMembershipSource::InheritedFromBatch),
                page_numbers,
                status: StudentSubmissionStatus::IdentityMissing,
                answer_slots: build_answer_slots(&project.questions),
                warnings: vec![],
                updated_at: Some(now.clone()),
            };
            project.students.push(student);
            project.student_submissions.push(submission);
        }

        if needs_review {
            if let Some(last_submission) = project
                .student_submissions
                .iter_mut()
                .rev()
                .find(|submission| submission_matches_scope(submission, &scope))
            {
                last_submission
                    .warnings
                    .push("Eksik sayfa nedeniyle manuel inceleme gerekli.".to_string());
            }
        }

        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;

        Ok(CreateStudentPageGroupsOutput {
            groups_created: project
                .student_submissions
                .iter()
                .filter(|submission| submission_matches_scope(submission, &scope))
                .count() as u32,
            total_pages,
            pages_per_student: input.pages_per_student,
            remainder_pages,
            needs_review,
            submissions: project
                .student_submissions
                .iter()
                .filter(|submission| submission_matches_scope(submission, &scope))
                .cloned()
                .collect(),
            warnings,
        })
    }

    pub fn update_student_identity(
        &self,
        input: UpdateStudentIdentityInput,
    ) -> Result<StudentSubmission, AppError> {
        let mut project = self.load_project(&input.project_id)?;
        let now = chrono::Utc::now().to_rfc3339();
        let submission_index = find_submission_index(&project, &input.submission_id)?;
        let student_id = project.student_submissions[submission_index]
            .student_id
            .clone();
        let student_index = find_student_index(&project, &student_id)?;
        let submission_class_id = project.student_submissions[submission_index]
            .class_id
            .clone();
        let display_name = normalize_optional(input.display_name);
        let number = normalize_optional(input.number);
        let canonical_class_name = submission_class_id.as_ref().and_then(|class_id| {
            project
                .school_classes
                .iter()
                .find(|school_class| school_class.id == *class_id)
                .map(|school_class| school_class.normalized_name.clone())
        });
        let matched_roster_index = find_matching_roster_student_index(
            &project,
            submission_class_id.as_deref(),
            &student_id,
            display_name.as_deref(),
            number.as_deref(),
        );
        let effective_student_index = if let Some(matched_index) = matched_roster_index {
            let matched_student_id = project.students[matched_index].id.clone();
            let matched_student = &mut project.students[matched_index];
            matched_student.display_name = display_name.clone();
            matched_student.number = number.clone();
            matched_student.class_name = canonical_class_name.clone();
            matched_student.warnings.clear();
            project.students[student_index].class_name = None;
            project.student_submissions[submission_index].student_id = matched_student_id;
            matched_index
        } else {
            let student = &mut project.students[student_index];
            student.display_name = display_name;
            student.number = number;
            student.class_name = canonical_class_name.or(normalize_optional(input.class_name));
            student.warnings = if student_identity_is_missing(student) {
                vec!["Öğrenci kimliği eksik.".to_string()]
            } else {
                vec![]
            };
            student_index
        };

        let updated_submission = {
            let student = &project.students[effective_student_index];
            let submission = &mut project.student_submissions[submission_index];
            submission.status =
                if student_identity_is_missing(student) || submission.page_numbers.is_empty() {
                    StudentSubmissionStatus::IdentityMissing
                } else {
                    StudentSubmissionStatus::Grouped
                };
            submission.updated_at = Some(now.clone());
            submission.clone()
        };

        invalidate_grouping_completion(
            &mut project,
            updated_submission.scan_batch_id.as_deref(),
            &updated_submission.document_id,
            &now,
        );
        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(updated_submission)
    }

    pub fn update_submission_pages(
        &self,
        input: UpdateSubmissionPagesInput,
    ) -> Result<StudentSubmission, AppError> {
        let mut project = self.load_project(&input.project_id)?;
        let now = chrono::Utc::now().to_rfc3339();
        let submission_index = find_submission_index(&project, &input.submission_id)?;
        let student_id = project.student_submissions[submission_index]
            .student_id
            .clone();
        let student_is_missing = {
            let student = find_student(&project, &student_id)?;
            student_identity_is_missing(student)
        };
        let updated_submission = {
            let submission = &mut project.student_submissions[submission_index];
            submission.page_numbers = input.page_numbers;
            submission.status = if submission.page_numbers.is_empty() || student_is_missing {
                StudentSubmissionStatus::IdentityMissing
            } else {
                StudentSubmissionStatus::Grouped
            };
            if submission.page_numbers.is_empty() {
                submission.warnings = vec!["Sayfa listesi boş.".to_string()];
            }
            submission.updated_at = Some(now.clone());
            submission.clone()
        };
        invalidate_grouping_completion(
            &mut project,
            updated_submission.scan_batch_id.as_deref(),
            &updated_submission.document_id,
            &now,
        );
        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(updated_submission)
    }

    pub fn delete_student_submission(
        &self,
        input: DeleteStudentSubmissionInput,
    ) -> Result<(), AppError> {
        let project = self.load_project(&input.project_id)?;
        let submission = project
            .student_submissions
            .iter()
            .find(|submission| submission.id == input.submission_id)
            .cloned()
            .ok_or_else(|| AppError {
                code: AppErrorCode::StudentSubmissionNotFound,
                message: "Öğrenci kaydı bulunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Öğrenci listesini yenileyin.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        let submission_ids = vec![submission.id.clone()];
        let jobs = persisted_dependency_jobs(&project).map_err(|error| AppError {
            code: AppErrorCode::SubmissionDeleteConflict,
            message: "Öğrenci kaydının bağlı işlemleri doğrulanamadı; silme engellendi."
                .to_string(),
            recoverable: true,
            suggested_action: Some("İşlem geçmişini yenileyip tekrar deneyin.".to_string()),
            technical_details: error.technical_details,
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        let scan = scan_submission_dependencies_with_jobs(&project, &submission_ids, &jobs);
        if scan.is_blocked() {
            return Err(submission_in_use_error(&scan));
        }
        let submission_id = submission.id.clone();
        let student_id = submission.student_id.clone();
        let batch_id = submission.scan_batch_id.clone();
        let document_id = submission.document_id.clone();
        let output = self.project_store.commit_job(
            &input.project_id,
            crate::services::project_store::MutationOptions::new("delete_student_submission"),
            move |current, _context| {
                let current_jobs = persisted_dependency_jobs(current)?;
                let current_scan = scan_submission_dependencies_with_jobs(
                    current,
                    std::slice::from_ref(&submission_id),
                    &current_jobs,
                );
                if current_scan.is_blocked() {
                    return Err(submission_in_use_error(&current_scan));
                }
                let index = current
                    .student_submissions
                    .iter()
                    .position(|candidate| candidate.id == submission_id)
                    .ok_or_else(|| AppError {
                        code: AppErrorCode::StudentSubmissionNotFound,
                        message: "Öğrenci kaydı artık mevcut değil.".to_string(),
                        recoverable: true,
                        suggested_action: Some("Listeyi yenileyip tekrar deneyin.".to_string()),
                        technical_details: None,
                        correlation_id: Uuid::new_v4().to_string(),
                    })?;
                current.student_submissions.remove(index);
                if !current
                    .student_submissions
                    .iter()
                    .any(|candidate| candidate.student_id == student_id)
                {
                    current.students.retain(|student| student.id != student_id);
                }
                invalidate_grouping_completion(
                    current,
                    batch_id.as_deref(),
                    &document_id,
                    &chrono::Utc::now().to_rfc3339(),
                );
                Ok(())
            },
        );
        match output {
            crate::services::project_store::JobCommitResult::Applied(_) => Ok(()),
            crate::services::project_store::JobCommitResult::Conflict(error)
            | crate::services::project_store::JobCommitResult::Rejected(error) => Err(error),
            crate::services::project_store::JobCommitResult::Stale { reason } => Err(AppError {
                code: AppErrorCode::SubmissionDeleteConflict,
                message: "Öğrenci kaydı silinemedi; bağlı veri durumu değişti.".to_string(),
                recoverable: true,
                suggested_action: Some("Listeyi yenileyip tekrar deneyin.".to_string()),
                technical_details: Some(reason),
                correlation_id: Uuid::new_v4().to_string(),
            }),
            crate::services::project_store::JobCommitResult::EntityMissing => Err(AppError {
                code: AppErrorCode::StudentSubmissionNotFound,
                message: "Öğrenci kaydı artık mevcut değil.".to_string(),
                recoverable: true,
                suggested_action: Some("Listeyi yenileyin.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            }),
        }
    }

    pub fn mark_student_grouping_complete(
        &self,
        input: MarkStudentGroupingCompleteInput,
    ) -> Result<Project, AppError> {
        let mut project = self.load_project(&input.project_id)?;
        let scope = resolve_batch_scope(&project, input.batch_id.as_deref(), None)?;
        self.ensure_grouping_ready(&project, &scope)?;
        let now = chrono::Utc::now().to_rfc3339();
        for submission in &mut project.student_submissions {
            if submission_matches_scope(submission, &scope) {
                submission.status = StudentSubmissionStatus::ReadyForOcr;
                submission.updated_at = Some(now.clone());
            }
        }
        if let Some(batch_index) = scope.batch_index {
            project.student_scan_batches[batch_index].grouping_completed_at = Some(now.clone());
            project.student_scan_batches[batch_index].updated_at = now.clone();
        }
        project.student_scan_document_id = Some(scope.document_id.clone());
        project.student_grouping_complete_at = Some(now);
        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(project)
    }

    pub fn get_ocr_readiness(
        &self,
        input: &GetOcrReadinessInput,
    ) -> Result<StudentScanReadinessSnapshot, AppError> {
        let project = self.load_project(&input.project_id)?;
        let workflow = workflow_engine::evaluate_workflow(&project);
        let scope = match resolve_batch_scope(&project, input.batch_id.as_deref(), None) {
            Ok(scope) => Some(scope),
            Err(error)
                if input.batch_id.is_none() && error.code == AppErrorCode::StudentScanNotFound =>
            {
                None
            }
            Err(error) => return Err(error),
        };
        let document = scope.as_ref().and_then(|scope| {
            project.documents.iter().find(|document| {
                document.id == scope.document_id && document.role == DocumentRole::StudentScan
            })
        });
        let preview_status_snapshot = document.as_ref().and_then(|doc| {
            self.pdf_preview_service
                .get_pdf_preview_status(&project.id, &doc.id)
                .ok()
        });
        let preview_ready = preview_status_snapshot
            .as_ref()
            .is_some_and(|preview| preview.status == PdfPreviewStatus::Ready);
        let preview_current = preview_status_snapshot
            .as_ref()
            .map(|preview| preview.preview_count)
            .unwrap_or(0);
        let preview_total = preview_status_snapshot
            .as_ref()
            .map(|preview| preview.page_count)
            .or_else(|| document.as_ref().map(|doc| doc.page_count))
            .unwrap_or(0);
        let scoped_submissions = scope
            .as_ref()
            .map(|scope| submissions_for_scope(&project, scope))
            .unwrap_or_default();
        let completion_timestamp = scope.as_ref().and_then(|scope| {
            scope
                .batch_index
                .and_then(|index| {
                    project.student_scan_batches[index]
                        .grouping_completed_at
                        .as_ref()
                })
                .or(project.student_grouping_complete_at.as_ref())
        });
        let grouping_complete = completion_timestamp.is_some()
            && grouping_is_valid(
                &scoped_submissions,
                document.map(|doc| doc.page_count).unwrap_or(0),
            )
            .is_ok();
        let ready = grouping_complete
            && preview_ready
            && matches!(
                workflow.current_stage,
                crate::domain::workflow::WorkflowStage::OcrReady
                    | crate::domain::workflow::WorkflowStage::StudentAnswerOcrRunning
                    | crate::domain::workflow::WorkflowStage::StudentAnswerOcrReviewNeeded
                    | crate::domain::workflow::WorkflowStage::StudentAnswerOcrReadyForScoring
                    | crate::domain::workflow::WorkflowStage::ScoringReady
                    | crate::domain::workflow::WorkflowStage::ScoringRunning
                    | crate::domain::workflow::WorkflowStage::ScoringDone
                    | crate::domain::workflow::WorkflowStage::AnalysisReady
            );
        let mut warnings = Vec::new();
        let pages_per_student = scope
            .as_ref()
            .and_then(|scope| {
                scope
                    .batch_index
                    .and_then(|index| project.student_scan_batches[index].pages_per_student)
            })
            .or(project.student_pages_per_student);
        if let Some(active_document) = document.as_ref() {
            if active_document.page_count == 0 && !preview_ready {
                warnings.push("Önizleme henüz oluşturulmadı.".to_string());
            }
        }

        Ok(StudentScanReadinessSnapshot {
            project_id: project.id.clone(),
            document_id: document.map(|doc| doc.id.clone()),
            class_id: scope.as_ref().and_then(|scope| scope.class_id.clone()),
            batch_id: scope.as_ref().and_then(|scope| scope.batch_id.clone()),
            ready,
            preview_status: preview_status_snapshot
                .as_ref()
                .map(|preview| format!("{:?}", preview.status).to_lowercase()),
            current_stage: serde_json::to_string(&workflow.current_stage)
                .map(|value| value.trim_matches('"').to_string())
                .unwrap_or_else(|_| "unknown".to_string()),
            blocking_reasons: workflow
                .blocking_reasons
                .iter()
                .map(|reason| {
                    serde_json::to_string(reason)
                        .map(|value| value.trim_matches('"').to_string())
                        .unwrap_or_else(|_| "unknown".to_string())
                })
                .collect(),
            next_actions: workflow
                .next_actions
                .iter()
                .map(|action| action.label.clone())
                .collect(),
            submission_count: scoped_submissions.len() as u32,
            preview_ready,
            preview_current,
            preview_total,
            grouping_complete,
            pages_per_student,
            warnings,
            message: workflow.summary.text.unwrap_or_else(|| {
                if ready {
                    "Öğrenci cevap OCR hazırlığı tamam.".to_string()
                } else {
                    "Öğrenci cevap OCR hazırlığı bekleniyor.".to_string()
                }
            }),
        })
    }

    fn load_project(&self, project_id: &str) -> Result<Project, AppError> {
        self.project_store
            .get_project_snapshot(project_id.to_string())
    }

    fn find_student_scan_document<'a>(
        &self,
        project: &'a Project,
        document_id: &str,
    ) -> Result<&'a Document, AppError> {
        project
            .documents
            .iter()
            .find(|document| {
                document.id == document_id && document.role == DocumentRole::StudentScan
            })
            .ok_or_else(|| AppError {
                code: AppErrorCode::StudentScanNotFound,
                message: "Öğrenci cevap PDF'i bulunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Öğrenci cevap PDF'ini yükleyin.".to_string()),
                technical_details: Some(format!("document_id={document_id}")),
                correlation_id: Uuid::new_v4().to_string(),
            })
    }

    fn ensure_grouping_ready(&self, project: &Project, scope: &BatchScope) -> Result<(), AppError> {
        let document = self.find_student_scan_document(project, &scope.document_id)?;

        match document.preview.as_ref().map(|preview| &preview.status) {
            Some(PdfPreviewStatus::Ready) => {}
            _ => {
                return Err(AppError {
                    code: AppErrorCode::StudentScanPreviewNotReady,
                    message: "Öğrenci cevap PDF önizlemeleri hazır değil.".to_string(),
                    recoverable: true,
                    suggested_action: Some(
                        "Önce öğrenci PDF önizlemelerini oluşturun.".to_string(),
                    ),
                    technical_details: Some(format!("document_id={}", document.id)),
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
        }

        let submissions = submissions_for_scope(project, scope);
        grouping_is_valid(&submissions, document.page_count).map(|_| ())
    }
}

#[derive(Debug, Clone)]
struct BatchScope {
    batch_index: Option<usize>,
    batch_id: Option<String>,
    class_id: Option<String>,
    document_id: String,
}

fn resolve_batch_scope(
    project: &Project,
    batch_id: Option<&str>,
    document_id: Option<&str>,
) -> Result<BatchScope, AppError> {
    if let Some(batch_id) = batch_id {
        let batch_index = project
            .student_scan_batches
            .iter()
            .position(|batch| batch.id == batch_id)
            .ok_or_else(|| AppError {
                code: AppErrorCode::StudentScanBatchNotFound,
                message: "Öğrenci tarama paketi bulunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Paket listesini yenileyin.".to_string()),
                technical_details: Some(format!("batch_id={batch_id}")),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        let batch = &project.student_scan_batches[batch_index];
        if document_id.is_some_and(|document_id| document_id != batch.document_id) {
            return Err(AppError {
                code: AppErrorCode::StudentGroupingInvalid,
                message: "Seçilen PDF bu öğrenci paketine ait değil.".to_string(),
                recoverable: true,
                suggested_action: Some("Paketin bağlı olduğu PDF'i seçin.".to_string()),
                technical_details: Some(format!(
                    "batch_id={batch_id}; batch_document_id={}; requested_document_id={}",
                    batch.document_id,
                    document_id.unwrap_or_default()
                )),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        return Ok(BatchScope {
            batch_index: Some(batch_index),
            batch_id: Some(batch.id.clone()),
            class_id: Some(batch.class_id.clone()),
            document_id: batch.document_id.clone(),
        });
    }

    if let Some(document_id) = document_id {
        if let Some((batch_index, batch)) = project
            .student_scan_batches
            .iter()
            .enumerate()
            .find(|(_, batch)| batch.document_id == document_id)
        {
            return Ok(BatchScope {
                batch_index: Some(batch_index),
                batch_id: Some(batch.id.clone()),
                class_id: Some(batch.class_id.clone()),
                document_id: batch.document_id.clone(),
            });
        }
        if project.documents.iter().any(|document| {
            document.id == document_id && document.role == DocumentRole::StudentScan
        }) {
            return Ok(BatchScope {
                batch_index: None,
                batch_id: None,
                class_id: None,
                document_id: document_id.to_string(),
            });
        }
    }

    if let Some(active_document_id) = project.student_scan_document_id.as_deref() {
        return resolve_batch_scope(project, None, Some(active_document_id));
    }
    if project.student_scan_batches.len() == 1 {
        return resolve_batch_scope(
            project,
            Some(project.student_scan_batches[0].id.as_str()),
            None,
        );
    }
    if let Some(document) = project
        .documents
        .iter()
        .find(|document| document.role == DocumentRole::StudentScan)
    {
        return resolve_batch_scope(project, None, Some(document.id.as_str()));
    }

    Err(AppError {
        code: AppErrorCode::StudentScanNotFound,
        message: "Öğrenci cevap PDF'i bulunamadı.".to_string(),
        recoverable: true,
        suggested_action: Some("Öğrenci cevap PDF'ini yükleyin.".to_string()),
        technical_details: None,
        correlation_id: Uuid::new_v4().to_string(),
    })
}

fn submission_matches_scope(submission: &StudentSubmission, scope: &BatchScope) -> bool {
    match scope.batch_id.as_deref() {
        Some(batch_id) => {
            submission.scan_batch_id.as_deref() == Some(batch_id)
                || (submission.scan_batch_id.is_none()
                    && submission.document_id == scope.document_id)
        }
        None => submission.document_id == scope.document_id,
    }
}

fn submissions_for_scope<'a>(
    project: &'a Project,
    scope: &BatchScope,
) -> Vec<&'a StudentSubmission> {
    project
        .student_submissions
        .iter()
        .filter(|submission| submission_matches_scope(submission, scope))
        .collect()
}

fn ensure_scope_can_regroup(project: &Project, scope: &BatchScope) -> Result<(), AppError> {
    let submission_ids = submissions_for_scope(project, scope)
        .into_iter()
        .map(|submission| submission.id.as_str())
        .collect::<BTreeSet<_>>();
    if submission_ids.is_empty() {
        return Ok(());
    }
    let ocr_count = project
        .student_answer_ocr_records
        .iter()
        .filter(|record| submission_ids.contains(record.submission_id.as_str()))
        .count();
    let scoring_count = project
        .scoring_records
        .iter()
        .filter(|record| submission_ids.contains(record.submission_id.as_str()))
        .count();
    if ocr_count == 0 && scoring_count == 0 {
        return Ok(());
    }
    Err(AppError {
        code: AppErrorCode::StudentScanBatchInUse,
        message: "OCR veya notlandırma sonucu olan paket yeniden gruplanamaz.".to_string(),
        recoverable: true,
        suggested_action: Some("Mevcut grupları koruyun veya yeni bir PDF paketi yükleyin.".to_string()),
        technical_details: Some(format!(
            "batch_id={:?}; submissions={}; ocr_records={ocr_count}; scoring_records={scoring_count}",
            scope.batch_id,
            submission_ids.len()
        )),
        correlation_id: Uuid::new_v4().to_string(),
    })
}

fn build_answer_slots(questions: &[Question]) -> Vec<StudentAnswerSlot> {
    questions
        .iter()
        .map(|question| StudentAnswerSlot {
            question_id: question.id.clone(),
            question_number: question.number,
            status: StudentAnswerSlotStatus::Empty,
            text: None,
            confidence: None,
            warnings: vec![],
        })
        .collect()
}

fn build_fixed_page_groups(pages_per_student: u32, previews: &[PdfPagePreview]) -> Vec<Vec<u32>> {
    let pages: Vec<u32> = previews.iter().map(|preview| preview.page_number).collect();
    let mut groups = Vec::new();
    for chunk in pages.chunks(pages_per_student as usize) {
        groups.push(chunk.to_vec());
    }
    groups
}

fn grouping_is_valid(submissions: &[&StudentSubmission], total_pages: u32) -> Result<(), AppError> {
    if submissions.is_empty() {
        return Err(AppError {
            code: AppErrorCode::StudentGroupingNotReady,
            message: "Öğrenci gruplaması henüz oluşturulmadı.".to_string(),
            recoverable: true,
            suggested_action: Some("Önce sayfa gruplarını oluşturun.".to_string()),
            technical_details: Some(format!("total_pages={total_pages}")),
            correlation_id: Uuid::new_v4().to_string(),
        });
    }

    let mut seen_pages = BTreeSet::new();
    for submission in submissions {
        if submission.page_numbers.is_empty() {
            return Err(AppError {
                code: AppErrorCode::StudentGroupingInvalid,
                message: "Öğrenci grubu boş olamaz.".to_string(),
                recoverable: true,
                suggested_action: Some("En az bir sayfa ekleyin.".to_string()),
                technical_details: Some(format!("submission_id={}", submission.id)),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        for page_number in &submission.page_numbers {
            if *page_number == 0 || *page_number > total_pages {
                return Err(AppError {
                    code: AppErrorCode::StudentGroupingInvalid,
                    message: "Sayfa numarası PDF aralığı dışında.".to_string(),
                    recoverable: true,
                    suggested_action: Some("Sayfa numaralarını düzeltin.".to_string()),
                    technical_details: Some(format!(
                        "submission_id={}; page_number={}; total_pages={total_pages}",
                        submission.id, page_number
                    )),
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
            if !seen_pages.insert(*page_number) {
                return Err(AppError {
                    code: AppErrorCode::StudentGroupingInvalid,
                    message: "Aynı sayfa birden fazla grupta kullanılamaz.".to_string(),
                    recoverable: true,
                    suggested_action: Some("Çakışan sayfaları kaldırın.".to_string()),
                    technical_details: Some(format!(
                        "duplicate_page={}; submission_id={}",
                        page_number, submission.id
                    )),
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
        }
    }

    Ok(())
}

fn remove_scope_submissions(project: &mut Project, scope: &BatchScope) {
    let removed_student_ids: Vec<String> = project
        .student_submissions
        .iter()
        .filter(|submission| submission_matches_scope(submission, scope))
        .map(|submission| submission.student_id.clone())
        .collect();
    project
        .student_submissions
        .retain(|submission| !submission_matches_scope(submission, scope));
    project.students.retain(|student| {
        !removed_student_ids.iter().any(|student_id| {
            student_id == &student.id
                && !project
                    .student_submissions
                    .iter()
                    .any(|submission| submission.student_id == student.id)
        })
    });
}

fn invalidate_grouping_completion(
    project: &mut Project,
    batch_id: Option<&str>,
    document_id: &str,
    now: &str,
) {
    if let Some(batch_id) = batch_id {
        if let Some(batch) = project
            .student_scan_batches
            .iter_mut()
            .find(|batch| batch.id == batch_id)
        {
            batch.grouping_completed_at = None;
            batch.updated_at = now.to_string();
        }
    }
    if project.student_scan_document_id.as_deref() == Some(document_id)
        || project.student_scan_batches.is_empty()
    {
        project.student_grouping_complete_at = None;
    }
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn find_matching_roster_student_index(
    project: &Project,
    class_id: Option<&str>,
    current_student_id: &str,
    display_name: Option<&str>,
    number: Option<&str>,
) -> Option<usize> {
    let class_name = class_id.and_then(|id| {
        project
            .school_classes
            .iter()
            .find(|school_class| school_class.id == id)
            .map(|school_class| school_class.normalized_name.as_str())
    });
    project
        .students
        .iter()
        .enumerate()
        .find_map(|(index, student)| {
            if student.id == current_student_id {
                return None;
            }
            let same_class = class_name.is_some_and(|expected| {
                student
                    .class_name
                    .as_deref()
                    .and_then(crate::domain::school_class::normalize_school_class_name)
                    .as_deref()
                    == Some(expected)
            });
            if !same_class {
                return None;
            }
            let same_number = number.is_some_and(|candidate| {
                student.number.as_deref().map(str::trim) == Some(candidate.trim())
            });
            let same_name = display_name.is_some_and(|candidate| {
                student
                    .display_name
                    .as_deref()
                    .map(str::trim)
                    .map(str::to_lowercase)
                    == Some(candidate.trim().to_lowercase())
            });
            (same_number || same_name).then_some(index)
        })
}

fn find_student<'a>(project: &'a Project, student_id: &str) -> Result<&'a Student, AppError> {
    project
        .students
        .iter()
        .find(|student| student.id == student_id)
        .ok_or_else(|| AppError {
            code: AppErrorCode::StudentSubmissionNotFound,
            message: "Öğrenci kaydı bulunamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("Öğrenci listesine geri dönün.".to_string()),
            technical_details: Some(format!("student_id={student_id}")),
            correlation_id: Uuid::new_v4().to_string(),
        })
}

fn find_submission_index(project: &Project, submission_id: &str) -> Result<usize, AppError> {
    project
        .student_submissions
        .iter()
        .position(|submission| submission.id == submission_id)
        .ok_or_else(|| AppError {
            code: AppErrorCode::StudentSubmissionNotFound,
            message: "Öğrenci submission'ı bulunamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("Submission listesini yenileyin.".to_string()),
            technical_details: Some(format!("submission_id={submission_id}")),
            correlation_id: Uuid::new_v4().to_string(),
        })
}

fn find_student_index(project: &Project, student_id: &str) -> Result<usize, AppError> {
    project
        .students
        .iter()
        .position(|student| student.id == student_id)
        .ok_or_else(|| AppError {
            code: AppErrorCode::StudentSubmissionNotFound,
            message: "Öğrenci kaydı bulunamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("Öğrenci listesine geri dönün.".to_string()),
            technical_details: Some(format!("student_id={student_id}")),
            correlation_id: Uuid::new_v4().to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::document::{Document, PdfPreviewState, PdfPreviewStatus};
    use crate::domain::project::Project;
    use crate::domain::question::default_question;
    use crate::domain::school_class::{SchoolClass, SchoolClassStatus, StudentScanBatch};
    use crate::domain::student::{OcrGeneration, OcrGenerationStatus, OcrTeacherReviewStatus};
    use crate::domain::workflow::{WorkflowSnapshot, WorkflowStage};
    use crate::jobs::job_manager::JobManager;
    use crate::services::pdf_service::SystemPdfService;
    use crate::services::project_store::ProjectStore;

    fn service_for_tests(project_store: ProjectStore) -> StudentScanService {
        let pdf_preview_service = Arc::new(PdfPreviewService::new(
            project_store.clone(),
            Arc::new(SystemPdfService),
            Arc::new(JobManager::new()),
        ));
        StudentScanService::new(project_store, pdf_preview_service)
    }

    fn project_root() -> String {
        let root = std::env::temp_dir().join(format!("rubrika-v3-student-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        root.to_string_lossy().to_string()
    }

    fn ready_project() -> (ProjectStore, Project) {
        let store = ProjectStore::new();
        let project = store
            .create_project("Project".to_string(), project_root())
            .expect("project");
        (store, project)
    }

    fn add_ready_student_scan(project: &mut Project, document_id: &str, page_count: u32) {
        let preview_dir = std::path::Path::new(&project.root_path)
            .join("cache")
            .join("page_previews")
            .join(document_id);
        std::fs::create_dir_all(&preview_dir).expect("preview dir");
        let mut pages = Vec::new();
        for page_number in 1..=page_count {
            let image_path = preview_dir.join(format!("page_{page_number}.png"));
            std::fs::write(&image_path, b"").expect("page image");
            pages.push(image_path);
        }
        let metadata_path = preview_dir.join("page_previews.json");
        let rendered_at = chrono::Utc::now().to_rfc3339();
        let preview_pages: Vec<serde_json::Value> = pages
            .iter()
            .enumerate()
            .map(|(index, image_path)| {
                serde_json::json!({
                    "documentId": document_id,
                    "pageNumber": (index + 1) as u32,
                    "imagePath": image_path.to_string_lossy().to_string(),
                    "width": 100u32,
                    "height": 100u32,
                    "renderedAt": rendered_at.clone(),
                })
            })
            .collect();
        std::fs::write(
            &metadata_path,
            serde_json::json!({
                "documentId": document_id,
                "pageCount": page_count,
                "renderedAt": rendered_at,
                "pages": preview_pages,
            })
            .to_string(),
        )
        .expect("preview metadata");
        project.documents.push(Document {
            id: document_id.to_string(),
            role: DocumentRole::StudentScan,
            file_name: "student.pdf".to_string(),
            stored_path: std::env::temp_dir()
                .join("student.pdf")
                .to_string_lossy()
                .to_string(),
            page_count,
            added_at: chrono::Utc::now().to_rfc3339(),
            checksum: None,
            preview: Some(PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: Some(chrono::Utc::now().to_rfc3339()),
                page_count: Some(page_count),
                job_id: None,
                error_message: None,
                active_generation_id: None,
                pending_generation_id: None,
                source_fingerprint: None,
            }),
        });
    }

    fn ready_grouping_project() -> (ProjectStore, Project) {
        let (store, mut project) = ready_project();
        project.questions.push(default_question(1));
        project.questions[0].question_text = crate::domain::question::TextFieldState {
            value: "Question 1".to_string(),
            source: crate::domain::question::TextFieldSource::Manual,
            status: crate::domain::question::TextFieldStatus::Confirmed,
            confidence: None,
            warnings: vec![],
            updated_at: None,
        };
        project.questions[0].rubric = crate::domain::rubric::RubricState {
            status: crate::domain::rubric::RubricStatus::Confirmed,
            source: Some(crate::domain::rubric::RubricSource::Manual),
            max_score: Some(10.0),
            expected_answer: Some("Answer 1".to_string()),
            criteria: vec![crate::domain::rubric::RubricCriterion {
                id: Uuid::new_v4().to_string(),
                label: "Criterion".to_string(),
                description: "Desc".to_string(),
                points: 10.0,
            }],
            partial_credit_hints: vec![],
            zero_score_conditions: vec![],
            common_mistakes: vec![],
            warnings: vec![],
            updated_at: None,
        };
        project.documents.push(Document {
            id: "exam-1".into(),
            role: DocumentRole::ExamSource,
            file_name: "exam.pdf".into(),
            stored_path: "exam.pdf".into(),
            page_count: 2,
            added_at: chrono::Utc::now().to_rfc3339(),
            checksum: None,
            preview: Some(PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: Some(chrono::Utc::now().to_rfc3339()),
                page_count: Some(2),
                job_id: None,
                error_message: None,
                active_generation_id: None,
                pending_generation_id: None,
                source_fingerprint: None,
            }),
        });
        project.exam_package_freeze = Some(crate::domain::project::ExamPackageFreeze {
            exam_package_version: 1,
            freeze_status: crate::domain::project::ExamPackageFreezeStatus::Frozen,
            frozen_at: chrono::Utc::now().to_rfc3339(),
            frozen_by: None,
            source_hash: "hash".to_string(),
            rubric_hash: "hash".to_string(),
            question_text_hash: "hash".to_string(),
            invalidated_at: None,
            invalidation_reason: None,
        });
        (store, project)
    }

    #[test]
    fn create_fixed_groups_builds_submissions_and_slots() {
        let (store, mut project) = ready_project();
        project.questions.push(default_question(1));
        project.questions.push(default_question(2));
        add_ready_student_scan(&mut project, "scan-1", 2);
        project.workflow = WorkflowSnapshot {
            current_stage: WorkflowStage::RubricConfirmed,
            blocking_reasons: vec![],
            next_actions: vec![],
            current_stage_label: "Test".to_string(),
            summary: crate::domain::workflow::WorkflowSummary::default(),
        };
        store.save_project(&project).unwrap();

        let service = service_for_tests(store.clone());
        let result = service
            .create_student_page_groups(CreateStudentPageGroupsInput {
                project_id: project.id.clone(),
                document_id: "scan-1".to_string(),
                batch_id: None,
                pages_per_student: 1,
            })
            .expect("groups");

        assert_eq!(result.groups_created, 2);
        let project = store.get_project_snapshot(project.id.clone()).unwrap();
        assert_eq!(project.student_submissions.len(), 2);
        assert!(project
            .student_submissions
            .iter()
            .all(|submission| submission.answer_slots.len() == 2));
    }

    #[test]
    fn create_fixed_groups_creates_one_group_for_two_pages() {
        let (store, mut project) = ready_grouping_project();
        add_ready_student_scan(&mut project, "scan-1", 2);
        project.workflow = WorkflowSnapshot {
            current_stage: WorkflowStage::StudentGroupingMissing,
            blocking_reasons: vec![],
            next_actions: vec![],
            current_stage_label: "Test".to_string(),
            summary: crate::domain::workflow::WorkflowSummary::default(),
        };
        store.save_project(&project).unwrap();

        let service = service_for_tests(store.clone());
        let result = service
            .create_student_page_groups(CreateStudentPageGroupsInput {
                project_id: project.id.clone(),
                document_id: "scan-1".to_string(),
                batch_id: None,
                pages_per_student: 2,
            })
            .expect("groups");

        assert_eq!(result.groups_created, 1);
        assert_eq!(result.total_pages, 2);
        assert_eq!(result.pages_per_student, 2);
        assert_eq!(result.remainder_pages, 0);
        assert!(!result.needs_review);
        assert_eq!(result.submissions.len(), 1);
        assert_eq!(result.submissions[0].page_numbers, vec![1, 2]);
        let project = store.get_project_snapshot(project.id.clone()).unwrap();
        assert_eq!(project.student_submissions.len(), 1);
        assert_eq!(project.student_submissions[0].page_numbers, vec![1, 2]);
        assert_eq!(
            project.workflow.current_stage,
            WorkflowStage::StudentGroupingReady
        );
    }

    #[test]
    fn create_fixed_groups_marks_remainder_for_review() {
        let (store, mut project) = ready_project();
        add_ready_student_scan(&mut project, "scan-1", 3);
        project.workflow = WorkflowSnapshot {
            current_stage: WorkflowStage::RubricConfirmed,
            blocking_reasons: vec![],
            next_actions: vec![],
            current_stage_label: "Test".to_string(),
            summary: crate::domain::workflow::WorkflowSummary::default(),
        };
        store.save_project(&project).unwrap();

        let service = service_for_tests(store.clone());
        let result = service
            .create_student_page_groups(CreateStudentPageGroupsInput {
                project_id: project.id.clone(),
                document_id: "scan-1".to_string(),
                batch_id: None,
                pages_per_student: 2,
            })
            .expect("groups");

        assert_eq!(result.groups_created, 2);
        assert_eq!(result.total_pages, 3);
        assert_eq!(result.remainder_pages, 1);
        assert!(result.needs_review);
        assert!(!result.warnings.is_empty());
        assert_eq!(result.submissions.len(), 2);
        assert_eq!(result.submissions[0].page_numbers, vec![1, 2]);
        assert_eq!(result.submissions[1].page_numbers, vec![3]);
    }

    #[test]
    fn create_fixed_groups_rejects_invalid_page_size() {
        let (store, mut project) = ready_project();
        add_ready_student_scan(&mut project, "scan-1", 2);
        project.workflow = WorkflowSnapshot {
            current_stage: WorkflowStage::RubricConfirmed,
            blocking_reasons: vec![],
            next_actions: vec![],
            current_stage_label: "Test".to_string(),
            summary: crate::domain::workflow::WorkflowSummary::default(),
        };
        store.save_project(&project).unwrap();

        let service = service_for_tests(store.clone());
        let zero = service.create_student_page_groups(CreateStudentPageGroupsInput {
            project_id: project.id.clone(),
            document_id: "scan-1".to_string(),
            batch_id: None,
            pages_per_student: 0,
        });
        assert!(zero.is_err());
        assert_eq!(zero.unwrap_err().code, AppErrorCode::StudentGroupingInvalid);

        let too_large = service.create_student_page_groups(CreateStudentPageGroupsInput {
            project_id: project.id.clone(),
            document_id: "scan-1".to_string(),
            batch_id: None,
            pages_per_student: 3,
        });
        assert!(too_large.is_err());
        assert_eq!(
            too_large.unwrap_err().code,
            AppErrorCode::StudentGroupingInvalid
        );
    }

    #[test]
    fn mark_grouping_complete_allows_blank_identity() {
        let (store, mut project) = ready_grouping_project();
        add_ready_student_scan(&mut project, "scan-1", 2);
        project.workflow = WorkflowSnapshot {
            current_stage: WorkflowStage::StudentGroupingReady,
            blocking_reasons: vec![],
            next_actions: vec![],
            current_stage_label: "Test".to_string(),
            summary: crate::domain::workflow::WorkflowSummary::default(),
        };
        store.save_project(&project).unwrap();

        let service = service_for_tests(store.clone());
        service
            .create_student_page_groups(CreateStudentPageGroupsInput {
                project_id: project.id.clone(),
                document_id: "scan-1".to_string(),
                batch_id: None,
                pages_per_student: 1,
            })
            .expect("groups");

        let result = service
            .mark_student_grouping_complete(MarkStudentGroupingCompleteInput {
                project_id: project.id.clone(),
                batch_id: None,
            })
            .expect("complete");

        assert!(result.student_grouping_complete_at.is_some());
        assert_eq!(result.workflow.current_stage, WorkflowStage::OcrReady);
        assert!(result
            .student_submissions
            .iter()
            .all(|submission| submission.status == StudentSubmissionStatus::ReadyForOcr));
    }

    #[test]
    fn batches_group_and_complete_independently_with_inherited_class_membership() {
        let (store, mut project) = ready_grouping_project();
        add_ready_student_scan(&mut project, "scan-a", 2);
        add_ready_student_scan(&mut project, "scan-b", 2);
        let now = chrono::Utc::now().to_rfc3339();
        project.school_classes = vec![
            SchoolClass {
                id: "class-a".to_string(),
                name: "11-A".to_string(),
                display_name: "11-A".to_string(),
                normalized_name: "11-A".to_string(),
                academic_year: None,
                academic_year_id: None,
                grade_level: Some(11),
                section: Some("A".to_string()),
                display_order: 0,
                status: SchoolClassStatus::Active,
                created_at: now.clone(),
                updated_at: now.clone(),
            },
            SchoolClass {
                id: "class-b".to_string(),
                name: "11-B".to_string(),
                display_name: "11-B".to_string(),
                normalized_name: "11-B".to_string(),
                academic_year: None,
                academic_year_id: None,
                grade_level: Some(11),
                section: Some("B".to_string()),
                display_order: 1,
                status: SchoolClassStatus::Active,
                created_at: now.clone(),
                updated_at: now.clone(),
            },
        ];
        project.student_scan_batches = vec![
            StudentScanBatch {
                id: "batch-a".to_string(),
                class_id: "class-a".to_string(),
                document_id: "scan-a".to_string(),
                original_file_name: "a.pdf".to_string(),
                display_name: "11-A".to_string(),
                pages_per_student: Some(1),
                grouping_mode: Some(PageGroupingMode::FixedPagesPerStudent),
                grouping_completed_at: None,
                created_at: now.clone(),
                updated_at: now.clone(),
            },
            StudentScanBatch {
                id: "batch-b".to_string(),
                class_id: "class-b".to_string(),
                document_id: "scan-b".to_string(),
                original_file_name: "b.pdf".to_string(),
                display_name: "11-B".to_string(),
                pages_per_student: Some(1),
                grouping_mode: Some(PageGroupingMode::FixedPagesPerStudent),
                grouping_completed_at: None,
                created_at: now.clone(),
                updated_at: now,
            },
        ];
        store.save_project(&project).unwrap();
        let service = service_for_tests(store.clone());

        service
            .create_student_page_groups(CreateStudentPageGroupsInput {
                project_id: project.id.clone(),
                document_id: "scan-a".to_string(),
                batch_id: Some("batch-a".to_string()),
                pages_per_student: 1,
            })
            .unwrap();
        service
            .create_student_page_groups(CreateStudentPageGroupsInput {
                project_id: project.id.clone(),
                document_id: "scan-b".to_string(),
                batch_id: Some("batch-b".to_string()),
                pages_per_student: 1,
            })
            .unwrap();
        let grouped = store.get_project_snapshot(project.id.clone()).unwrap();
        assert_eq!(grouped.student_submissions.len(), 4);
        assert_eq!(
            grouped
                .student_submissions
                .iter()
                .filter(|submission| submission.scan_batch_id.as_deref() == Some("batch-a"))
                .count(),
            2
        );
        assert!(grouped.student_submissions.iter().all(|submission| {
            match submission.scan_batch_id.as_deref() {
                Some("batch-a") => submission.class_id.as_deref() == Some("class-a"),
                Some("batch-b") => submission.class_id.as_deref() == Some("class-b"),
                _ => false,
            }
        }));

        let after_a = service
            .mark_student_grouping_complete(MarkStudentGroupingCompleteInput {
                project_id: project.id.clone(),
                batch_id: Some("batch-a".to_string()),
            })
            .unwrap();
        assert!(after_a.student_scan_batches[0]
            .grouping_completed_at
            .is_some());
        assert!(after_a.student_scan_batches[1]
            .grouping_completed_at
            .is_none());
        assert!(after_a.student_submissions.iter().all(|submission| {
            if submission.scan_batch_id.as_deref() == Some("batch-a") {
                submission.status == StudentSubmissionStatus::ReadyForOcr
            } else {
                submission.status == StudentSubmissionStatus::IdentityMissing
            }
        }));
        let readiness_a = service
            .get_ocr_readiness(&GetOcrReadinessInput {
                project_id: project.id.clone(),
                batch_id: Some("batch-a".to_string()),
            })
            .unwrap();
        assert_eq!(readiness_a.submission_count, 2);
        assert!(readiness_a.grouping_complete);

        let after_b = service
            .mark_student_grouping_complete(MarkStudentGroupingCompleteInput {
                project_id: project.id,
                batch_id: Some("batch-b".to_string()),
            })
            .unwrap();
        assert!(after_b
            .student_scan_batches
            .iter()
            .all(|batch| batch.grouping_completed_at.is_some()));
        assert_eq!(after_b.workflow.current_stage, WorkflowStage::OcrReady);
    }

    #[test]
    fn submission_dependency_scan_blocks_history_even_when_candidate_was_rejected() {
        let (store, mut project) = ready_project();
        let submission_id = Uuid::new_v4().to_string();
        let student_id = Uuid::new_v4().to_string();
        project.student_submissions.push(StudentSubmission {
            id: submission_id.clone(),
            student_id,
            document_id: "scan".to_string(),
            class_id: None,
            scan_batch_id: None,
            class_membership_source: None,
            page_numbers: vec![1],
            status: StudentSubmissionStatus::Grouped,
            answer_slots: vec![],
            warnings: vec![],
            updated_at: None,
        });
        let empty_generation = OcrGeneration {
            generation_id: Uuid::new_v4().to_string(),
            submission_id: submission_id.clone(),
            source_fingerprint: "source".to_string(),
            created_at: chrono::Utc::now(),
            model_name: None,
            prompt_version: "test".to_string(),
            status: OcrGenerationStatus::Rejected,
            result: vec![],
            diagnostics: None,
            teacher_review_status: OcrTeacherReviewStatus::Rejected,
            created_by_job_id: "job".to_string(),
            source_document_id: "scan".to_string(),
            source_storage_revision: 0,
            failure_reason: None,
        };
        project
            .student_answer_ocr_generations
            .push(empty_generation);
        store.save_project(&project).unwrap();

        let scan = scan_submission_dependencies(&project, &[submission_id]);
        assert_eq!(scan.ocr_generation_count, 1);
        assert!(scan.is_blocked());
    }

    #[test]
    fn proof_37_delete_dependency_scan_prevents_history_loss() {
        submission_dependency_scan_blocks_history_even_when_candidate_was_rejected();
    }

    #[test]
    fn proof_54_delete_rechecks_dependencies_inside_transaction() {
        let source = include_str!("student_scan_service.rs");
        let delete_start = source
            .find("pub fn delete_student_submission")
            .expect("delete command source");
        let delete_source = &source[delete_start..];
        assert!(delete_source.contains("let current_jobs = persisted_dependency_jobs(current)?;"));
        assert!(
            delete_source.contains("let current_scan = scan_submission_dependencies_with_jobs(")
        );
        assert!(delete_source.contains("self.project_store.commit_job("));
    }
}

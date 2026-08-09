use serde::{Deserialize, Serialize};

use super::assessment::{AssessmentActivity, TeachingAssignment};
use super::document::Document;
use super::question::Question;
use super::school_class::{SchoolClass, StudentScanBatch};
use super::scoring::{ScoringAnchor, ScoringRecord};
use super::speaking::SpeakingExam;
use super::student::{
    OcrGeneration, OcrGenerationStatus, PageGroupingMode, Student, StudentAnswerCropTemplate,
    StudentAnswerOcrRecord, StudentIdentityCropTemplate, StudentSubmission,
};
use super::workflow::WorkflowSnapshot;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub root_path: String,
    /// Authoritative storage revision. Legacy project files may omit this
    /// field and are loaded as revision zero.
    #[serde(default)]
    pub storage_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub academic_year_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub course_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub course_name: Option<String>,
    #[serde(default)]
    pub sections: Vec<Section>,
    #[serde(default)]
    pub students: Vec<Student>,
    #[serde(default)]
    pub school_classes: Vec<SchoolClass>,
    #[serde(default)]
    pub teaching_assignments: Vec<TeachingAssignment>,
    #[serde(default)]
    pub assessment_activities: Vec<AssessmentActivity>,
    #[serde(default)]
    pub student_scan_batches: Vec<StudentScanBatch>,
    #[serde(default)]
    pub student_submissions: Vec<StudentSubmission>,
    #[serde(default)]
    pub student_answer_ocr_records: Vec<StudentAnswerOcrRecord>,
    /// Versioned OCR history. The flat records above remain a compatibility
    /// read projection for legacy consumers and point at the active records.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub student_answer_ocr_generations: Vec<OcrGeneration>,
    #[serde(default)]
    pub student_answer_crop_template: StudentAnswerCropTemplate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub student_identity_crop_template: Option<StudentIdentityCropTemplate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub student_scan_document_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub student_grouping_mode: Option<PageGroupingMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub student_pages_per_student: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub student_grouping_complete_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_question_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exam_package_freeze: Option<ExamPackageFreeze>,
    /// Backend-authoritative pointer to the written-family assessment activity
    /// that the project-level written collections (`questions`,
    /// `student_submissions`, OCR records, scoring records, freeze) currently
    /// belong to. Additive TD-01 field; legacy projects omit it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_written_assessment_activity_id: Option<String>,
    pub documents: Vec<Document>,
    pub questions: Vec<Question>,
    #[serde(default)]
    pub scoring_records: Vec<ScoringRecord>,
    /// Teacher-approved scoring anchors. Legacy project files omit this
    /// additive collection and deserialize it as empty without touching any
    /// existing scoring records.
    #[serde(default)]
    pub scoring_anchors: Vec<ScoringAnchor>,
    #[serde(default)]
    pub speaking_exams: Vec<SpeakingExam>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_scoring_run_id: Option<String>,
    /// CACHE ONLY — persisted diagnostic snapshot of the written workflow.
    /// This is NOT authoritative: the live workflow is always recomputed from
    /// canonical project state by `workflow_engine::evaluate_workflow` (and the
    /// `get_workflow_snapshot` command returns that live result, never this
    /// field). Readers must not use this field as workflow truth; the persisted
    /// value exists solely as a cached/diagnostic projection and is refreshed
    /// opportunistically after mutations. `current_stage` may be read as a
    /// running-flag hint during live evaluation (e.g. `ScoringRunning`), never
    /// as a short circuit for the computed stage.
    pub workflow: WorkflowSnapshot,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExamPackageFreezeStatus {
    Frozen,
    Invalidated,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExamPackageFreeze {
    pub exam_package_version: u32,
    pub freeze_status: ExamPackageFreezeStatus,
    pub frozen_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen_by: Option<String>,
    pub source_hash: String,
    pub rubric_hash: String,
    pub question_text_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invalidated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invalidation_reason: Option<String>,
    /// TD-01: owning written-family assessment activity. Legacy freezes omit it
    /// and the migration attaches them to the sole (or synthetic) activity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessment_activity_id: Option<String>,
}

impl Project {
    pub fn invalidate_exam_package_if_frozen(&mut self, reason: &str) {
        if let Some(freeze) = self.exam_package_freeze.as_mut() {
            if freeze.freeze_status == ExamPackageFreezeStatus::Frozen {
                freeze.freeze_status = ExamPackageFreezeStatus::Invalidated;
                freeze.invalidated_at = Some(chrono::Utc::now().to_rfc3339());
                freeze.invalidation_reason = Some(reason.to_string());
            }
        }
    }

    /// A loaded project has no live in-process job ownership. Candidates left
    /// behind by a previous process are therefore interrupted in memory; the
    /// active flat projection is deliberately untouched.
    pub fn recover_orphaned_ocr_generations(&mut self) -> usize {
        let mut recovered = 0;
        for generation in &mut self.student_answer_ocr_generations {
            if generation.status == OcrGenerationStatus::Candidate {
                generation.status = OcrGenerationStatus::Interrupted;
                generation.failure_reason =
                    Some("Uygulama kapanırken OCR işi tamamlanamadı.".to_string());
                recovered += 1;
            }
        }
        recovered
    }

    /// Resolves the active OCR projection without forcing a legacy project
    /// rewrite. A flat legacy result is treated as the active generation until
    /// its first real OCR mutation creates versioned metadata.
    pub fn resolved_active_ocr_records(&self) -> Vec<StudentAnswerOcrRecord> {
        let active = self
            .student_answer_ocr_generations
            .iter()
            .filter(|generation| generation.status == OcrGenerationStatus::Active)
            .flat_map(|generation| generation.result.clone())
            .collect::<Vec<_>>();
        if active.is_empty() {
            self.student_answer_ocr_records.clone()
        } else {
            active
        }
    }

    pub fn pending_ocr_generations(&self) -> Vec<&OcrGeneration> {
        self.student_answer_ocr_generations
            .iter()
            .filter(|generation| {
                matches!(
                    generation.status,
                    OcrGenerationStatus::Candidate | OcrGenerationStatus::ReadyForReview
                )
            })
            .collect()
    }

    /// Written-family (`written` + `listening`) assessment activity ids.
    pub fn written_family_activity_ids(&self) -> Vec<&str> {
        self.assessment_activities
            .iter()
            .filter(|activity| {
                matches!(
                    activity.assessment_type,
                    super::assessment::AssessmentType::Written
                        | super::assessment::AssessmentType::Listening
                )
            })
            .map(|activity| activity.id.as_str())
            .collect()
    }

    pub fn is_written_family_activity(&self, activity_id: &str) -> bool {
        self.assessment_activities.iter().any(|activity| {
            activity.id == activity_id
                && matches!(
                    activity.assessment_type,
                    super::assessment::AssessmentType::Written
                        | super::assessment::AssessmentType::Listening
                )
        })
    }

    /// Resolves the canonical written-family scope for the project-level
    /// written collections. Precedence:
    /// 1. the persisted active written pointer (backend-authoritative),
    /// 2. a project with exactly one written-family activity,
    /// 3. `None` for legacy projects that never used the assessment-activity
    ///    organization layer (their flat data is intentionally project-wide).
    ///
    /// A project with several written-family activities and no active pointer
    /// is ambiguous and returns a typed error instead of guessing.
    pub fn resolve_written_scope_id(
        &self,
    ) -> Result<Option<String>, crate::domain::errors::AppError> {
        if let Some(id) = self.active_written_assessment_activity_id.as_deref() {
            if !id.trim().is_empty() {
                if self.is_written_family_activity(id) {
                    return Ok(Some(id.to_string()));
                }
                return Err(crate::domain::errors::AppError {
                    code: crate::domain::errors::AppErrorCode::ActiveWrittenActivityNotFound,
                    message: "Aktif yazılı sınav çalışma alanı bulunamadı. Sınavı tekrar seçin.".to_string(),
                    recoverable: true,
                    suggested_action: Some(
                        "Sınav listesinden geçerli bir yazılı sınav seçin.".to_string(),
                    ),
                    technical_details: Some(format!(
                        "active_written_assessment_activity_id={id} is not a written-family activity"
                    )),
                    correlation_id: uuid::Uuid::new_v4().to_string(),
                });
            }
        }
        let written = self.written_family_activity_ids();
        match written.as_slice() {
            [only] => Ok(Some((*only).to_string())),
            [] => Ok(None),
            _ => Err(crate::domain::errors::AppError {
                code: crate::domain::errors::AppErrorCode::WrittenScopeAmbiguous,
                message: "Bu projede birden fazla yazılı sınav var; çalışma alanı seçilmedi."
                    .to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Yazılı sınav çalışma alanına girip işlemi tekrar deneyin.".to_string(),
                ),
                technical_details: Some(
                    "multiple written-family activities and no active pointer".to_string(),
                ),
                correlation_id: uuid::Uuid::new_v4().to_string(),
            }),
        }
    }

    /// True when `record_activity` belongs to the resolved written scope.
    /// Untagged legacy records belong to the scope only when the scope is the
    /// deterministic single-activity (or legacy no-activity) case.
    pub fn record_belongs_to_written_scope(
        &self,
        scope_id: Option<&str>,
        record_activity: Option<&str>,
    ) -> bool {
        match (scope_id, record_activity) {
            (Some(scope), Some(record)) => scope == record,
            (Some(_), None) => self.written_family_activity_ids().len() == 1,
            (None, _) => true,
        }
    }

    /// Read-model view of the project-level written collections scoped to the
    /// resolved written activity. On an ambiguous project (several written
    /// activities without an active pointer) the fallback returns the whole
    /// legacy projection; WRITE paths must use `resolve_written_scope_id`
    /// instead so they never stamp records into an unknown scope.
    pub fn written_scope_view(&self) -> WrittenScopeView<'_> {
        let scope_id = self
            .active_written_assessment_activity_id
            .as_deref()
            .filter(|id| !id.trim().is_empty())
            .filter(|id| self.is_written_family_activity(id))
            .map(str::to_string)
            .or_else(|| {
                let written = self.written_family_activity_ids();
                match written.as_slice() {
                    [only] => Some((*only).to_string()),
                    _ => None,
                }
            });
        let questions = self
            .questions
            .iter()
            .filter(|q| {
                self.record_belongs_to_written_scope(
                    scope_id.as_deref(),
                    q.assessment_activity_id.as_deref(),
                )
            })
            .collect();
        let student_submissions = self
            .student_submissions
            .iter()
            .filter(|s| {
                self.record_belongs_to_written_scope(
                    scope_id.as_deref(),
                    s.assessment_activity_id.as_deref(),
                )
            })
            .collect();
        let student_answer_ocr_records = self
            .student_answer_ocr_records
            .iter()
            .filter(|r| {
                self.record_belongs_to_written_scope(
                    scope_id.as_deref(),
                    r.assessment_activity_id.as_deref(),
                )
            })
            .collect();
        let scoring_records = self
            .scoring_records
            .iter()
            .filter(|r| {
                self.record_belongs_to_written_scope(
                    scope_id.as_deref(),
                    r.assessment_activity_id.as_deref(),
                )
            })
            .collect();
        let scoring_anchors = self
            .scoring_anchors
            .iter()
            .filter(|r| {
                self.record_belongs_to_written_scope(
                    scope_id.as_deref(),
                    r.assessment_activity_id.as_deref(),
                )
            })
            .collect();
        let exam_package_freeze = self.exam_package_freeze.as_ref().filter(|freeze| {
            self.record_belongs_to_written_scope(
                scope_id.as_deref(),
                freeze.assessment_activity_id.as_deref(),
            )
        });
        WrittenScopeView {
            scope_id,
            questions,
            student_submissions,
            student_answer_ocr_records,
            scoring_records,
            scoring_anchors,
            exam_package_freeze,
        }
    }
}

/// Borrowed, scope-filtered projection of the project-level written collections.
/// Used by domain read-models so `Project` remains the single canonical store.
#[derive(Debug, Clone)]
pub struct WrittenScopeView<'a> {
    pub scope_id: Option<String>,
    pub questions: Vec<&'a Question>,
    pub student_submissions: Vec<&'a StudentSubmission>,
    pub student_answer_ocr_records: Vec<&'a StudentAnswerOcrRecord>,
    pub scoring_records: Vec<&'a ScoringRecord>,
    pub scoring_anchors: Vec<&'a ScoringAnchor>,
    pub exam_package_freeze: Option<&'a ExamPackageFreeze>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub id: String,
    pub name: String,
    pub students: Vec<Student>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::student::{
        OcrGeneration, OcrGenerationStatus, OcrTeacherReviewStatus, StudentAnswerOcrRecord,
        StudentAnswerOcrStatus,
    };
    use crate::domain::workflow::{
        WorkflowReadiness, WorkflowSnapshot, WorkflowStage, WorkflowSummary,
    };

    fn empty_project() -> Project {
        Project {
            id: "project-1".into(),
            name: "Project".into(),
            created_at: "now".into(),
            updated_at: "now".into(),
            root_path: "/tmp/project".into(),
            storage_revision: 0,
            academic_year_id: None,
            course_id: None,
            course_name: None,
            sections: vec![],
            students: vec![],
            school_classes: vec![],
            teaching_assignments: vec![],
            assessment_activities: vec![],
            student_scan_batches: vec![],
            student_submissions: vec![],
            student_answer_ocr_records: vec![],
            student_answer_ocr_generations: vec![],
            student_answer_crop_template: Default::default(),
            student_identity_crop_template: None,
            student_scan_document_id: None,
            student_grouping_mode: None,
            student_pages_per_student: None,
            student_grouping_complete_at: None,
            expected_question_count: None,
            exam_package_freeze: None,
            active_written_assessment_activity_id: None,
            documents: vec![],
            questions: vec![],
            scoring_records: vec![],
            scoring_anchors: vec![],
            speaking_exams: vec![],
            latest_scoring_run_id: None,
            workflow: WorkflowSnapshot {
                current_stage: WorkflowStage::DocumentsMissing,
                current_stage_label: String::new(),
                blocking_reasons: vec![],
                next_actions: vec![],
                summary: WorkflowSummary {
                    text: None,
                    steps: vec![],
                    readiness: WorkflowReadiness {
                        exam_package_freeze: false,
                        student_intake: false,
                        scoring: false,
                    },
                },
            },
        }
    }

    fn flat_record(answer_text: &str) -> StudentAnswerOcrRecord {
        StudentAnswerOcrRecord {
            id: format!("ocr-{answer_text}"),
            submission_id: "submission-1".into(),
            question_id: "q-1".into(),
            question_number: 1,
            answer_text: answer_text.into(),
            status: StudentAnswerOcrStatus::TeacherApproved,
            ..Default::default()
        }
    }

    #[test]
    fn active_generation_projection_is_consistent_after_accept() {
        // Accept flow in student_answer_ocr_service marks one generation
        // Active and mirrors its result into the flat records. The resolved
        // projection must equal the accepted generation's results.
        let mut project = empty_project();
        let accepted = flat_record("new-answer");
        project.student_answer_ocr_generations = vec![OcrGeneration {
            assessment_activity_id: None,
            generation_id: "gen-1".into(),
            submission_id: "submission-1".into(),
            source_fingerprint: "fp-1".into(),
            created_at: chrono::Utc::now(),
            model_name: None,
            prompt_version: "v1".into(),
            status: OcrGenerationStatus::Active,
            result: vec![accepted.clone()],
            diagnostics: None,
            teacher_review_status: OcrTeacherReviewStatus::Approved,
            created_by_job_id: "job-1".into(),
            source_document_id: "doc-1".into(),
            source_storage_revision: 0,
            failure_reason: None,
            job_mode: crate::domain::student::StudentAnswerOcrJobMode::Production,
        }];
        // Mirrored flat record (what accept writes).
        project.student_answer_ocr_records = vec![flat_record("old-answer")];
        project
            .student_answer_ocr_records
            .retain(|record| record.submission_id != "submission-1");
        project
            .student_answer_ocr_records
            .extend(vec![accepted.clone()]);

        let resolved = project.resolved_active_ocr_records();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].answer_text, "new-answer");
    }

    #[test]
    fn flat_records_are_the_fallback_projection_after_reject() {
        // Reject flow leaves no Active generation; the projection must fall
        // back to the canonical flat records unchanged.
        let mut project = empty_project();
        project.student_answer_ocr_records = vec![flat_record("kept-answer")];
        project.student_answer_ocr_generations = vec![OcrGeneration {
            assessment_activity_id: None,
            generation_id: "gen-rejected".into(),
            submission_id: "submission-1".into(),
            source_fingerprint: "fp-2".into(),
            created_at: chrono::Utc::now(),
            model_name: None,
            prompt_version: "v1".into(),
            status: OcrGenerationStatus::Rejected,
            result: vec![flat_record("discarded")],
            diagnostics: None,
            teacher_review_status: OcrTeacherReviewStatus::Rejected,
            created_by_job_id: "job-2".into(),
            source_document_id: "doc-1".into(),
            source_storage_revision: 0,
            failure_reason: None,
            job_mode: crate::domain::student::StudentAnswerOcrJobMode::Production,
        }];

        let resolved = project.resolved_active_ocr_records();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].answer_text, "kept-answer");
    }

    fn written_activity(id: &str) -> crate::domain::assessment::AssessmentActivity {
        crate::domain::assessment::AssessmentActivity {
            id: id.to_string(),
            academic_year_id: "2026-2027".into(),
            course_id: "tde".into(),
            course_name: "TDE".into(),
            title: String::new(),
            grade_level: 9,
            term: 1,
            assessment_type: crate::domain::assessment::AssessmentType::Written,
            workflow_family: crate::domain::assessment::WorkflowFamily::Written,
            sequence_number: 1,
            status: Default::default(),
            common_document_ids: vec![],
            listening_details: None,
            speaking_configuration: None,
            class_applications: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn two_written_activities_isolate_scoped_data_by_pointer() {
        let mut project = empty_project();
        project.assessment_activities =
            vec![written_activity("written-a"), written_activity("written-b")];
        project.active_written_assessment_activity_id = Some("written-a".into());

        let mut qa = crate::domain::question::Question {
            assessment_activity_id: Some("written-a".into()),
            ..crate::domain::question::default_question(1)
        };
        qa.question_text.value = "Sınav A sorusu".into();
        let mut qb = crate::domain::question::Question {
            assessment_activity_id: Some("written-b".into()),
            ..crate::domain::question::default_question(2)
        };
        qb.question_text.value = "Sınav B sorusu".into();
        project.questions = vec![qa.clone(), qb.clone()];

        project.student_submissions = vec![
            crate::domain::student::StudentSubmission {
                assessment_activity_id: Some("written-a".into()),
                id: "sub-a".into(),
                student_id: "st-a".into(),
                document_id: "doc".into(),
                page_numbers: vec![1],
                status: crate::domain::student::StudentSubmissionStatus::Grouped,
                answer_slots: vec![],
                warnings: vec![],
                class_id: None,
                scan_batch_id: None,
                class_membership_source: None,
                updated_at: None,
            },
            crate::domain::student::StudentSubmission {
                assessment_activity_id: Some("written-b".into()),
                id: "sub-b".into(),
                student_id: "st-b".into(),
                document_id: "doc".into(),
                page_numbers: vec![1],
                status: crate::domain::student::StudentSubmissionStatus::Grouped,
                answer_slots: vec![],
                warnings: vec![],
                class_id: None,
                scan_batch_id: None,
                class_membership_source: None,
                updated_at: None,
            },
        ];

        // While pointer is on written-a, only A data is visible.
        let view_a = project.written_scope_view();
        assert_eq!(view_a.questions.len(), 1);
        assert_eq!(view_a.questions[0].number, 1);
        assert_eq!(view_a.questions[0].question_text.value, "Sınav A sorusu");
        assert_eq!(view_a.student_submissions.len(), 1);
        assert_eq!(view_a.student_submissions[0].id, "sub-a");

        // Switching the backend pointer to written-b isolates B.
        project.active_written_assessment_activity_id = Some("written-b".into());
        let view_b = project.written_scope_view();
        assert_eq!(view_b.questions.len(), 1);
        assert_eq!(view_b.questions[0].number, 2);
        assert_eq!(view_b.questions[0].question_text.value, "Sınav B sorusu");
        assert_eq!(view_b.student_submissions.len(), 1);
        assert_eq!(view_b.student_submissions[0].id, "sub-b");

        // A's data is untouched; no cross-activity leak in either direction.
        assert_eq!(project.questions.len(), 2);
        assert_eq!(project.student_submissions.len(), 2);
    }

    #[test]
    fn ambiguous_written_scope_without_pointer_returns_typed_error() {
        let mut project = empty_project();
        project.assessment_activities =
            vec![written_activity("written-a"), written_activity("written-b")];
        let error = project.resolve_written_scope_id().unwrap_err();
        assert_eq!(
            error.code,
            crate::domain::errors::AppErrorCode::WrittenScopeAmbiguous
        );
    }

    #[test]
    fn single_written_activity_resolves_as_scope_without_pointer() {
        let mut project = empty_project();
        project.assessment_activities = vec![written_activity("written-only")];
        assert_eq!(
            project.resolve_written_scope_id().unwrap().as_deref(),
            Some("written-only")
        );
    }
}

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::domain::document::DocumentRole;
use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::project::{ExamPackageFreeze, ExamPackageFreezeStatus, Project};
use crate::domain::question::{is_question_text_ready, AnswerType, Question};
use crate::domain::rubric::{
    normalize_text, teacher_facing_warnings, validate_rubric_state, RubricCriterion, RubricSource,
    RubricState, RubricStatus, RubricValidationIssue,
};
use crate::services::project_store::ProjectStore;
use crate::services::workflow_engine;

#[derive(Clone)]
pub struct RubricService {
    project_store: ProjectStore,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRubricJsonInput {
    pub project_id: String,
    #[serde(default)]
    pub document_id: Option<String>,
    #[serde(default)]
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRubricJsonOutput {
    pub imported_count: u32,
    pub missing_count: u32,
    pub invalid_count: u32,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateQuestionRubricInput {
    pub project_id: String,
    pub question_id: String,
    #[serde(default)]
    pub answer_type: Option<AnswerType>,
    pub max_score: Option<f32>,
    pub expected_answer: Option<String>,
    pub criteria: Vec<RubricCriterion>,
    pub partial_credit_hints: Vec<String>,
    pub zero_score_conditions: Vec<String>,
    pub common_mistakes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RubricQuestionSnapshot {
    pub question: Question,
    pub validation: RubricValidationSnapshot,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RubricValidationSnapshot {
    pub valid: bool,
    pub confirmable: bool,
    pub warnings: Vec<String>,
    pub issues: Vec<RubricValidationIssue>,
    pub total_points: Option<f32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RubricStateSnapshot {
    pub project_id: String,
    pub current_stage: String,
    pub items: Vec<RubricQuestionSnapshot>,
    pub missing_count: u32,
    pub imported_count: u32,
    pub manual_count: u32,
    pub confirmed_count: u32,
    pub invalid_count: u32,
    pub warnings: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RubricValidationQuestionSnapshot {
    pub question_id: String,
    pub number: u32,
    pub status: String,
    pub valid: bool,
    pub warnings: Vec<String>,
    pub issues: Vec<RubricValidationIssue>,
    pub total_points: Option<f32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RubricValidationReport {
    pub project_id: String,
    pub valid: bool,
    pub confirmable: bool,
    pub warnings: Vec<String>,
    pub blocking_questions: Vec<u32>,
    pub questions: Vec<RubricValidationQuestionSnapshot>,
}

#[derive(Debug, Clone)]
struct NormalizedRubricEntry {
    question_number: u32,
    max_score: Option<f32>,
    expected_answer: Option<String>,
    criteria: Vec<RubricCriterion>,
    partial_credit_hints: Vec<String>,
    zero_score_conditions: Vec<String>,
    common_mistakes: Vec<String>,
    warnings: Vec<String>,
}

impl RubricService {
    pub fn new(project_store: ProjectStore) -> Self {
        Self { project_store }
    }

    pub fn import_rubric_json(
        &self,
        input: ImportRubricJsonInput,
    ) -> Result<ImportRubricJsonOutput, AppError> {
        let mut project = self.load_project(&input.project_id)?;
        ensure_question_text_ready(&project)?;
        if project.questions.is_empty() {
            return Err(AppError {
                code: AppErrorCode::RubricNotReady,
                message: "No questions are available for rubric preparation.".to_string(),
                recoverable: true,
                suggested_action: Some("Run question text extraction first.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        let source_text = self.read_source_text(
            &project,
            input.document_id.as_deref(),
            input.file_path.as_deref(),
        )?;
        let entries = parse_rubric_entries(&source_text)?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut warnings = Vec::new();
        let mut imported_count = 0u32;

        for entry in entries {
            let question = project
                .questions
                .iter_mut()
                .find(|question| question.number == entry.question_number);

            let Some(question) = question else {
                warnings.push(format!(
                    "Question {} JSON içinde var ama projede bulunamadı.",
                    entry.question_number
                ));
                continue;
            };

            imported_count += 1;
            let state = RubricState {
                status: RubricStatus::Imported,
                source: Some(RubricSource::Json),
                max_score: entry.max_score,
                expected_answer: entry.expected_answer.clone(),
                criteria: entry.criteria.clone(),
                partial_credit_hints: entry.partial_credit_hints.clone(),
                zero_score_conditions: entry.zero_score_conditions.clone(),
                common_mistakes: entry.common_mistakes.clone(),
                warnings: entry.warnings.clone(),
                updated_at: Some(now.clone()),
            };
            let validation = validate_rubric_state(&state, Some(&question.answer_type));
            apply_validation_to_question(question, state, validation);
            warnings.extend(teacher_facing_warnings(&question.rubric.warnings));
        }

        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store.save_project(&project)?;

        Ok(ImportRubricJsonOutput {
            imported_count,
            missing_count: project
                .questions
                .iter()
                .filter(|question| question.rubric.status == RubricStatus::Missing)
                .count() as u32,
            invalid_count: project
                .questions
                .iter()
                .filter(|question| question.rubric.status == RubricStatus::Invalid)
                .count() as u32,
            warnings,
        })
    }

    pub fn get_rubric_state(&self, project_id: &str) -> Result<RubricStateSnapshot, AppError> {
        let project = self.load_project(project_id)?;
        Ok(self.build_state_snapshot(&project))
    }

    pub fn list_rubric_items(
        &self,
        project_id: &str,
    ) -> Result<Vec<RubricQuestionSnapshot>, AppError> {
        let project = self.load_project(project_id)?;
        Ok(self.build_state_snapshot(&project).items)
    }

    pub fn update_question_rubric(
        &self,
        input: UpdateQuestionRubricInput,
    ) -> Result<Question, AppError> {
        let mut project = self.load_project(&input.project_id)?;
        ensure_question_text_ready(&project)?;
        if project.questions.is_empty() {
            return Err(AppError {
                code: AppErrorCode::RubricNotReady,
                message: "No questions are available for rubric preparation.".to_string(),
                recoverable: true,
                suggested_action: Some("Run question text extraction first.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        let now = chrono::Utc::now().to_rfc3339();
        let question = project
            .questions
            .iter_mut()
            .find(|question| question.id == input.question_id)
            .ok_or_else(|| AppError {
                code: AppErrorCode::RubricQuestionNotFound,
                message: "Question not found.".to_string(),
                recoverable: false,
                suggested_action: None,
                technical_details: Some(format!("question_id={}", input.question_id)),
                correlation_id: Uuid::new_v4().to_string(),
            })?;

        let state = RubricState {
            status: RubricStatus::Manual,
            source: Some(RubricSource::Manual),
            max_score: input.max_score,
            expected_answer: input
                .expected_answer
                .clone()
                .map(|text| normalize_text(&text)),
            criteria: input.criteria.clone(),
            partial_credit_hints: input.partial_credit_hints.clone(),
            zero_score_conditions: input.zero_score_conditions.clone(),
            common_mistakes: input.common_mistakes.clone(),
            warnings: vec![],
            updated_at: Some(now),
        };
        if let Some(answer_type) = input.answer_type {
            question.answer_type = answer_type;
        }
        let validation = validate_rubric_state(&state, Some(&question.answer_type));
        apply_validation_to_question(question, state, validation);

        let updated = question.clone();
        project.invalidate_exam_package_if_frozen("package_changed_after_freeze");
        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store.save_project(&project)?;
        Ok(updated)
    }

    pub fn confirm_question_rubric(
        &self,
        project_id: &str,
        question_id: &str,
    ) -> Result<Question, AppError> {
        let mut project = self.load_project(project_id)?;
        ensure_question_text_ready(&project)?;
        if project.questions.is_empty() {
            return Err(AppError {
                code: AppErrorCode::RubricNotReady,
                message: "No questions are available for rubric preparation.".to_string(),
                recoverable: true,
                suggested_action: Some("Run question text extraction first.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        let question = project
            .questions
            .iter_mut()
            .find(|question| question.id == question_id)
            .ok_or_else(|| AppError {
                code: AppErrorCode::RubricQuestionNotFound,
                message: "Question not found.".to_string(),
                recoverable: false,
                suggested_action: None,
                technical_details: Some(format!("question_id={question_id}")),
                correlation_id: Uuid::new_v4().to_string(),
            })?;

        let validation = validate_rubric_state(&question.rubric, Some(&question.answer_type));
        if !validation.valid || question.rubric.max_score.is_none() {
            return Err(build_rubric_confirm_error(question.number, &validation));
        }

        question.rubric.status = RubricStatus::Confirmed;
        question.rubric.source = question
            .rubric
            .source
            .clone()
            .or(Some(RubricSource::Manual));
        question.rubric.warnings = validation.warnings.clone();
        question.rubric.updated_at = Some(chrono::Utc::now().to_rfc3339());

        let updated = question.clone();
        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store.save_project(&project)?;
        Ok(updated)
    }

    pub fn confirm_all_rubrics(&self, project_id: &str) -> Result<Project, AppError> {
        let mut project = self.load_project(project_id)?;
        ensure_question_text_ready(&project)?;
        if project.questions.is_empty() {
            return Err(AppError {
                code: AppErrorCode::RubricNotReady,
                message: "No questions are available for rubric preparation.".to_string(),
                recoverable: true,
                suggested_action: Some("Run question text extraction first.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        let report = self.validate_project_rubrics(&project);
        if !report.valid || !report.confirmable {
            let details = report
                .questions
                .iter()
                .filter(|question| !question.valid || question.status != "confirmed")
                .map(|question| {
                    format!(
                        "Soru {}: {}",
                        question.number,
                        question
                            .issues
                            .iter()
                            .map(|issue| issue.message.clone())
                            .collect::<Vec<_>>()
                            .join("; ")
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
            return Err(AppError {
                code: AppErrorCode::RubricConfirmFailed,
                message: "Invalid rubrics cannot be confirmed.".to_string(),
                recoverable: true,
                suggested_action: Some("Fix invalid rubrics and try again.".to_string()),
                technical_details: Some(details),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        if let Some(existing_freeze) = project.exam_package_freeze.as_ref() {
            if existing_freeze.freeze_status == ExamPackageFreezeStatus::Frozen {
                let candidate = build_exam_package_freeze(
                    &project,
                    existing_freeze.exam_package_version,
                    existing_freeze.frozen_at.clone(),
                );
                if existing_freeze.source_hash == candidate.source_hash
                    && existing_freeze.rubric_hash == candidate.rubric_hash
                    && existing_freeze.question_text_hash == candidate.question_text_hash
                {
                    return Ok(project);
                }
                return Err(AppError {
                    code: AppErrorCode::WorkflowBlocked,
                    message: "Dondurulmuş sınav paketi otomatik olarak değiştirilemez.".to_string(),
                    recoverable: true,
                    suggested_action: Some(
                        "Önce soru veya rubrik değişikliğini açıkça kaydedip paketi geçersiz kılın."
                            .to_string(),
                    ),
                    technical_details: Some(
                        "frozen_package_content_changed_without_invalidation".to_string(),
                    ),
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
        }

        let now = chrono::Utc::now().to_rfc3339();
        for question in &mut project.questions {
            if matches!(
                question.rubric.status,
                RubricStatus::Imported | RubricStatus::Manual | RubricStatus::Suggested
            ) {
                question.rubric.status = RubricStatus::Confirmed;
                if question.rubric.source.is_none() {
                    question.rubric.source = Some(RubricSource::Manual);
                }
                question.rubric.updated_at = Some(now.clone());
            }
        }

        let previous_version = project
            .exam_package_freeze
            .as_ref()
            .map(|freeze| freeze.exam_package_version)
            .unwrap_or(0);
        project.exam_package_freeze = Some(build_exam_package_freeze(
            &project,
            previous_version.saturating_add(1),
            now,
        ));
        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store.save_project(&project)?;
        Ok(project)
    }

    pub fn validate_rubrics(&self, project_id: &str) -> Result<RubricValidationReport, AppError> {
        let project = self.load_project(project_id)?;
        Ok(self.validate_project_rubrics(&project))
    }

    fn validate_project_rubrics(&self, project: &Project) -> RubricValidationReport {
        let mut warnings = Vec::new();
        let mut blocking_questions = Vec::new();
        let mut valid = true;
        let mut confirmable = true;
        let mut questions = Vec::new();
        let expected_count = project
            .expected_question_count
            .unwrap_or(project.questions.len() as u32);

        for number in 1..=expected_count {
            if project
                .questions
                .iter()
                .all(|question| question.number != number)
            {
                valid = false;
                confirmable = false;
                blocking_questions.push(number);
                questions.push(RubricValidationQuestionSnapshot {
                    question_id: format!("missing-{number}"),
                    number,
                    status: "missing".to_string(),
                    valid: false,
                    warnings: vec![],
                    issues: vec![RubricValidationIssue {
                        code: "QUESTION_TEXT_MISSING".to_string(),
                        message: "Soru metni eksik.".to_string(),
                    }],
                    total_points: None,
                });
            }
        }

        for question in &project.questions {
            let validation = validate_rubric_state(&question.rubric, Some(&question.answer_type));
            let mut issues = validation.issues.clone();
            let question_text_ready = is_question_text_ready(&question.question_text);
            if !question_text_ready {
                issues.push(RubricValidationIssue {
                    code: "QUESTION_TEXT_MISSING".to_string(),
                    message: "Soru metni eksik.".to_string(),
                });
            }
            warnings.extend(validation.warnings.clone());
            let status = if question.rubric.status == RubricStatus::Imported && !validation.valid {
                RubricStatus::Invalid
            } else {
                question.rubric.status.clone()
            };
            if !validation.valid || !question_text_ready {
                valid = false;
                confirmable = false;
                blocking_questions.push(question.number);
            }
            if matches!(
                question.rubric.status,
                RubricStatus::Missing | RubricStatus::Invalid | RubricStatus::Legacy
            ) {
                confirmable = false;
                if !blocking_questions.contains(&question.number) {
                    blocking_questions.push(question.number);
                }
            }
            questions.push(RubricValidationQuestionSnapshot {
                question_id: question.id.clone(),
                number: question.number,
                status: crate::domain::rubric::rubric_status_label(&status).to_string(),
                valid: validation.valid && question_text_ready,
                warnings: validation.warnings,
                issues,
                total_points: validation.total_points,
            });
        }

        RubricValidationReport {
            project_id: project.id.clone(),
            valid,
            confirmable,
            warnings,
            blocking_questions,
            questions,
        }
    }

    fn build_state_snapshot(&self, project: &Project) -> RubricStateSnapshot {
        let mut items = Vec::new();
        let mut warnings = Vec::new();
        let mut missing_count = 0u32;
        let mut imported_count = 0u32;
        let mut manual_count = 0u32;
        let mut confirmed_count = 0u32;
        let mut invalid_count = 0u32;

        for question in &project.questions {
            let validation = validate_rubric_state(&question.rubric, Some(&question.answer_type));
            let mut effective_question = question.clone();
            let effective_status =
                if question.rubric.status == RubricStatus::Imported && !validation.valid {
                    RubricStatus::Invalid
                } else {
                    question.rubric.status.clone()
                };
            effective_question.rubric.status = effective_status.clone();
            warnings.extend(validation.warnings.clone());
            match effective_status {
                RubricStatus::Missing => missing_count += 1,
                RubricStatus::Imported => imported_count += 1,
                RubricStatus::Manual => manual_count += 1,
                RubricStatus::Confirmed => confirmed_count += 1,
                RubricStatus::Invalid | RubricStatus::Legacy => invalid_count += 1,
                RubricStatus::Suggested => {}
            }
            items.push(RubricQuestionSnapshot {
                question: effective_question,
                validation: RubricValidationSnapshot {
                    valid: validation.valid,
                    confirmable: validation.confirmable,
                    warnings: validation.warnings,
                    issues: validation.issues,
                    total_points: validation.total_points,
                },
            });
        }

        let summary = format!(
            "{} soru, {} onaylı, {} içe aktarıldı, {} manuel, {} geçersiz, {} eksik",
            project.questions.len(),
            confirmed_count,
            imported_count,
            manual_count,
            invalid_count,
            missing_count,
        );

        RubricStateSnapshot {
            project_id: project.id.clone(),
            current_stage: workflow_stage_key(
                &crate::services::workflow_engine::evaluate_workflow(project).current_stage,
            ),
            items,
            missing_count,
            imported_count,
            manual_count,
            confirmed_count,
            invalid_count,
            warnings,
            summary,
        }
    }

    fn load_project(&self, project_id: &str) -> Result<Project, AppError> {
        self.project_store
            .get_project_snapshot(project_id.to_string())
    }

    fn read_source_text(
        &self,
        project: &Project,
        document_id: Option<&str>,
        file_path: Option<&str>,
    ) -> Result<String, AppError> {
        let path = if let Some(file_path) = file_path {
            Path::new(file_path).to_path_buf()
        } else if let Some(document_id) = document_id {
            let document = project
                .documents
                .iter()
                .find(|document| {
                    document.id == document_id
                        && matches!(
                            document.role,
                            DocumentRole::AnswerKey | DocumentRole::Rubric
                        )
                })
                .ok_or_else(|| AppError {
                    code: AppErrorCode::DocumentNotFound,
                    message: "Rubric JSON document was not found.".to_string(),
                    recoverable: true,
                    suggested_action: Some("Select a valid rubric JSON file.".to_string()),
                    technical_details: Some(format!("document_id={document_id}")),
                    correlation_id: Uuid::new_v4().to_string(),
                })?;
            Path::new(&document.stored_path).to_path_buf()
        } else {
            return Err(AppError {
                code: AppErrorCode::RubricJsonInvalid,
                message: "A file path or document id is required.".to_string(),
                recoverable: true,
                suggested_action: Some("Select a rubric JSON file.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            });
        };

        std::fs::read_to_string(&path).map_err(|error| AppError {
            code: AppErrorCode::FileReadFailed,
            message: "Rubric JSON file could not be read.".to_string(),
            recoverable: true,
            suggested_action: Some("Check the file path and permissions.".to_string()),
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })
    }
}

fn apply_validation_to_question(
    question: &mut Question,
    mut rubric: RubricState,
    validation: crate::domain::rubric::RubricValidationResult,
) {
    rubric.warnings = validation.warnings;
    rubric.status = if validation.valid {
        rubric.status
    } else {
        RubricStatus::Invalid
    };
    question.rubric = rubric;
}

fn build_rubric_confirm_error(
    number: u32,
    validation: &crate::domain::rubric::RubricValidationResult,
) -> AppError {
    AppError {
        code: AppErrorCode::RubricConfirmFailed,
        message: format!("Soru {number} rubriği onaylanamadı."),
        recoverable: true,
        suggested_action: Some("Rubrik eksiklerini düzeltin ve tekrar deneyin.".to_string()),
        technical_details: Some(
            validation
                .issues
                .iter()
                .map(|issue| format!("{}: {}", issue.code, issue.message))
                .collect::<Vec<_>>()
                .join(" | "),
        ),
        correlation_id: Uuid::new_v4().to_string(),
    }
}

fn workflow_stage_key(stage: &crate::domain::workflow::WorkflowStage) -> String {
    serde_json::to_value(stage)
        .ok()
        .and_then(|value| value.as_str().map(|value| value.to_string()))
        .unwrap_or_default()
}

fn ensure_question_text_ready(project: &Project) -> Result<(), AppError> {
    if project.questions.is_empty()
        || project
            .questions
            .iter()
            .any(|question| !is_question_text_ready(&question.question_text))
    {
        return Err(AppError {
            code: AppErrorCode::RubricNotReady,
            message: "Question text must be confirmed before rubric preparation.".to_string(),
            recoverable: true,
            suggested_action: Some("Confirm or edit the question texts first.".to_string()),
            technical_details: None,
            correlation_id: Uuid::new_v4().to_string(),
        });
    }
    Ok(())
}

fn build_exam_package_freeze(
    project: &Project,
    version: u32,
    frozen_at: String,
) -> ExamPackageFreeze {
    let question_text_hash = hash_project_part(
        project
            .questions
            .iter()
            .map(|question| {
                format!(
                    "{}:{}:{:?}",
                    question.number, question.question_text.value, question.question_text.status
                )
            })
            .collect::<Vec<_>>(),
    );
    let rubric_hash = hash_project_part(
        project
            .questions
            .iter()
            .map(|question| {
                serde_json::to_string(&question.rubric).unwrap_or_else(|_| question.id.clone())
            })
            .collect::<Vec<_>>(),
    );

    ExamPackageFreeze {
        exam_package_version: version,
        freeze_status: ExamPackageFreezeStatus::Frozen,
        frozen_at,
        frozen_by: Some("teacher".to_string()),
        source_hash: hash_project_part(vec![
            project
                .expected_question_count
                .unwrap_or(project.questions.len() as u32)
                .to_string(),
            question_text_hash.clone(),
            rubric_hash.clone(),
        ]),
        rubric_hash,
        question_text_hash,
        invalidated_at: None,
        invalidation_reason: None,
    }
}

fn hash_project_part(values: Vec<String>) -> String {
    let mut hasher = DefaultHasher::new();
    values.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn parse_rubric_entries(text: &str) -> Result<Vec<NormalizedRubricEntry>, AppError> {
    let value: Value = serde_json::from_str(text).map_err(|error| AppError {
        code: AppErrorCode::RubricJsonInvalid,
        message: "Rubric JSON could not be parsed.".to_string(),
        recoverable: true,
        suggested_action: Some("Fix the JSON format and try again.".to_string()),
        technical_details: Some(error.to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    })?;

    if let Some(questions) = value.get("questions").and_then(|value| value.as_array()) {
        return questions.iter().map(parse_format_a_question).collect();
    }

    if let Some(rubric) = value.get("rubric").and_then(|value| value.as_array()) {
        return rubric.iter().map(parse_format_b_question).collect();
    }

    Err(AppError {
        code: AppErrorCode::RubricJsonSchemaUnsupported,
        message: "Unsupported rubric JSON schema.".to_string(),
        recoverable: true,
        suggested_action: Some("Use the supported questions or rubric array format.".to_string()),
        technical_details: Some("Expected top-level `questions` or `rubric` array.".to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    })
}

fn parse_format_a_question(value: &Value) -> Result<NormalizedRubricEntry, AppError> {
    let question_number = get_u32(value, &["number", "question_number"])?;
    let criteria = get_criteria(value, &["criteria"])?;
    Ok(NormalizedRubricEntry {
        question_number,
        max_score: get_optional_f32(value, &["max_score", "points"])?,
        expected_answer: get_optional_string(value, &["expected_answer", "answer_key"])?,
        criteria,
        partial_credit_hints: get_string_list(value, &["partial_credit_hints"])?,
        zero_score_conditions: get_string_list(value, &["zero_score_conditions"])?,
        common_mistakes: get_string_list(value, &["common_mistakes"])?,
        warnings: Vec::new(),
    })
}

fn parse_format_b_question(value: &Value) -> Result<NormalizedRubricEntry, AppError> {
    let question_number = get_u32(value, &["question_number", "number"])?;
    let criteria = get_criteria(value, &["rubric_items", "criteria"])?;
    Ok(NormalizedRubricEntry {
        question_number,
        max_score: get_optional_f32(value, &["points", "max_score"])?,
        expected_answer: get_optional_string(value, &["answer_key", "expected_answer"])?,
        criteria,
        partial_credit_hints: get_string_list(value, &["partial_credit_hints"])?,
        zero_score_conditions: get_string_list(value, &["zero_score_conditions"])?,
        common_mistakes: get_string_list(value, &["common_mistakes"])?,
        warnings: Vec::new(),
    })
}

fn get_u32(value: &Value, keys: &[&str]) -> Result<u32, AppError> {
    for key in keys {
        if let Some(raw) = value.get(key) {
            if let Some(number) = raw.as_u64() {
                return Ok(number as u32);
            }
        }
    }

    Err(AppError {
        code: AppErrorCode::RubricJsonInvalid,
        message: "Rubric question number is missing.".to_string(),
        recoverable: true,
        suggested_action: Some("Ensure each rubric item has a question number.".to_string()),
        technical_details: Some(format!("keys={:?}", keys)),
        correlation_id: Uuid::new_v4().to_string(),
    })
}

fn get_optional_f32(value: &Value, keys: &[&str]) -> Result<Option<f32>, AppError> {
    for key in keys {
        if let Some(raw) = value.get(key) {
            if raw.is_null() {
                return Ok(None);
            }
            if let Some(number) = raw.as_f64() {
                return Ok(Some(number as f32));
            }
            if let Some(text) = raw.as_str() {
                let parsed = text.trim().parse::<f32>().map_err(|error| AppError {
                    code: AppErrorCode::RubricJsonInvalid,
                    message: "Numeric rubric field could not be parsed.".to_string(),
                    recoverable: true,
                    suggested_action: Some(
                        "Use numeric values for max score and points.".to_string(),
                    ),
                    technical_details: Some(error.to_string()),
                    correlation_id: Uuid::new_v4().to_string(),
                })?;
                return Ok(Some(parsed));
            }
        }
    }
    Ok(None)
}

fn get_optional_string(value: &Value, keys: &[&str]) -> Result<Option<String>, AppError> {
    for key in keys {
        if let Some(raw) = value.get(key) {
            if raw.is_null() {
                return Ok(None);
            }
            if let Some(text) = raw.as_str() {
                let normalized = normalize_text(text);
                return Ok(if normalized.is_empty() {
                    None
                } else {
                    Some(normalized)
                });
            }
            return Err(AppError {
                code: AppErrorCode::RubricJsonInvalid,
                message: "Rubric text field must be a string.".to_string(),
                recoverable: true,
                suggested_action: Some("Use string values for rubric text fields.".to_string()),
                technical_details: Some(format!("keys={:?}", keys)),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
    }
    Ok(None)
}

fn get_string_list(value: &Value, keys: &[&str]) -> Result<Vec<String>, AppError> {
    for key in keys {
        if let Some(raw) = value.get(key) {
            let array = raw.as_array().ok_or_else(|| AppError {
                code: AppErrorCode::RubricJsonInvalid,
                message: "Rubric list field must be an array.".to_string(),
                recoverable: true,
                suggested_action: Some("Use arrays for rubric list fields.".to_string()),
                technical_details: Some(format!("keys={:?}", keys)),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
            return Ok(array
                .iter()
                .filter_map(|item| item.as_str())
                .map(normalize_text)
                .filter(|text| !text.is_empty())
                .collect());
        }
    }
    Ok(Vec::new())
}

fn get_criteria(value: &Value, keys: &[&str]) -> Result<Vec<RubricCriterion>, AppError> {
    for key in keys {
        if let Some(raw) = value.get(key) {
            let array = raw.as_array().ok_or_else(|| AppError {
                code: AppErrorCode::RubricJsonInvalid,
                message: "Rubric criteria field must be an array.".to_string(),
                recoverable: true,
                suggested_action: Some("Use arrays for rubric criteria.".to_string()),
                technical_details: Some(format!("keys={:?}", keys)),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
            return array
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let label = get_optional_string(item, &["label"])?
                        .unwrap_or_else(|| format!("Kriter {}", index + 1));
                    let description = get_optional_string(item, &["description"])?;
                    let description = description.unwrap_or_default();
                    let points = get_optional_f32(item, &["points"])?;
                    Ok(RubricCriterion {
                        id: get_optional_string(item, &["id"])?
                            .unwrap_or_else(|| Uuid::new_v4().to_string()),
                        label,
                        description,
                        points: points.unwrap_or(0.0),
                    })
                })
                .collect();
        }
    }
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::document::{Document, DocumentRole, PdfPreviewState, PdfPreviewStatus};
    use crate::domain::errors::AppErrorCode;
    use crate::domain::project::{ExamPackageFreezeStatus, Project};
    use crate::domain::question::{
        AnswerType, Question, TextFieldSource, TextFieldState, TextFieldStatus,
    };
    use crate::domain::rubric::{RubricCriterion, RubricSource, RubricState, RubricStatus};
    use crate::domain::workflow::WorkflowStage;
    use crate::services::project_store::ProjectStore;
    use crate::services::workflow_engine;

    fn temp_project_root() -> String {
        let root = std::env::temp_dir().join(format!("rubrika-v3-rubric-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp root");
        root.to_string_lossy().to_string()
    }

    fn confirmed_question(number: u32) -> Question {
        Question {
            id: Uuid::new_v4().to_string(),
            number,
            max_score: 10.0,
            answer_type: AnswerType::Essay,
            question_text: TextFieldState {
                value: format!("Question {number}"),
                source: TextFieldSource::Manual,
                status: TextFieldStatus::Confirmed,
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
        }
    }

    fn confirmed_project() -> (ProjectStore, Project) {
        let root = temp_project_root();
        let store = ProjectStore::new();
        let mut project = store
            .create_project("Project".to_string(), root.clone())
            .expect("project");
        project.documents.push(Document {
            id: Uuid::new_v4().to_string(),
            role: DocumentRole::ExamSource,
            file_name: "exam.pdf".to_string(),
            stored_path: format!("{root}/exam.pdf"),
            page_count: 1,
            added_at: "now".to_string(),
            checksum: None,
            preview: Some(PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: Some("now".to_string()),
                page_count: Some(1),
                job_id: None,
                error_message: None,
            }),
        });
        project.questions = vec![confirmed_question(1), confirmed_question(2)];
        project.workflow = workflow_engine::evaluate_workflow(&project);
        store.save_project(&project).expect("save");
        (store, project)
    }

    fn valid_format_a_json() -> String {
        serde_json::json!({
            "questions": [
                {
                    "number": 1,
                    "max_score": 10,
                    "expected_answer": "Cevap 1",
                    "criteria": [
                        {
                            "id": "c1",
                            "label": "Konuya uygunluk",
                            "description": "Yanıtın konuya uyumu",
                            "points": 4
                        },
                        {
                            "id": "c2",
                            "label": "Açıklık",
                            "description": "Cevabın açıklığı",
                            "points": 6
                        }
                    ],
                    "partial_credit_hints": ["Doğru adım varsa kısmi puan"],
                    "zero_score_conditions": ["Boş cevap"],
                    "common_mistakes": ["Konuyu karıştırma"]
                }
            ]
        })
        .to_string()
    }

    fn valid_format_b_json() -> String {
        serde_json::json!({
            "rubric": [
                {
                    "question_number": 1,
                    "points": 10,
                    "answer_key": "Cevap 1",
                    "rubric_items": [
                        {
                            "label": "Konu",
                            "description": "Yanıtın konuya uyumu",
                            "points": 10
                        }
                    ]
                }
            ]
        })
        .to_string()
    }

    #[test]
    fn imports_valid_format_a_json_and_maps_numbers() {
        let (store, project) = confirmed_project();
        let service = RubricService::new(store.clone());
        let json_path =
            std::env::temp_dir().join(format!("rubrika-v3-format-a-{}.json", Uuid::new_v4()));
        std::fs::write(&json_path, valid_format_a_json()).expect("write json");

        let result = service
            .import_rubric_json(ImportRubricJsonInput {
                project_id: project.id.clone(),
                document_id: None,
                file_path: Some(json_path.to_string_lossy().to_string()),
            })
            .expect("import");

        assert_eq!(result.imported_count, 1);
        assert_eq!(result.missing_count, 1);
        assert_eq!(result.invalid_count, 0);

        let updated = store
            .get_project_snapshot(project.id.clone())
            .expect("snapshot");
        assert_eq!(updated.questions[0].rubric.status, RubricStatus::Imported);
        assert_eq!(updated.questions[0].rubric.max_score, Some(10.0));
        assert_eq!(updated.questions[0].rubric.criteria.len(), 2);
        assert_eq!(updated.questions[1].rubric.status, RubricStatus::Missing);
    }

    #[test]
    fn imports_valid_format_b_json() {
        let (store, project) = confirmed_project();
        let service = RubricService::new(store.clone());
        let json_path =
            std::env::temp_dir().join(format!("rubrika-v3-format-b-{}.json", Uuid::new_v4()));
        std::fs::write(&json_path, valid_format_b_json()).expect("write json");

        let result = service
            .import_rubric_json(ImportRubricJsonInput {
                project_id: project.id.clone(),
                document_id: None,
                file_path: Some(json_path.to_string_lossy().to_string()),
            })
            .expect("import");

        assert_eq!(result.imported_count, 1);
        let updated = store
            .get_project_snapshot(project.id.clone())
            .expect("snapshot");
        assert_eq!(
            updated.questions[0].rubric.expected_answer.as_deref(),
            Some("Cevap 1")
        );
        assert_eq!(updated.questions[0].rubric.criteria[0].label, "Konu");
    }

    #[test]
    fn unknown_question_number_reports_warning() {
        let (store, project) = confirmed_project();
        let service = RubricService::new(store.clone());
        let json_path =
            std::env::temp_dir().join(format!("rubrika-v3-unknown-{}.json", Uuid::new_v4()));
        std::fs::write(
            &json_path,
            serde_json::json!({
                "questions": [
                    {
                        "number": 99,
                        "max_score": 10,
                        "expected_answer": "Cevap",
                        "criteria": []
                    }
                ]
            })
            .to_string(),
        )
        .expect("write json");

        let result = service
            .import_rubric_json(ImportRubricJsonInput {
                project_id: project.id.clone(),
                document_id: None,
                file_path: Some(json_path.to_string_lossy().to_string()),
            })
            .expect("import");

        assert_eq!(result.imported_count, 0);
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.contains("bulunamadı")));
    }

    #[test]
    fn placeholder_expected_answer_marks_invalid() {
        let rubric = RubricState {
            status: RubricStatus::Imported,
            source: Some(RubricSource::Json),
            max_score: Some(10.0),
            expected_answer: Some("örnek cevap".to_string()),
            criteria: vec![RubricCriterion {
                id: "c1".to_string(),
                label: "Kriter".to_string(),
                description: "Açıklama".to_string(),
                points: 10.0,
            }],
            partial_credit_hints: vec![],
            zero_score_conditions: vec![],
            common_mistakes: vec![],
            warnings: vec![],
            updated_at: None,
        };
        let validation = validate_rubric_state(&rubric, Some(&AnswerType::Essay));
        assert!(!validation.valid);
        assert!(validation
            .issues
            .iter()
            .any(|issue| issue.code == "RUBRIC_PLACEHOLDER_DETECTED"));
    }

    #[test]
    fn criteria_mismatch_blocks_confirmation() {
        let rubric = RubricState {
            status: RubricStatus::Imported,
            source: Some(RubricSource::Json),
            max_score: Some(10.0),
            expected_answer: Some("Cevap".to_string()),
            criteria: vec![RubricCriterion {
                id: "c1".to_string(),
                label: "Kriter".to_string(),
                description: "Açıklama".to_string(),
                points: 8.0,
            }],
            partial_credit_hints: vec![],
            zero_score_conditions: vec![],
            common_mistakes: vec![],
            warnings: vec![],
            updated_at: None,
        };
        let validation = validate_rubric_state(&rubric, Some(&AnswerType::Essay));
        assert!(!validation.valid);
        assert!(validation
            .issues
            .iter()
            .any(|issue| issue.code == "RUBRIC_POINTS_TOTAL_MISMATCH"));
    }

    #[test]
    fn missing_max_score_cannot_be_confirmed() {
        let rubric = RubricState {
            status: RubricStatus::Imported,
            source: Some(RubricSource::Json),
            max_score: None,
            expected_answer: Some("Cevap".to_string()),
            criteria: vec![RubricCriterion {
                id: "c1".to_string(),
                label: "Kriter".to_string(),
                description: "Açıklama".to_string(),
                points: 10.0,
            }],
            partial_credit_hints: vec![],
            zero_score_conditions: vec![],
            common_mistakes: vec![],
            warnings: vec![],
            updated_at: None,
        };
        let validation = validate_rubric_state(&rubric, Some(&AnswerType::Essay));
        assert!(!validation.valid);
        assert!(validation
            .issues
            .iter()
            .any(|issue| issue.code == "RUBRIC_MAX_SCORE_MISSING"));
    }

    #[test]
    fn confirm_all_blocks_invalid_rubrics() {
        let (store, project) = confirmed_project();
        let service = RubricService::new(store.clone());

        let mut snapshot = store
            .get_project_snapshot(project.id.clone())
            .expect("snapshot");
        snapshot.questions[0].rubric = RubricState {
            status: RubricStatus::Imported,
            source: Some(RubricSource::Json),
            max_score: Some(10.0),
            expected_answer: Some("Cevap".to_string()),
            criteria: vec![RubricCriterion {
                id: "c1".to_string(),
                label: "Kriter".to_string(),
                description: "Açıklama".to_string(),
                points: 8.0,
            }],
            partial_credit_hints: vec![],
            zero_score_conditions: vec![],
            common_mistakes: vec![],
            warnings: vec![],
            updated_at: None,
        };
        snapshot.workflow = workflow_engine::evaluate_workflow(&snapshot);
        store.save_project(&snapshot).expect("save");

        let error = service
            .confirm_all_rubrics(&project.id)
            .expect_err("invalid rubric should block");
        assert_eq!(error.code, AppErrorCode::RubricConfirmFailed);
    }

    #[test]
    fn project_save_reload_persists_rubric() {
        let (store, project) = confirmed_project();
        let service = RubricService::new(store.clone());
        let json_path =
            std::env::temp_dir().join(format!("rubrika-v3-persist-{}.json", Uuid::new_v4()));
        std::fs::write(&json_path, valid_format_a_json()).expect("write json");

        service
            .import_rubric_json(ImportRubricJsonInput {
                project_id: project.id.clone(),
                document_id: None,
                file_path: Some(json_path.to_string_lossy().to_string()),
            })
            .expect("import");

        let reopened = store.open_project(project.root_path.clone()).expect("open");
        assert_eq!(reopened.questions[0].rubric.status, RubricStatus::Imported);
        assert_eq!(reopened.questions[0].rubric.max_score, Some(10.0));
    }

    #[test]
    fn confirm_all_moves_workflow_to_rubric_confirmed() {
        let (store, project) = confirmed_project();
        let service = RubricService::new(store.clone());
        let json_path =
            std::env::temp_dir().join(format!("rubrika-v3-confirm-{}.json", Uuid::new_v4()));
        std::fs::write(&json_path, valid_format_a_json()).expect("write json");

        service
            .import_rubric_json(ImportRubricJsonInput {
                project_id: project.id.clone(),
                document_id: None,
                file_path: Some(json_path.to_string_lossy().to_string()),
            })
            .expect("import");

        let snapshot = store
            .get_project_snapshot(project.id.clone())
            .expect("snapshot");
        service
            .update_question_rubric(UpdateQuestionRubricInput {
                project_id: project.id.clone(),
                question_id: snapshot.questions[1].id.clone(),
                answer_type: None,
                max_score: Some(10.0),
                expected_answer: Some("Cevap 2".to_string()),
                criteria: vec![RubricCriterion {
                    id: "c2".to_string(),
                    label: "Kriter".to_string(),
                    description: "Açıklama".to_string(),
                    points: 10.0,
                }],
                partial_credit_hints: vec![],
                zero_score_conditions: vec![],
                common_mistakes: vec![],
            })
            .expect("update second question");

        let updated = service
            .confirm_all_rubrics(&project.id)
            .expect("confirm all");
        assert_eq!(
            updated.workflow.current_stage,
            WorkflowStage::StudentScansMissing
        );
        let freeze = updated.exam_package_freeze.expect("freeze metadata");
        assert_eq!(freeze.freeze_status, ExamPackageFreezeStatus::Frozen);
        assert_eq!(freeze.exam_package_version, 1);
        assert!(!freeze.rubric_hash.is_empty());
        assert!(!freeze.question_text_hash.is_empty());
    }

    #[test]
    fn confirm_all_allows_empty_optional_guidance_fields() {
        let (store, project) = confirmed_project();
        let service = RubricService::new(store.clone());
        let snapshot = store
            .get_project_snapshot(project.id.clone())
            .expect("snapshot");

        for question in snapshot.questions {
            service
                .update_question_rubric(UpdateQuestionRubricInput {
                    project_id: project.id.clone(),
                    question_id: question.id,
                    answer_type: None,
                    max_score: Some(10.0),
                    expected_answer: Some(format!("Cevap {}", question.number)),
                    criteria: vec![RubricCriterion {
                        id: format!("c{}", question.number),
                        label: "Kriter".to_string(),
                        description: "Açıklama".to_string(),
                        points: 10.0,
                    }],
                    partial_credit_hints: vec![],
                    zero_score_conditions: vec![],
                    common_mistakes: vec![],
                })
                .expect("update question");
        }

        let updated = service
            .confirm_all_rubrics(&project.id)
            .expect("empty optional guidance must not block");
        assert!(updated
            .questions
            .iter()
            .all(|question| question.rubric.status == RubricStatus::Confirmed));
    }

    #[test]
    fn confirm_all_is_idempotent_for_an_unchanged_frozen_package() {
        let (store, project) = confirmed_project();
        let service = RubricService::new(store.clone());
        let snapshot = store
            .get_project_snapshot(project.id.clone())
            .expect("snapshot");

        for question in snapshot.questions {
            service
                .update_question_rubric(UpdateQuestionRubricInput {
                    project_id: project.id.clone(),
                    question_id: question.id,
                    answer_type: None,
                    max_score: Some(10.0),
                    expected_answer: Some(format!("Cevap {}", question.number)),
                    criteria: vec![RubricCriterion {
                        id: format!("c{}", question.number),
                        label: "Kriter".to_string(),
                        description: "Açıklama".to_string(),
                        points: 10.0,
                    }],
                    partial_credit_hints: vec![],
                    zero_score_conditions: vec![],
                    common_mistakes: vec![],
                })
                .expect("update question");
        }

        let first = service
            .confirm_all_rubrics(&project.id)
            .expect("first freeze")
            .exam_package_freeze
            .expect("first freeze metadata");
        let second = service
            .confirm_all_rubrics(&project.id)
            .expect("idempotent freeze")
            .exam_package_freeze
            .expect("second freeze metadata");

        assert_eq!(second.exam_package_version, first.exam_package_version);
        assert_eq!(second.frozen_at, first.frozen_at);
        assert_eq!(second.source_hash, first.source_hash);
    }

    #[test]
    fn rubric_update_after_freeze_invalidates_package() {
        let (store, project) = confirmed_project();
        let service = RubricService::new(store.clone());
        let snapshot = store
            .get_project_snapshot(project.id.clone())
            .expect("snapshot");

        for question in snapshot.questions {
            service
                .update_question_rubric(UpdateQuestionRubricInput {
                    project_id: project.id.clone(),
                    question_id: question.id,
                    answer_type: None,
                    max_score: Some(10.0),
                    expected_answer: Some(format!("Cevap {}", question.number)),
                    criteria: vec![RubricCriterion {
                        id: format!("c{}", question.number),
                        label: "Kriter".to_string(),
                        description: "Açıklama".to_string(),
                        points: 10.0,
                    }],
                    partial_credit_hints: vec![],
                    zero_score_conditions: vec![],
                    common_mistakes: vec![],
                })
                .expect("update question");
        }
        let frozen = service
            .confirm_all_rubrics(&project.id)
            .expect("confirm all");
        let question_id = frozen.questions[0].id.clone();

        service
            .update_question_rubric(UpdateQuestionRubricInput {
                project_id: project.id.clone(),
                question_id,
                answer_type: None,
                max_score: Some(10.0),
                expected_answer: Some("Değişen cevap".to_string()),
                criteria: vec![RubricCriterion {
                    id: "c1".to_string(),
                    label: "Kriter".to_string(),
                    description: "Açıklama".to_string(),
                    points: 10.0,
                }],
                partial_credit_hints: vec![],
                zero_score_conditions: vec![],
                common_mistakes: vec![],
            })
            .expect("update after freeze");

        let updated = store
            .get_project_snapshot(project.id.clone())
            .expect("snapshot");
        let freeze = updated.exam_package_freeze.expect("freeze metadata");
        assert_eq!(freeze.freeze_status, ExamPackageFreezeStatus::Invalidated);
        assert_eq!(
            freeze.invalidation_reason.as_deref(),
            Some("package_changed_after_freeze")
        );
    }

    #[test]
    fn rubric_update_persists_teacher_selected_answer_type() {
        let (store, project) = confirmed_project();
        let service = RubricService::new(store.clone());
        let question_id = project.questions[0].id.clone();

        let updated = service
            .update_question_rubric(UpdateQuestionRubricInput {
                project_id: project.id.clone(),
                question_id: question_id.clone(),
                answer_type: Some(AnswerType::Matching),
                max_score: Some(10.0),
                expected_answer: Some("A-2, B-1".to_string()),
                criteria: vec![RubricCriterion {
                    id: "matching".to_string(),
                    label: "Eşler".to_string(),
                    description: "Doğru eşleri kurar.".to_string(),
                    points: 10.0,
                }],
                partial_credit_hints: vec![],
                zero_score_conditions: vec![],
                common_mistakes: vec![],
            })
            .expect("update answer type");

        assert_eq!(updated.answer_type, AnswerType::Matching);
        let persisted = store
            .get_project_snapshot(project.id)
            .expect("persisted project");
        assert_eq!(persisted.questions[0].answer_type, AnswerType::Matching);
    }
}

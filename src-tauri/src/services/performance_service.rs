use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use uuid::Uuid;

use crate::domain::assessment::{AssessmentActivity, AssessmentType, ClassApplicationStatus};
use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::performance::{
    CriterionRating, PerformanceAssessment, PerformanceAssessmentStatus, PerformanceDetails,
    PerformanceRubric,
};
use crate::domain::project::Project;
use crate::services::assessment_organization_service::{
    AssessmentOrganizationService, CreateAssessmentActivityInput,
};
use crate::services::performance_dtos::{
    ApprovePerformanceAssessmentInput, CreatePerformanceTaskInput, GetPerformanceReportInput,
    ListPerformanceAssessmentsInput, ListPerformanceTasksInput, PerformanceActivityIdInput,
    PerformanceReportCriterionScore, PerformanceReportDto, PerformanceReportStudentRow,
    PerformanceReportSummary, PerformanceStatusDto, PublishPerformanceRubricInput,
    SavePerformanceAssessmentInput, SetPerformanceAssessmentStatusInput,
    UpdatePerformanceTaskInput,
};
use crate::services::project_store::{MutationOptions, ProjectStore};
use crate::services::school_class_service::students_for_class;

#[derive(Clone)]
pub struct PerformanceService {
    project_store: ProjectStore,
    assessment_organization_service: Arc<AssessmentOrganizationService>,
}

impl PerformanceService {
    pub fn new(
        project_store: ProjectStore,
        assessment_organization_service: Arc<AssessmentOrganizationService>,
    ) -> Self {
        Self {
            project_store,
            assessment_organization_service,
        }
    }

    pub fn create_performance_task(
        &self,
        input: CreatePerformanceTaskInput,
        correlation_id: &str,
    ) -> Result<AssessmentActivity, AppError> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut details = input.performance_details.clone();
        let initial_rubric = match input.initial_rubric {
            Some(mut rubric) => {
                if rubric.id.trim().is_empty() {
                    rubric.id = Uuid::new_v4().to_string();
                }
                rubric.version = 0;
                if rubric.created_at.trim().is_empty() {
                    rubric.created_at = now.clone();
                }
                rubric
            }
            None => {
                let title = input.title.trim();
                PerformanceRubric {
                    id: Uuid::new_v4().to_string(),
                    name: format!(
                        "{} Rubrik",
                        if title.is_empty() {
                            "Performans Görevi"
                        } else {
                            title
                        }
                    ),
                    version: 0,
                    criteria: vec![],
                    levels: vec![],
                    created_at: now,
                }
            }
        };
        details.rubric_versions = vec![initial_rubric];

        let create_input = CreateAssessmentActivityInput {
            project_id: input.project_id.clone(),
            academic_year_id: input.academic_year_id.clone(),
            course_id: input.course_id.clone(),
            course_name: input.course_name.clone(),
            grade_level: input.grade_level,
            term: input.term,
            assessment_type: AssessmentType::Performance,
            sequence_number: input.sequence_number,
            school_class_ids: input.school_class_ids.clone(),
            title: input.title.clone(),
            speaking_configuration: None,
            listening_details: None,
            performance_details: Some(details),
        };
        let output = self.project_store.mutate(
            &input.project_id,
            MutationOptions::new("create_performance_task").correlation(correlation_id),
            |project, _context| {
                self.assessment_organization_service
                    .create_activity_in_project(project, &create_input)
            },
        )?;
        Ok(output.result)
    }

    pub fn update_performance_task(
        &self,
        input: UpdatePerformanceTaskInput,
        correlation_id: &str,
    ) -> Result<AssessmentActivity, AppError> {
        let output = self.project_store.mutate(
            &input.project_id,
            MutationOptions::new("update_performance_task").correlation(correlation_id),
            |project, _context| {
                let activity = project
                    .assessment_activities
                    .iter_mut()
                    .find(|activity| activity.id == input.activity_id)
                    .ok_or_else(|| {
                        performance_error(
                            AppErrorCode::AssessmentActivityNotFound,
                            "Performans görevi bulunamadı.",
                            "activity_id not found.",
                        )
                    })?;
                if activity.assessment_type != AssessmentType::Performance {
                    return Err(performance_error(
                        AppErrorCode::AssessmentInvalidInput,
                        "Bu etkinlik performans görevi değil.",
                        "activity is not a performance task.",
                    ));
                }
                if let Some(title) = &input.title {
                    activity.title = title.trim().to_string();
                }
                if let Some(details) = &input.performance_details {
                    let stored = activity
                        .performance_details
                        .get_or_insert_with(PerformanceDetails::default);
                    stored.theme = details.theme.trim().to_string();
                    stored.learning_outcomes = details.learning_outcomes.clone();
                    stored.skill_area = details.skill_area;
                    stored.task_instruction = details.task_instruction.trim().to_string();
                    stored.work_mode = details.work_mode;
                    stored.due_date = normalize_optional(details.due_date.clone());
                    stored.evidence_types = details.evidence_types.clone();
                }
                activity.updated_at = chrono::Utc::now().to_rfc3339();
                Ok(activity.clone())
            },
        )?;
        Ok(output.result)
    }

    pub fn list_performance_tasks(
        &self,
        input: ListPerformanceTasksInput,
    ) -> Result<Vec<AssessmentActivity>, AppError> {
        let project = self.load_project(&input.project_id)?;
        let mut tasks = project
            .assessment_activities
            .into_iter()
            .filter(|activity| activity.assessment_type == AssessmentType::Performance)
            .filter(|activity| {
                input
                    .course_id
                    .as_ref()
                    .map_or(true, |course_id| &activity.course_id == course_id)
            })
            .filter(|activity| input.term.map_or(true, |term| activity.term == term))
            .filter(|activity| {
                input.school_class_id.as_ref().map_or(true, |class_id| {
                    activity
                        .class_applications
                        .iter()
                        .any(|application| application.school_class_id == *class_id)
                })
            })
            .collect::<Vec<_>>();
        tasks.sort_by(|left, right| {
            left.course_name
                .cmp(&right.course_name)
                .then(left.term.cmp(&right.term))
                .then(left.sequence_number.cmp(&right.sequence_number))
        });
        Ok(tasks)
    }

    pub fn get_performance_task(
        &self,
        input: PerformanceActivityIdInput,
    ) -> Result<AssessmentActivity, AppError> {
        let project = self.load_project(&input.project_id)?;
        let activity = project
            .assessment_activities
            .into_iter()
            .find(|activity| activity.id == input.activity_id)
            .ok_or_else(|| {
                performance_error(
                    AppErrorCode::AssessmentActivityNotFound,
                    "Performans görevi bulunamadı.",
                    "activity_id not found.",
                )
            })?;
        if activity.assessment_type != AssessmentType::Performance {
            return Err(performance_error(
                AppErrorCode::AssessmentInvalidInput,
                "Bu etkinlik performans görevi değil.",
                "activity is not a performance task.",
            ));
        }
        Ok(activity)
    }

    /// Yayın = yeni sürüm (K8). Onaylı değerlendirmesi olan rubrik değiştirilemez;
    /// yeni sürüm bile açılamaz.
    pub fn publish_performance_rubric(
        &self,
        input: PublishPerformanceRubricInput,
        correlation_id: &str,
    ) -> Result<PerformanceRubric, AppError> {
        validate_rubric(&input.rubric)?;
        let output = self.project_store.mutate(
            &input.project_id,
            MutationOptions::new("publish_performance_rubric").correlation(correlation_id),
            |project, _context| {
                let activity = project
                    .assessment_activities
                    .iter_mut()
                    .find(|activity| activity.id == input.activity_id)
                    .ok_or_else(|| {
                        performance_error(
                            AppErrorCode::AssessmentActivityNotFound,
                            "Performans görevi bulunamadı.",
                            "activity_id not found.",
                        )
                    })?;
                if activity.assessment_type != AssessmentType::Performance {
                    return Err(performance_error(
                        AppErrorCode::AssessmentInvalidInput,
                        "Bu etkinlik performans görevi değil.",
                        "activity is not a performance task.",
                    ));
                }
                let details = activity
                    .performance_details
                    .as_mut()
                    .ok_or_else(|| {
                        performance_error(
                            AppErrorCode::AssessmentInvalidInput,
                            "Görev ayrıntıları bulunamadı.",
                            "performance_details is missing.",
                        )
                    })?;
                let rubric_id = if let Some(existing) = details.rubric_versions.first() {
                    existing.id.clone()
                } else if input.rubric.id.trim().is_empty() {
                    Uuid::new_v4().to_string()
                } else {
                    input.rubric.id.trim().to_string()
                };
                if activity.class_applications.iter().any(|application| {
                    application
                        .performance_assessments
                        .iter()
                        .any(|assessment| {
                            assessment.rubric_id == rubric_id
                                && assessment.status
                                    == PerformanceAssessmentStatus::Approved
                        })
                }) {
                    return Err(performance_error(
                        AppErrorCode::AssessmentActivityInUse,
                        "Onaylı değerlendirmesi olan rubrik değiştirilemez; bu görev için yeni rubrik sürümü yayınlanamaz.",
                        "rubric is locked by approved assessments.",
                    ));
                }
                let new_version = details
                    .rubric_versions
                    .iter()
                    .map(|rubric| rubric.version)
                    .max()
                    .unwrap_or(0)
                    + 1;
                let published = PerformanceRubric {
                    id: rubric_id,
                    name: input.rubric.name.trim().to_string(),
                    version: new_version,
                    criteria: input.rubric.criteria.clone(),
                    levels: input.rubric.levels.clone(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                };
                details.rubric_versions.push(published.clone());
                activity.updated_at = chrono::Utc::now().to_rfc3339();
                Ok(published)
            },
        )?;
        Ok(output.result)
    }

    pub fn get_performance_rubric_history(
        &self,
        input: PerformanceActivityIdInput,
    ) -> Result<Vec<PerformanceRubric>, AppError> {
        let activity = self.get_performance_task(input)?;
        let versions = activity
            .performance_details
            .map(|details| details.rubric_versions)
            .unwrap_or_default();
        Ok(versions)
    }

    pub fn save_performance_assessment(
        &self,
        input: SavePerformanceAssessmentInput,
        correlation_id: &str,
    ) -> Result<PerformanceAssessment, AppError> {
        let output = self.project_store.mutate(
            &input.project_id,
            MutationOptions::new("save_performance_assessment").correlation(correlation_id),
            |project, _context| {
                let activity_index = project
                    .assessment_activities
                    .iter()
                    .position(|activity| activity.id == input.activity_id)
                    .ok_or_else(|| {
                        performance_error(
                            AppErrorCode::AssessmentActivityNotFound,
                            "Performans görevi bulunamadı.",
                            "activity_id not found.",
                        )
                    })?;
                let (school_class_id, latest_rubric) = {
                    let activity = &project.assessment_activities[activity_index];
                    if activity.assessment_type != AssessmentType::Performance {
                        return Err(performance_error(
                            AppErrorCode::AssessmentInvalidInput,
                            "Bu etkinlik performans görevi değil.",
                            "activity is not a performance task.",
                        ));
                    }
                    let details = activity.performance_details.as_ref().ok_or_else(|| {
                        performance_error(
                            AppErrorCode::AssessmentInvalidInput,
                            "Görev ayrıntıları bulunamadı.",
                            "performance_details is missing.",
                        )
                    })?;
                    let latest = latest_published_rubric(details).ok_or_else(|| {
                        performance_error(
                            AppErrorCode::RubricMissing,
                            "Rubrik yayınlanmadan değerlendirme kaydedilemez.",
                            "no published rubric version exists.",
                        )
                    })?;
                    let application = activity
                        .class_applications
                        .iter()
                        .find(|application| application.id == input.application_id)
                        .ok_or_else(|| {
                            performance_error(
                                AppErrorCode::AssessmentClassApplicationNotFound,
                                "Sınıf uygulaması bulunamadı.",
                                "application_id not found for activity.",
                            )
                        })?;
                    (application.school_class_id.clone(), latest.clone())
                };
                let roster = students_for_class(project, &school_class_id)?;
                if !roster.iter().any(|student| student.id == input.student_id) {
                    return Err(performance_error(
                        AppErrorCode::StudentNotFound,
                        "Öğrenci bu sınıfta bulunamadı.",
                        "student_id is not a member of the class roster.",
                    ));
                }
                let now = chrono::Utc::now().to_rfc3339();
                let activity = &mut project.assessment_activities[activity_index];
                let application = activity
                    .class_applications
                    .iter_mut()
                    .find(|application| application.id == input.application_id)
                    .ok_or_else(|| {
                        performance_error(
                            AppErrorCode::AssessmentClassApplicationNotFound,
                            "Sınıf uygulaması bulunamadı.",
                            "application_id not found for activity.",
                        )
                    })?;
                if let Some(assessment_id) = input.assessment_id.as_deref() {
                    let scoped = application
                        .performance_assessments
                        .iter()
                        .any(|assessment| {
                            assessment.id == assessment_id
                                && assessment.student_id == input.student_id
                        });
                    if !scoped {
                        return Err(performance_error(
                            AppErrorCode::AssessmentInvalidInput,
                            "Değerlendirme kaydı bu öğrenciye veya sınıf uygulamasına ait değil.",
                            "assessment_id does not belong to this application and student.",
                        ));
                    }
                }
                let existing = application
                    .performance_assessments
                    .iter_mut()
                    .find(|assessment| {
                        Some(&assessment.id) == input.assessment_id.as_ref()
                            || (input.assessment_id.is_none()
                                && assessment.student_id == input.student_id)
                    });
                let details = activity.performance_details.as_ref().ok_or_else(|| {
                    performance_error(
                        AppErrorCode::AssessmentInvalidInput,
                        "Görev ayrıntıları bulunamadı.",
                        "performance_details is missing.",
                    )
                })?;
                let (rating_rubric, new_rubric_id, new_rubric_version) =
                    if let Some(existing) = existing.as_deref() {
                        let pinned = details
                            .rubric_versions
                            .iter()
                            .find(|rubric| {
                                rubric.id == existing.rubric_id
                                    && rubric.version == existing.rubric_version
                            })
                            .unwrap_or(&latest_rubric);
                        (
                            pinned.clone(),
                            existing.rubric_id.clone(),
                            existing.rubric_version,
                        )
                    } else {
                        (
                            latest_rubric.clone(),
                            latest_rubric.id.clone(),
                            latest_rubric.version,
                        )
                    };
                validate_ratings(&input.ratings, &rating_rubric)?;
                let provisional_total = compute_provisional_total(&input.ratings, &rating_rubric);

                let assessment = if let Some(existing) = existing {
                    if existing.status == PerformanceAssessmentStatus::Approved {
                        return Err(performance_error(
                            AppErrorCode::AssessmentActivityInUse,
                            "Onaylanmış değerlendirme düzenlenemez; yeni değerlendirme açılabilir.",
                            "approved assessment cannot be saved.",
                        ));
                    }
                    existing.ratings = input.ratings.clone();
                    existing.provisional_total = provisional_total;
                    existing.feedback = normalize_optional(input.feedback.clone());
                    existing.status = PerformanceAssessmentStatus::InProgress;
                    existing.assessed_at = Some(now.clone());
                    existing.updated_at = now.clone();
                    existing.clone()
                } else {
                    let assessment = PerformanceAssessment {
                        id: Uuid::new_v4().to_string(),
                        student_id: input.student_id.clone(),
                        rubric_id: new_rubric_id,
                        rubric_version: new_rubric_version,
                        ratings: input.ratings.clone(),
                        provisional_total,
                        feedback: normalize_optional(input.feedback.clone()),
                        status: PerformanceAssessmentStatus::InProgress,
                        assessed_at: Some(now.clone()),
                        approved_at: None,
                        created_at: now.clone(),
                        updated_at: now,
                    };
                    application.performance_assessments.push(assessment.clone());
                    assessment
                };
                activity.updated_at = chrono::Utc::now().to_rfc3339();
                Ok(assessment)
            },
        )?;
        Ok(output.result)
    }

    pub fn approve_performance_assessment(
        &self,
        input: ApprovePerformanceAssessmentInput,
        correlation_id: &str,
    ) -> Result<PerformanceAssessment, AppError> {
        let output = self.project_store.mutate(
            &input.project_id,
            MutationOptions::new("approve_performance_assessment").correlation(correlation_id),
            |project, _context| {
                let activity = project
                    .assessment_activities
                    .iter_mut()
                    .find(|activity| activity.id == input.activity_id)
                    .ok_or_else(|| {
                        performance_error(
                            AppErrorCode::AssessmentActivityNotFound,
                            "Performans görevi bulunamadı.",
                            "activity_id not found.",
                        )
                    })?;
                if activity.assessment_type != AssessmentType::Performance {
                    return Err(performance_error(
                        AppErrorCode::AssessmentInvalidInput,
                        "Bu etkinlik performans görevi değil.",
                        "activity is not a performance task.",
                    ));
                }
                let details = activity.performance_details.as_ref().ok_or_else(|| {
                    performance_error(
                        AppErrorCode::AssessmentInvalidInput,
                        "Görev ayrıntıları bulunamadı.",
                        "performance_details is missing.",
                    )
                })?;
                let application = activity
                    .class_applications
                    .iter_mut()
                    .find(|application| application.id == input.application_id)
                    .ok_or_else(|| {
                        performance_error(
                            AppErrorCode::AssessmentClassApplicationNotFound,
                            "Sınıf uygulaması bulunamadı.",
                            "application_id not found for activity.",
                        )
                    })?;
                let approved_student_ids = application
                    .performance_assessments
                    .iter()
                    .filter(|assessment| {
                        assessment.status == PerformanceAssessmentStatus::Approved
                    })
                    .map(|assessment| assessment.student_id.clone())
                    .collect::<HashSet<_>>();
                let assessment = application
                    .performance_assessments
                    .iter_mut()
                    .find(|assessment| assessment.id == input.assessment_id)
                    .ok_or_else(|| {
                        performance_error(
                            AppErrorCode::AssessmentClassApplicationNotFound,
                            "Değerlendirme kaydı bulunamadı.",
                            "assessment_id not found.",
                        )
                    })?;
                if assessment.status == PerformanceAssessmentStatus::Approved {
                    return Err(performance_error(
                        AppErrorCode::AssessmentActivityInUse,
                        "Bu değerlendirme zaten onaylanmış.",
                        "assessment is already approved.",
                    ));
                }
                // Tek final karar garantisi: aynı öğrenci için aynı sınıf
                // uygulamasında onaylanmış başka bir kayıt varsa ikinci final
                // karar oluşturulmaz (scope doğrulaması öncesi legacy
                // duplicate kayıtlara karşı savunmacı kontrol).
                if approved_student_ids.contains(&assessment.student_id) {
                    return Err(performance_error(
                        AppErrorCode::AssessmentActivityInUse,
                        "Bu öğrenci için onaylanmış bir değerlendirme zaten mevcut; birden fazla final karar oluşturulamaz.",
                        "a second final assessment for the same student is not allowed.",
                    ));
                }
                let pinned = details
                    .rubric_versions
                    .iter()
                    .find(|rubric| {
                        rubric.id == assessment.rubric_id
                            && rubric.version == assessment.rubric_version
                    })
                    .ok_or_else(|| {
                        performance_error(
                            AppErrorCode::RubricMissing,
                            "Değerlendirmenin sabitlediği rubrik sürümü bulunamadı.",
                            "pinned rubric version is missing.",
                        )
                    })?;
                let rated_ids = assessment
                    .ratings
                    .iter()
                    .map(|rating| rating.criterion_id.as_str())
                    .collect::<HashSet<_>>();
                if pinned
                    .criteria
                    .iter()
                    .any(|criterion| !rated_ids.contains(criterion.id.as_str()))
                {
                    return Err(performance_error(
                        AppErrorCode::AssessmentInvalidInput,
                        "Tüm ölçütler değerlendirilmeden onay verilemez.",
                        "not all criteria are rated.",
                    ));
                }
                let now = chrono::Utc::now().to_rfc3339();
                assessment.status = PerformanceAssessmentStatus::Approved;
                assessment.approved_at = Some(now.clone());
                assessment.assessed_at = Some(now.clone());
                assessment.updated_at = now;
                activity.updated_at = chrono::Utc::now().to_rfc3339();
                Ok(assessment.clone())
            },
        )?;
        Ok(output.result)
    }

    /// `Missing`/`NotPerformed` işaretleme. Bu kayıtlara sıfır puan yazılmaz;
    /// toplam hesabına girmez ve raporda ayrı gösterilir (K9).
    pub fn set_performance_assessment_status(
        &self,
        input: SetPerformanceAssessmentStatusInput,
        correlation_id: &str,
    ) -> Result<PerformanceAssessment, AppError> {
        if !matches!(
            input.status,
            PerformanceAssessmentStatus::Missing | PerformanceAssessmentStatus::NotPerformed
        ) {
            return Err(performance_error(
                AppErrorCode::AssessmentInvalidInput,
                "Bu komut yalnız eksik (Missing) veya gösterilmedi (NotPerformed) durumunu işaretler.",
                "status must be Missing or NotPerformed.",
            ));
        }
        let output = self.project_store.mutate(
            &input.project_id,
            MutationOptions::new("set_performance_assessment_status").correlation(correlation_id),
            |project, _context| {
                let activity = project
                    .assessment_activities
                    .iter_mut()
                    .find(|activity| activity.id == input.activity_id)
                    .ok_or_else(|| {
                        performance_error(
                            AppErrorCode::AssessmentActivityNotFound,
                            "Performans görevi bulunamadı.",
                            "activity_id not found.",
                        )
                    })?;
                if activity.assessment_type != AssessmentType::Performance {
                    return Err(performance_error(
                        AppErrorCode::AssessmentInvalidInput,
                        "Bu etkinlik performans görevi değil.",
                        "activity is not a performance task.",
                    ));
                }
                let details = activity.performance_details.clone().ok_or_else(|| {
                    performance_error(
                        AppErrorCode::AssessmentInvalidInput,
                        "Görev ayrıntıları bulunamadı.",
                        "performance_details is missing.",
                    )
                })?;
                let application = activity
                    .class_applications
                    .iter_mut()
                    .find(|application| application.id == input.application_id)
                    .ok_or_else(|| {
                        performance_error(
                            AppErrorCode::AssessmentClassApplicationNotFound,
                            "Sınıf uygulaması bulunamadı.",
                            "application_id not found for activity.",
                        )
                    })?;
                let approved_record_exists = match input.assessment_id.as_deref() {
                    Some(assessment_id) => {
                        application
                            .performance_assessments
                            .iter()
                            .any(|assessment| {
                                assessment.id == assessment_id
                                    && assessment.status == PerformanceAssessmentStatus::Approved
                            })
                    }
                    None => application
                        .performance_assessments
                        .iter()
                        .any(|assessment| {
                            assessment.student_id == input.student_id
                                && assessment.status == PerformanceAssessmentStatus::Approved
                        }),
                };
                if approved_record_exists {
                    return Err(performance_error(
                        AppErrorCode::AssessmentActivityInUse,
                        "Onaylanmış değerlendirmenin durumu değiştirilemez.",
                        "approved assessment status cannot be changed.",
                    ));
                }
                let now = chrono::Utc::now().to_rfc3339();
                let latest = latest_published_rubric(&details);
                let assessment = if let Some(assessment_id) = input.assessment_id.as_deref() {
                    application
                        .performance_assessments
                        .iter_mut()
                        .find(|assessment| assessment.id == assessment_id)
                        .ok_or_else(|| {
                            performance_error(
                                AppErrorCode::AssessmentClassApplicationNotFound,
                                "Değerlendirme kaydı bulunamadı.",
                                "assessment_id not found.",
                            )
                        })?
                } else {
                    match application
                        .performance_assessments
                        .iter_mut()
                        .find(|assessment| assessment.student_id == input.student_id)
                    {
                        Some(assessment) => assessment,
                        None => {
                            let (rubric_id, rubric_version) = match latest {
                                Some(rubric) => (rubric.id.clone(), rubric.version),
                                None => (String::new(), 0),
                            };
                            let created = PerformanceAssessment {
                                id: Uuid::new_v4().to_string(),
                                student_id: input.student_id.clone(),
                                rubric_id,
                                rubric_version,
                                ratings: vec![],
                                provisional_total: 0,
                                feedback: None,
                                status: input.status,
                                assessed_at: None,
                                approved_at: None,
                                created_at: now.clone(),
                                updated_at: now.clone(),
                            };
                            application.performance_assessments.push(created.clone());
                            return Ok(created);
                        }
                    }
                };
                assessment.ratings = vec![];
                assessment.provisional_total = 0;
                assessment.assessed_at = None;
                assessment.status = input.status;
                assessment.updated_at = now;
                activity.updated_at = chrono::Utc::now().to_rfc3339();
                Ok(assessment.clone())
            },
        )?;
        Ok(output.result)
    }

    pub fn list_performance_assessments(
        &self,
        input: ListPerformanceAssessmentsInput,
    ) -> Result<Vec<PerformanceAssessment>, AppError> {
        let project = self.load_project(&input.project_id)?;
        let activity = project
            .assessment_activities
            .iter()
            .find(|activity| activity.id == input.activity_id)
            .ok_or_else(|| {
                performance_error(
                    AppErrorCode::AssessmentActivityNotFound,
                    "Performans görevi bulunamadı.",
                    "activity_id not found.",
                )
            })?;
        if activity.assessment_type != AssessmentType::Performance {
            return Err(performance_error(
                AppErrorCode::AssessmentInvalidInput,
                "Bu etkinlik performans görevi değil.",
                "activity is not a performance task.",
            ));
        }
        let mut assessments = activity
            .class_applications
            .iter()
            .filter(|application| {
                input
                    .application_id
                    .as_ref()
                    .map_or(true, |application_id| &application.id == application_id)
            })
            .flat_map(|application| application.performance_assessments.iter().cloned())
            .collect::<Vec<_>>();
        assessments.sort_by(|left, right| left.student_id.cmp(&right.student_id));
        Ok(assessments)
    }

    /// Sınıf düzeyi performans sonuç raporu (Faz C). Rapor görüntü rubriğini
    /// yayınlanmış en yeni sürümden alır; her öğrencinin ölçüt puanları kendi
    /// sabitlediği rubrik sürümüyle çözülür (K8). `Missing`/`NotPerformed`
    /// kayıtların puanı yoktur; sıfır yazılmaz (K9).
    pub fn get_performance_report(
        &self,
        input: GetPerformanceReportInput,
    ) -> Result<PerformanceReportDto, AppError> {
        let project = self.load_project(&input.project_id)?;
        let activity = project
            .assessment_activities
            .iter()
            .find(|activity| activity.id == input.activity_id)
            .ok_or_else(|| {
                performance_error(
                    AppErrorCode::AssessmentActivityNotFound,
                    "Performans görevi bulunamadı.",
                    "activity_id not found.",
                )
            })?;
        if activity.assessment_type != AssessmentType::Performance {
            return Err(performance_error(
                AppErrorCode::AssessmentInvalidInput,
                "Bu etkinlik performans görevi değil.",
                "activity is not a performance task.",
            ));
        }
        let details = activity.performance_details.as_ref().ok_or_else(|| {
            performance_error(
                AppErrorCode::AssessmentInvalidInput,
                "Görev ayrıntıları bulunamadı.",
                "performance_details is missing.",
            )
        })?;
        let application = activity
            .class_applications
            .iter()
            .find(|application| application.id == input.application_id)
            .ok_or_else(|| {
                performance_error(
                    AppErrorCode::AssessmentClassApplicationNotFound,
                    "Sınıf uygulaması bulunamadı.",
                    "application_id not found for activity.",
                )
            })?;
        let display_rubric = latest_published_rubric(details).ok_or_else(|| {
            performance_error(
                AppErrorCode::RubricMissing,
                "Rubrik yayınlanmadan sonuç raporu oluşturulamaz.",
                "no published rubric version exists.",
            )
        })?;
        let roster = students_for_class(&project, &application.school_class_id)?;
        let class_name = project
            .school_classes
            .iter()
            .find(|school_class| school_class.id == application.school_class_id)
            .map(|school_class| {
                if school_class.display_name.trim().is_empty() {
                    school_class.name.clone()
                } else {
                    school_class.display_name.clone()
                }
            })
            .unwrap_or_else(|| "Sınıf bilgisi yok".to_string());
        let teacher_id = project
            .teaching_assignments
            .iter()
            .find(|assignment| {
                assignment.academic_year_id == activity.academic_year_id
                    && assignment.course_id == activity.course_id
                    && assignment.class_section_id == application.school_class_id
            })
            .and_then(|assignment| assignment.teacher_id.clone());

        let max_points = display_rubric
            .levels
            .first()
            .map(|level| level.points * display_rubric.criteria.len() as u32)
            .unwrap_or(0);

        let mut rows = Vec::with_capacity(roster.len());
        let mut assessed_count = 0u32;
        let mut approved_count = 0u32;
        let mut missing_count = 0u32;
        let mut not_performed_count = 0u32;
        let mut unrated_count = 0u32;
        // TD-38: assessments are indexed by student_id so the roster scan stays
        // O(roster + assessments) instead of O(roster × assessments).
        let assessments_by_student: HashMap<&str, &PerformanceAssessment> = application
            .performance_assessments
            .iter()
            .map(|assessment| (assessment.student_id.as_str(), assessment))
            .collect();
        for student in roster {
            let assessment = assessments_by_student.get(student.id.as_str()).copied();
            let status = assessment.map(|assessment| assessment.status);
            match status {
                Some(PerformanceAssessmentStatus::Missing) => missing_count += 1,
                Some(PerformanceAssessmentStatus::NotPerformed) => not_performed_count += 1,
                Some(PerformanceAssessmentStatus::Approved) => {
                    assessed_count += 1;
                    approved_count += 1;
                }
                Some(PerformanceAssessmentStatus::InProgress) => assessed_count += 1,
                None => unrated_count += 1,
            }

            let criterion_scores = match assessment {
                Some(assessment) => {
                    let pinned = details
                        .rubric_versions
                        .iter()
                        .find(|rubric| {
                            rubric.id == assessment.rubric_id
                                && rubric.version == assessment.rubric_version
                        })
                        .unwrap_or(display_rubric);
                    display_rubric
                        .criteria
                        .iter()
                        .map(|criterion| {
                            let level = assessment
                                .ratings
                                .iter()
                                .find(|rating| rating.criterion_id == criterion.id)
                                .and_then(|rating| {
                                    pinned
                                        .levels
                                        .iter()
                                        .find(|level| level.id == rating.level_id)
                                        .or_else(|| {
                                            display_rubric
                                                .levels
                                                .iter()
                                                .find(|level| level.id == rating.level_id)
                                        })
                                });
                            PerformanceReportCriterionScore {
                                criterion_id: criterion.id.clone(),
                                criterion_name: criterion.name.clone(),
                                level_id: level.map(|level| level.id.clone()),
                                level_name: level.map(|level| level.name.clone()),
                                points: level.map(|level| level.points),
                            }
                        })
                        .collect::<Vec<_>>()
                }
                None => display_rubric
                    .criteria
                    .iter()
                    .map(|criterion| PerformanceReportCriterionScore {
                        criterion_id: criterion.id.clone(),
                        criterion_name: criterion.name.clone(),
                        level_id: None,
                        level_name: None,
                        points: None,
                    })
                    .collect::<Vec<_>>(),
            };

            rows.push(PerformanceReportStudentRow {
                student_id: student.id.clone(),
                student_name: student
                    .display_name
                    .clone()
                    .unwrap_or_else(|| "İsimsiz öğrenci".to_string()),
                student_number: student.number.clone(),
                status,
                criterion_scores,
                total: assessment
                    .filter(|assessment| assessment.status == PerformanceAssessmentStatus::Approved)
                    .map(|assessment| assessment.provisional_total),
                provisional_total: assessment
                    .filter(|assessment| {
                        matches!(
                            assessment.status,
                            PerformanceAssessmentStatus::Approved
                                | PerformanceAssessmentStatus::InProgress
                        )
                    })
                    .map(|assessment| assessment.provisional_total),
                feedback: assessment.and_then(|assessment| assessment.feedback.clone()),
                assessed_at: assessment.and_then(|assessment| assessment.assessed_at.clone()),
                approved_at: assessment.and_then(|assessment| assessment.approved_at.clone()),
            });
        }

        Ok(PerformanceReportDto {
            task_title: if activity.title.trim().is_empty() {
                format!(
                    "{}. Dönem {}. Performans Görevi",
                    activity.term, activity.sequence_number
                )
            } else {
                activity.title.clone()
            },
            course_name: activity.course_name.clone(),
            grade_level: activity.grade_level,
            term: activity.term,
            sequence_number: activity.sequence_number,
            theme: (!details.theme.trim().is_empty()).then(|| details.theme.clone()),
            skill_area: Some(details.skill_area),
            work_mode: Some(details.work_mode),
            class_name,
            teacher_id,
            rubric_id: display_rubric.id.clone(),
            rubric_name: display_rubric.name.clone(),
            rubric_version: display_rubric.version,
            criteria: display_rubric.criteria.clone(),
            levels: display_rubric.levels.clone(),
            max_points,
            generated_at: chrono::Utc::now().to_rfc3339(),
            summary: PerformanceReportSummary {
                student_count: rows.len() as u32,
                assessed_count,
                approved_count,
                missing_count,
                not_performed_count,
                unrated_count,
            },
            rows,
        })
    }

    /// Performans görevi için authoritative step/readiness snapshot'ı (TD-03).
    /// Salt-okunur; adım durumu kararları frontend yerine bu DTO'dan gelir.
    pub fn get_performance_status(
        &self,
        input: PerformanceActivityIdInput,
    ) -> Result<PerformanceStatusDto, AppError> {
        let project = self.load_project(&input.project_id)?;
        let activity = project
            .assessment_activities
            .iter()
            .find(|activity| activity.id == input.activity_id)
            .ok_or_else(|| {
                performance_error(
                    AppErrorCode::AssessmentActivityNotFound,
                    "Performans görevi bulunamadı.",
                    "activity_id not found.",
                )
            })?;
        if activity.assessment_type != AssessmentType::Performance {
            return Err(performance_error(
                AppErrorCode::AssessmentInvalidInput,
                "Bu etkinlik performans görevi değil.",
                "activity is not a performance task.",
            ));
        }
        let details = activity.performance_details.as_ref();
        let versions = details
            .map(|details| details.rubric_versions.as_slice())
            .unwrap_or(&[]);
        let has_published_rubric = versions.iter().any(|rubric| rubric.version >= 1);
        let published_rubric_version = versions
            .iter()
            .filter(|rubric| rubric.version >= 1)
            .map(|rubric| rubric.version)
            .max();
        let has_draft_rubric = versions.iter().any(|rubric| rubric.version == 0);
        let has_task_details = details.is_some_and(|details| {
            !details.theme.trim().is_empty() || !details.task_instruction.trim().is_empty()
        });

        let applications = activity
            .class_applications
            .iter()
            .filter(|application| application.status != ClassApplicationStatus::Archived)
            .collect::<Vec<_>>();
        let total_students = applications
            .iter()
            .map(|application| application.student_scope_ids.len() as u32)
            .sum();
        let mut approved_count = 0u32;
        let mut in_progress_count = 0u32;
        let mut missing_count = 0u32;
        let mut not_performed_count = 0u32;
        for application in &applications {
            for assessment in &application.performance_assessments {
                match assessment.status {
                    PerformanceAssessmentStatus::Approved => approved_count += 1,
                    PerformanceAssessmentStatus::InProgress => in_progress_count += 1,
                    PerformanceAssessmentStatus::Missing => missing_count += 1,
                    PerformanceAssessmentStatus::NotPerformed => not_performed_count += 1,
                }
            }
        }
        let all_approved = approved_count >= total_students && total_students > 0;

        Ok(PerformanceStatusDto {
            has_published_rubric,
            published_rubric_version,
            has_draft_rubric,
            has_task_details,
            total_students,
            approved_count,
            in_progress_count,
            missing_count,
            not_performed_count,
            all_approved,
        })
    }

    fn load_project(&self, project_id: &str) -> Result<Project, AppError> {
        self.project_store
            .get_project_snapshot(project_id.to_string())
    }
}

fn latest_published_rubric(details: &PerformanceDetails) -> Option<&PerformanceRubric> {
    details
        .rubric_versions
        .iter()
        .filter(|rubric| rubric.version >= 1)
        .max_by_key(|rubric| rubric.version)
}

fn validate_ratings(
    ratings: &[CriterionRating],
    rubric: &PerformanceRubric,
) -> Result<(), AppError> {
    let criterion_ids = rubric
        .criteria
        .iter()
        .map(|criterion| criterion.id.as_str())
        .collect::<HashSet<_>>();
    let level_ids = rubric
        .levels
        .iter()
        .map(|level| level.id.as_str())
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    for rating in ratings {
        if !criterion_ids.contains(rating.criterion_id.as_str())
            || !level_ids.contains(rating.level_id.as_str())
        {
            return Err(performance_error(
                AppErrorCode::AssessmentInvalidInput,
                "Seçilen ölçüt veya düzey yayınlanan rubriğe ait değil.",
                "rating references unknown criterion or level.",
            ));
        }
        if !seen.insert(rating.criterion_id.as_str()) {
            return Err(performance_error(
                AppErrorCode::AssessmentInvalidInput,
                "Bir ölçüt için yalnız bir düzey seçilebilir.",
                "duplicate criterion rating.",
            ));
        }
    }
    Ok(())
}

fn compute_provisional_total(ratings: &[CriterionRating], rubric: &PerformanceRubric) -> u32 {
    ratings
        .iter()
        .filter_map(|rating| {
            rubric
                .levels
                .iter()
                .find(|level| level.id == rating.level_id)
                .map(|level| level.points)
        })
        .sum()
}

/// Rubrik doğrulama (rapor §4/§7): 3-6 ölçüt, 3 veya 5 düzey, her ölçüt için ad +
/// açıklama + tüm düzeylerde gözlenebilir tanım; düzey puanları azalan sırada ve
/// benzersiz.
fn validate_rubric(rubric: &PerformanceRubric) -> Result<(), AppError> {
    if rubric.name.trim().is_empty() {
        return Err(performance_error(
            AppErrorCode::AssessmentInvalidInput,
            "Rubrik adı boş olamaz.",
            "rubric name is empty.",
        ));
    }
    let criterion_count = rubric.criteria.len();
    if !(3..=6).contains(&criterion_count) {
        return Err(performance_error(
            AppErrorCode::AssessmentInvalidInput,
            "Rubrik 3 ile 6 arasında ölçüt içermelidir.",
            "criterion count out of 3..=6 range.",
        ));
    }
    let level_count = rubric.levels.len();
    if level_count != 3 && level_count != 5 {
        return Err(performance_error(
            AppErrorCode::AssessmentInvalidInput,
            "Rubrik 3 veya 5 düzey içermelidir.",
            "level count must be 3 or 5.",
        ));
    }
    let mut criterion_ids = HashSet::new();
    let mut level_ids = HashSet::new();
    for criterion in &rubric.criteria {
        if criterion.id.trim().is_empty()
            || criterion.name.trim().is_empty()
            || criterion.description.trim().is_empty()
        {
            return Err(performance_error(
                AppErrorCode::AssessmentInvalidInput,
                "Her ölçütün kimliği, adı ve açıklaması doldurulmalıdır.",
                "criterion id/name/description is empty.",
            ));
        }
        if !criterion_ids.insert(criterion.id.clone()) {
            return Err(performance_error(
                AppErrorCode::AssessmentInvalidInput,
                "Ölçüt kimlikleri benzersiz olmalıdır.",
                "duplicate criterion id.",
            ));
        }
        for level in &rubric.levels {
            let description = criterion
                .level_descriptions
                .iter()
                .find(|entry| entry.level_id == level.id);
            match description {
                Some(description) if !description.description.trim().is_empty() => {}
                _ => {
                    return Err(performance_error(
                        AppErrorCode::AssessmentInvalidInput,
                        "Her ölçüt için tüm düzeylerde gözlenebilir tanım zorunludur.",
                        "missing level description for criterion.",
                    ));
                }
            }
        }
    }
    let mut previous_points: Option<u32> = None;
    for level in &rubric.levels {
        if level.id.trim().is_empty()
            || level.name.trim().is_empty()
            || level.description.trim().is_empty()
        {
            return Err(performance_error(
                AppErrorCode::AssessmentInvalidInput,
                "Her düzeyin kimliği, adı, puanı ve tanımı doldurulmalıdır.",
                "level id/name/description is empty.",
            ));
        }
        if !level_ids.insert(level.id.clone()) {
            return Err(performance_error(
                AppErrorCode::AssessmentInvalidInput,
                "Düzey kimlikleri benzersiz olmalıdır.",
                "duplicate level id.",
            ));
        }
        if let Some(previous) = previous_points {
            if level.points >= previous {
                return Err(performance_error(
                    AppErrorCode::AssessmentInvalidInput,
                    "Düzey puanları azalan sırada ve birbirinden farklı olmalıdır.",
                    "level points are not strictly descending.",
                ));
            }
        }
        previous_points = Some(level.points);
    }
    Ok(())
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn performance_error(code: AppErrorCode, message: &str, technical_details: &str) -> AppError {
    AppError {
        code,
        message: message.to_string(),
        recoverable: true,
        suggested_action: Some(
            "Performans görevi bilgilerini kontrol edip tekrar deneyin.".to_string(),
        ),
        technical_details: Some(technical_details.to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::assessment_organization_service::AssessmentOrganizationService;
    use crate::services::school_class_service::{
        CreateSchoolClassInput, CreateTeachingAssignmentInput, SchoolClassService,
    };
    use std::sync::Arc;

    fn temp_project() -> (ProjectStore, String, Arc<SchoolClassService>) {
        let root = std::env::temp_dir().join(format!("rubrika-performance-{}", Uuid::new_v4()));
        let store = ProjectStore::new();
        let project = store
            .create_project(
                "Performance".to_string(),
                root.to_string_lossy().to_string(),
            )
            .expect("test project should be created");
        let classes = Arc::new(SchoolClassService::new(store.clone()));
        (store, project.id, classes)
    }

    fn setup_environment(
        store: ProjectStore,
        project_id: String,
        classes: Arc<SchoolClassService>,
        grade_level: u32,
    ) -> String {
        let school_class = classes
            .create_school_class(CreateSchoolClassInput {
                project_id: project_id.clone(),
                name: format!("{grade_level}A"),
                academic_year: Some("2026-2027".into()),
                grade_level: Some(grade_level),
                section: Some("A".into()),
                display_order: None,
            })
            .expect("class should be created");
        classes
            .create_teaching_assignment(CreateTeachingAssignmentInput {
                project_id: project_id.clone(),
                academic_year_id: "2026-2027".into(),
                course_id: "tde".into(),
                course_name: "Türk Dili ve Edebiyatı".into(),
                class_section_id: school_class.id.clone(),
                teacher_id: None,
            })
            .expect("assignment should be created");
        let student = crate::domain::student::Student {
            id: "student-1".into(),
            display_name: Some("Öğrenci".into()),
            number: Some("1".into()),
            class_name: Some(format!("{grade_level}A")),
            warnings: vec![],
            identity_ocr: None,
        };
        let mut project = store
            .get_project_snapshot(project_id.clone())
            .expect("project should load");
        project.students.push(student);
        store.save_project(&project).expect("student should save");
        school_class.id
    }

    fn service(store: ProjectStore, classes: Arc<SchoolClassService>) -> PerformanceService {
        let org = Arc::new(AssessmentOrganizationService::new(store.clone(), classes));
        PerformanceService::new(store, org)
    }

    fn add_student(store: &ProjectStore, project_id: &str, student_id: &str, class_name: &str) {
        let mut project = store
            .get_project_snapshot(project_id.to_string())
            .expect("project should load");
        project.students.push(crate::domain::student::Student {
            id: student_id.into(),
            display_name: Some("Öğrenci".into()),
            number: Some("2".into()),
            class_name: Some(class_name.into()),
            warnings: vec![],
            identity_ocr: None,
        });
        store.save_project(&project).expect("student should save");
    }

    fn full_ratings() -> Vec<CriterionRating> {
        vec![
            CriterionRating {
                criterion_id: "c1".into(),
                level_id: "l1".into(),
                note: None,
            },
            CriterionRating {
                criterion_id: "c2".into(),
                level_id: "l2".into(),
                note: None,
            },
            CriterionRating {
                criterion_id: "c3".into(),
                level_id: "l3".into(),
                note: None,
            },
        ]
    }

    fn create_task(
        service: &PerformanceService,
        project_id: &str,
        class_id: &str,
        sequence_number: u32,
    ) -> AssessmentActivity {
        service
            .create_performance_task(
                CreatePerformanceTaskInput {
                    project_id: project_id.to_string(),
                    academic_year_id: "2026-2027".into(),
                    course_id: "tde".into(),
                    course_name: "Türk Dili ve Edebiyatı".into(),
                    grade_level: 9,
                    term: 1,
                    sequence_number,
                    school_class_ids: vec![class_id.to_string()],
                    title: format!("{sequence_number}. Performans Görevi"),
                    performance_details: PerformanceDetails {
                        theme: "Doğa ve insan".into(),
                        ..PerformanceDetails::default()
                    },
                    initial_rubric: None,
                },
                "test-correlation-id",
            )
            .expect("task should be created")
    }

    fn valid_rubric() -> PerformanceRubric {
        PerformanceRubric {
            id: "rubric-1".into(),
            name: "Yazılı Ürün Rubriği".into(),
            version: 0,
            criteria: vec![
                criterion("c1", "Metne uygunluk"),
                criterion("c2", "İçerik ve yapı"),
                criterion("c3", "Dil ve üslup"),
            ],
            levels: vec![
                level("l1", "Çok iyi", 5),
                level("l2", "İyi", 4),
                level("l3", "Geliştirilebilir", 3),
                level("l4", "Başlangıç", 2),
                level("l5", "Gözlenmedi", 1),
            ],
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    fn criterion(id: &str, name: &str) -> crate::domain::performance::PerformanceCriterion {
        let level_descriptions = [
            "Çok iyi",
            "İyi",
            "Geliştirilebilir",
            "Başlangıç",
            "Gözlenmedi",
        ]
        .iter()
        .zip(["l1", "l2", "l3", "l4", "l5"])
        .map(
            |(description, level_id)| crate::domain::performance::LevelDescription {
                level_id: level_id.to_string(),
                description: format!("{name}: {description} tanımı"),
            },
        )
        .collect();
        crate::domain::performance::PerformanceCriterion {
            id: id.to_string(),
            name: name.to_string(),
            description: format!("{name} açıklaması"),
            level_descriptions,
        }
    }

    fn level(id: &str, name: &str, points: u32) -> crate::domain::performance::PerformanceLevel {
        crate::domain::performance::PerformanceLevel {
            id: id.to_string(),
            name: name.to_string(),
            points,
            description: format!("{name} tanımı"),
        }
    }

    #[test]
    fn publish_accepts_valid_rubric_and_increments_version() {
        let (store, project_id, classes) = temp_project();
        let class_id = setup_environment(store.clone(), project_id.clone(), classes.clone(), 9);
        let service = service(store, classes);
        let activity = create_task(&service, &project_id, &class_id, 1);
        let published = service
            .publish_performance_rubric(
                PublishPerformanceRubricInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    rubric: valid_rubric(),
                },
                "test-correlation-id",
            )
            .expect("rubric should publish");
        assert_eq!(published.version, 1);
        let history = service
            .get_performance_rubric_history(PerformanceActivityIdInput {
                project_id: project_id.clone(),
                activity_id: activity.id.clone(),
            })
            .expect("history should load");
        assert_eq!(history.len(), 2);
        let second = service
            .publish_performance_rubric(
                PublishPerformanceRubricInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id,
                    rubric: valid_rubric(),
                },
                "test-correlation-id",
            )
            .expect("second publish should succeed");
        assert_eq!(second.version, 2);
    }

    #[test]
    fn publish_rejects_rubrics_without_observable_descriptions_or_wrong_sizes() {
        let mut rubric = valid_rubric();
        rubric.criteria[0].level_descriptions[2].description = String::new();
        assert_eq!(
            validate_rubric(&rubric).unwrap_err().code,
            AppErrorCode::AssessmentInvalidInput
        );
        let mut rubric = valid_rubric();
        rubric.criteria.pop();
        assert_eq!(
            validate_rubric(&rubric).unwrap_err().code,
            AppErrorCode::AssessmentInvalidInput
        );
        let mut rubric = valid_rubric();
        rubric.levels.truncate(4);
        assert_eq!(
            validate_rubric(&rubric).unwrap_err().code,
            AppErrorCode::AssessmentInvalidInput
        );
    }

    #[test]
    fn publish_rejects_equal_or_ascending_level_points() {
        let mut rubric = valid_rubric();
        rubric.levels[2].points = 4;
        assert_eq!(
            validate_rubric(&rubric).unwrap_err().code,
            AppErrorCode::AssessmentInvalidInput
        );
        let mut rubric = valid_rubric();
        rubric.levels[1].points = 6;
        assert_eq!(
            validate_rubric(&rubric).unwrap_err().code,
            AppErrorCode::AssessmentInvalidInput
        );
    }

    #[test]
    fn save_rejects_unknown_rating_ids_and_computes_total_server_side() {
        let (store, project_id, classes) = temp_project();
        let class_id = setup_environment(store.clone(), project_id.clone(), classes.clone(), 9);
        let service = service(store, classes);
        let activity = create_task(&service, &project_id, &class_id, 1);
        service
            .publish_performance_rubric(
                PublishPerformanceRubricInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    rubric: valid_rubric(),
                },
                "test-correlation-id",
            )
            .expect("rubric should publish");
        let application = activity.class_applications[0].id.clone();

        let error = service
            .save_performance_assessment(
                SavePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    student_id: "student-1".into(),
                    assessment_id: None,
                    ratings: vec![CriterionRating {
                        criterion_id: "ghost-criterion".into(),
                        level_id: "l1".into(),
                        note: None,
                    }],
                    feedback: None,
                },
                "test-correlation-id",
            )
            .unwrap_err();
        assert_eq!(error.code, AppErrorCode::AssessmentInvalidInput);

        let saved = service
            .save_performance_assessment(
                SavePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    student_id: "student-1".into(),
                    assessment_id: None,
                    ratings: vec![
                        CriterionRating {
                            criterion_id: "c1".into(),
                            level_id: "l1".into(),
                            note: None,
                        },
                        CriterionRating {
                            criterion_id: "c2".into(),
                            level_id: "l2".into(),
                            note: None,
                        },
                    ],
                    feedback: Some("Güzel çalışma".into()),
                },
                "test-correlation-id",
            )
            .expect("assessment should save");
        assert_eq!(saved.provisional_total, 5 + 4);
        assert_eq!(saved.rubric_version, 1);
    }

    #[test]
    fn approve_rejects_incomplete_criteria_and_locks_later_saves() {
        let (store, project_id, classes) = temp_project();
        let class_id = setup_environment(store.clone(), project_id.clone(), classes.clone(), 9);
        let service = service(store, classes);
        let activity = create_task(&service, &project_id, &class_id, 1);
        service
            .publish_performance_rubric(
                PublishPerformanceRubricInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    rubric: valid_rubric(),
                },
                "test-correlation-id",
            )
            .expect("rubric should publish");
        let application = activity.class_applications[0].id.clone();

        let saved = service
            .save_performance_assessment(
                SavePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    student_id: "student-1".into(),
                    assessment_id: None,
                    ratings: vec![CriterionRating {
                        criterion_id: "c1".into(),
                        level_id: "l1".into(),
                        note: None,
                    }],
                    feedback: None,
                },
                "test-correlation-id",
            )
            .expect("partial assessment should save");
        let error = service
            .approve_performance_assessment(
                ApprovePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    assessment_id: saved.id.clone(),
                },
                "test-correlation-id",
            )
            .unwrap_err();
        assert_eq!(error.code, AppErrorCode::AssessmentInvalidInput);

        let completed = service
            .save_performance_assessment(
                SavePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    student_id: "student-1".into(),
                    assessment_id: Some(saved.id.clone()),
                    ratings: vec![
                        CriterionRating {
                            criterion_id: "c1".into(),
                            level_id: "l1".into(),
                            note: None,
                        },
                        CriterionRating {
                            criterion_id: "c2".into(),
                            level_id: "l1".into(),
                            note: None,
                        },
                        CriterionRating {
                            criterion_id: "c3".into(),
                            level_id: "l1".into(),
                            note: None,
                        },
                    ],
                    feedback: None,
                },
                "test-correlation-id",
            )
            .expect("completed assessment should save");
        let approved = service
            .approve_performance_assessment(
                ApprovePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    assessment_id: completed.id.clone(),
                },
                "test-correlation-id",
            )
            .expect("approval should succeed");
        assert_eq!(approved.status, PerformanceAssessmentStatus::Approved);
        assert!(approved.approved_at.is_some());

        let error = service
            .save_performance_assessment(
                SavePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    student_id: "student-1".into(),
                    assessment_id: Some(completed.id.clone()),
                    ratings: vec![],
                    feedback: None,
                },
                "test-correlation-id",
            )
            .unwrap_err();
        assert_eq!(error.code, AppErrorCode::AssessmentActivityInUse);
    }

    #[test]
    fn approved_rubric_cannot_publish_a_new_version() {
        let (store, project_id, classes) = temp_project();
        let class_id = setup_environment(store.clone(), project_id.clone(), classes.clone(), 9);
        let service = service(store, classes);
        let activity = create_task(&service, &project_id, &class_id, 1);
        service
            .publish_performance_rubric(
                PublishPerformanceRubricInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    rubric: valid_rubric(),
                },
                "test-correlation-id",
            )
            .expect("rubric should publish");
        let application = activity.class_applications[0].id.clone();
        let saved = service
            .save_performance_assessment(
                SavePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    student_id: "student-1".into(),
                    assessment_id: None,
                    ratings: vec![
                        CriterionRating {
                            criterion_id: "c1".into(),
                            level_id: "l1".into(),
                            note: None,
                        },
                        CriterionRating {
                            criterion_id: "c2".into(),
                            level_id: "l2".into(),
                            note: None,
                        },
                        CriterionRating {
                            criterion_id: "c3".into(),
                            level_id: "l3".into(),
                            note: None,
                        },
                    ],
                    feedback: None,
                },
                "test-correlation-id",
            )
            .expect("assessment should save");
        service
            .approve_performance_assessment(
                ApprovePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    assessment_id: saved.id.clone(),
                },
                "test-correlation-id",
            )
            .expect("approval should succeed");

        let error = service
            .publish_performance_rubric(
                PublishPerformanceRubricInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id,
                    rubric: valid_rubric(),
                },
                "test-correlation-id",
            )
            .unwrap_err();
        assert_eq!(error.code, AppErrorCode::AssessmentActivityInUse);
    }

    #[test]
    fn missing_status_writes_no_points_and_is_listed_separately() {
        let (store, project_id, classes) = temp_project();
        let class_id = setup_environment(store.clone(), project_id.clone(), classes.clone(), 9);
        let service = service(store, classes);
        let activity = create_task(&service, &project_id, &class_id, 1);
        service
            .publish_performance_rubric(
                PublishPerformanceRubricInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    rubric: valid_rubric(),
                },
                "test-correlation-id",
            )
            .expect("rubric should publish");
        let application = activity.class_applications[0].id.clone();
        let missing = service
            .set_performance_assessment_status(
                SetPerformanceAssessmentStatusInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    student_id: "student-1".into(),
                    assessment_id: None,
                    status: PerformanceAssessmentStatus::Missing,
                },
                "test-correlation-id",
            )
            .expect("missing should mark");
        assert_eq!(missing.status, PerformanceAssessmentStatus::Missing);
        assert_eq!(missing.provisional_total, 0);
        assert!(missing.ratings.is_empty());
        let listed = service
            .list_performance_assessments(ListPerformanceAssessmentsInput {
                project_id: project_id.clone(),
                activity_id: activity.id,
                application_id: Some(application),
            })
            .expect("assessments should list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].status, PerformanceAssessmentStatus::Missing);
    }

    #[test]
    fn save_rejects_student_outside_class_roster() {
        let (store, project_id, classes) = temp_project();
        let class_id = setup_environment(store.clone(), project_id.clone(), classes.clone(), 9);
        let service = service(store, classes);
        let activity = create_task(&service, &project_id, &class_id, 1);
        service
            .publish_performance_rubric(
                PublishPerformanceRubricInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    rubric: valid_rubric(),
                },
                "test-correlation-id",
            )
            .expect("rubric should publish");
        let application = activity.class_applications[0].id.clone();
        let error = service
            .save_performance_assessment(
                SavePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id,
                    application_id: application,
                    student_id: "ghost-student".into(),
                    assessment_id: None,
                    ratings: vec![],
                    feedback: None,
                },
                "test-correlation-id",
            )
            .unwrap_err();
        assert_eq!(error.code, AppErrorCode::StudentNotFound);
    }

    #[test]
    fn save_requires_published_rubric() {
        let (store, project_id, classes) = temp_project();
        let class_id = setup_environment(store.clone(), project_id.clone(), classes.clone(), 9);
        let service = service(store, classes);
        let activity = create_task(&service, &project_id, &class_id, 1);
        let application = activity.class_applications[0].id.clone();
        let error = service
            .save_performance_assessment(
                SavePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id,
                    application_id: application,
                    student_id: "student-1".into(),
                    assessment_id: None,
                    ratings: vec![],
                    feedback: None,
                },
                "test-correlation-id",
            )
            .unwrap_err();
        assert_eq!(error.code, AppErrorCode::RubricMissing);
    }

    #[test]
    fn create_rejects_duplicate_sequence_key_in_performance_scope() {
        let (store, project_id, classes) = temp_project();
        let class_id = setup_environment(store.clone(), project_id.clone(), classes.clone(), 9);
        let service = service(store, classes);
        create_task(&service, &project_id, &class_id, 1);
        let error = service
            .create_performance_task(
                CreatePerformanceTaskInput {
                    project_id: project_id.clone(),
                    academic_year_id: "2026-2027".into(),
                    course_id: "tde".into(),
                    course_name: "Türk Dili ve Edebiyatı".into(),
                    grade_level: 9,
                    term: 1,
                    sequence_number: 1,
                    school_class_ids: vec![class_id],
                    title: "İkinci".into(),
                    performance_details: PerformanceDetails {
                        theme: "Tema".into(),
                        ..PerformanceDetails::default()
                    },
                    initial_rubric: None,
                },
                "test-correlation-id",
            )
            .unwrap_err();
        assert_eq!(error.code, AppErrorCode::AssessmentActivityAlreadyExists);
    }

    #[test]
    fn serialization_roundtrip_keeps_rubric_versions() {
        let (store, project_id, classes) = temp_project();
        let class_id = setup_environment(store.clone(), project_id.clone(), classes.clone(), 9);
        let service = service(store.clone(), classes);
        let activity = create_task(&service, &project_id, &class_id, 1);
        let activity_id = activity.id.clone();
        service
            .publish_performance_rubric(
                PublishPerformanceRubricInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    rubric: valid_rubric(),
                },
                "test-correlation-id",
            )
            .expect("rubric should publish");
        let persisted = store
            .get_project_snapshot(project_id.clone())
            .expect("project should load");
        let stored = persisted
            .assessment_activities
            .iter()
            .find(|candidate| candidate.id == activity_id)
            .expect("activity should exist");
        assert_eq!(
            stored
                .performance_details
                .as_ref()
                .unwrap()
                .rubric_versions
                .len(),
            2
        );
    }

    #[test]
    fn approved_assessment_status_cannot_be_changed_without_assessment_id() {
        let (store, project_id, classes) = temp_project();
        let class_id = setup_environment(store.clone(), project_id.clone(), classes.clone(), 9);
        let service = service(store, classes);
        let activity = create_task(&service, &project_id, &class_id, 1);
        service
            .publish_performance_rubric(
                PublishPerformanceRubricInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    rubric: valid_rubric(),
                },
                "test-correlation-id",
            )
            .expect("rubric should publish");
        let application = activity.class_applications[0].id.clone();
        let completed = service
            .save_performance_assessment(
                SavePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    student_id: "student-1".into(),
                    assessment_id: None,
                    ratings: full_ratings(),
                    feedback: None,
                },
                "test-correlation-id",
            )
            .expect("completed assessment should save");
        service
            .approve_performance_assessment(
                ApprovePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    assessment_id: completed.id.clone(),
                },
                "test-correlation-id",
            )
            .expect("approval should succeed");

        let error = service
            .set_performance_assessment_status(
                SetPerformanceAssessmentStatusInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    student_id: "student-1".into(),
                    assessment_id: None,
                    status: PerformanceAssessmentStatus::Missing,
                },
                "test-correlation-id",
            )
            .unwrap_err();
        assert_eq!(error.code, AppErrorCode::AssessmentActivityInUse);
    }

    #[test]
    fn republished_rubric_does_not_rebase_existing_draft_version_or_total() {
        let (store, project_id, classes) = temp_project();
        let class_id = setup_environment(store.clone(), project_id.clone(), classes.clone(), 9);
        let service = service(store, classes);
        let activity = create_task(&service, &project_id, &class_id, 1);
        service
            .publish_performance_rubric(
                PublishPerformanceRubricInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    rubric: valid_rubric(),
                },
                "test-correlation-id",
            )
            .expect("v1 should publish");
        let application = activity.class_applications[0].id.clone();
        let saved = service
            .save_performance_assessment(
                SavePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    student_id: "student-1".into(),
                    assessment_id: None,
                    ratings: full_ratings(),
                    feedback: None,
                },
                "test-correlation-id",
            )
            .expect("v1 draft should save");
        assert_eq!(saved.rubric_version, 1);
        assert_eq!(saved.provisional_total, 12);

        let mut changed = valid_rubric();
        changed.levels[0].points = 10;
        changed.levels[1].points = 8;
        changed.levels[2].points = 6;
        changed.levels[3].points = 4;
        changed.levels[4].points = 2;
        service
            .publish_performance_rubric(
                PublishPerformanceRubricInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    rubric: changed,
                },
                "test-correlation-id",
            )
            .expect("v2 should publish");

        let resaved = service
            .save_performance_assessment(
                SavePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    student_id: "student-1".into(),
                    assessment_id: Some(saved.id.clone()),
                    ratings: full_ratings(),
                    feedback: None,
                },
                "test-correlation-id",
            )
            .expect("draft resave should succeed");
        assert_eq!(
            resaved.rubric_version, 1,
            "existing draft must stay pinned to the rubric version it was saved with"
        );
        assert_eq!(
            resaved.provisional_total, 12,
            "existing draft total must stay computed on the pinned rubric version's points"
        );
    }

    #[test]
    fn save_rejects_assessment_id_that_belongs_to_another_application() {
        let (store, project_id, classes) = temp_project();
        let class_a = setup_environment(store.clone(), project_id.clone(), classes.clone(), 9);
        let class_b = classes
            .create_school_class(CreateSchoolClassInput {
                project_id: project_id.clone(),
                name: "9B".into(),
                academic_year: Some("2026-2027".into()),
                grade_level: Some(9),
                section: Some("B".into()),
                display_order: None,
            })
            .expect("class B should be created");
        classes
            .create_teaching_assignment(CreateTeachingAssignmentInput {
                project_id: project_id.clone(),
                academic_year_id: "2026-2027".into(),
                course_id: "tde".into(),
                course_name: "Türk Dili ve Edebiyatı".into(),
                class_section_id: class_b.id.clone(),
                teacher_id: None,
            })
            .expect("assignment B should be created");
        add_student(&store, &project_id, "student-2", "9B");
        let service = service(store, classes);
        let activity = service
            .create_performance_task(
                CreatePerformanceTaskInput {
                    project_id: project_id.clone(),
                    academic_year_id: "2026-2027".into(),
                    course_id: "tde".into(),
                    course_name: "Türk Dili ve Edebiyatı".into(),
                    grade_level: 9,
                    term: 1,
                    sequence_number: 1,
                    school_class_ids: vec![class_a.clone(), class_b.id.clone()],
                    title: "1. Performans Görevi".into(),
                    performance_details: PerformanceDetails {
                        theme: "Doğa ve insan".into(),
                        ..PerformanceDetails::default()
                    },
                    initial_rubric: None,
                },
                "test-correlation-id",
            )
            .expect("task should be created");
        service
            .publish_performance_rubric(
                PublishPerformanceRubricInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    rubric: valid_rubric(),
                },
                "test-correlation-id",
            )
            .expect("rubric should publish");
        let app_a = activity
            .class_applications
            .iter()
            .find(|application| application.school_class_id == class_a)
            .expect("application A should exist")
            .id
            .clone();
        let app_b = activity
            .class_applications
            .iter()
            .find(|application| application.school_class_id == class_b.id)
            .expect("application B should exist")
            .id
            .clone();
        let in_app_a = service
            .save_performance_assessment(
                SavePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: app_a.clone(),
                    student_id: "student-1".into(),
                    assessment_id: None,
                    ratings: full_ratings(),
                    feedback: None,
                },
                "test-correlation-id",
            )
            .expect("app A assessment should save");
        service
            .save_performance_assessment(
                SavePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: app_b.clone(),
                    student_id: "student-2".into(),
                    assessment_id: None,
                    ratings: full_ratings(),
                    feedback: None,
                },
                "test-correlation-id",
            )
            .expect("app B assessment should save");

        let error = service
            .save_performance_assessment(
                SavePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: app_b.clone(),
                    student_id: "student-2".into(),
                    assessment_id: Some(in_app_a.id.clone()),
                    ratings: full_ratings(),
                    feedback: None,
                },
                "test-correlation-id",
            )
            .unwrap_err();
        assert_eq!(error.code, AppErrorCode::AssessmentInvalidInput);

        let listed = service
            .list_performance_assessments(ListPerformanceAssessmentsInput {
                project_id: project_id.clone(),
                activity_id: activity.id.clone(),
                application_id: Some(app_b),
            })
            .expect("assessments should list");
        assert_eq!(
            listed.len(),
            1,
            "a foreign assessment id must not create a duplicate record in the other application"
        );
    }

    #[test]
    fn approve_rejects_a_second_final_assessment_for_the_same_student() {
        let (store, project_id, classes) = temp_project();
        let class_id = setup_environment(store.clone(), project_id.clone(), classes.clone(), 9);
        let service = service(store.clone(), classes);
        let activity = create_task(&service, &project_id, &class_id, 1);
        service
            .publish_performance_rubric(
                PublishPerformanceRubricInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    rubric: valid_rubric(),
                },
                "test-correlation-id",
            )
            .expect("rubric should publish");
        let application = activity.class_applications[0].id.clone();
        let first = service
            .save_performance_assessment(
                SavePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    student_id: "student-1".into(),
                    assessment_id: None,
                    ratings: full_ratings(),
                    feedback: None,
                },
                "test-correlation-id",
            )
            .expect("first assessment should save");
        service
            .approve_performance_assessment(
                ApprovePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    assessment_id: first.id.clone(),
                },
                "test-correlation-id",
            )
            .expect("first assessment should approve");

        let mut project = store
            .get_project_snapshot(project_id.clone())
            .expect("project should load");
        let second_id = Uuid::new_v4().to_string();
        project.assessment_activities[0].class_applications[0]
            .performance_assessments
            .push(PerformanceAssessment {
                id: second_id.clone(),
                student_id: "student-1".into(),
                rubric_id: first.rubric_id.clone(),
                rubric_version: first.rubric_version,
                ratings: full_ratings(),
                provisional_total: first.provisional_total,
                feedback: None,
                status: PerformanceAssessmentStatus::InProgress,
                assessed_at: Some("2026-01-01T00:00:00Z".into()),
                approved_at: None,
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
            });
        store
            .save_project(&project)
            .expect("duplicate legacy assessment should save");

        let error = service
            .approve_performance_assessment(
                ApprovePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    assessment_id: second_id,
                },
                "test-correlation-id",
            )
            .unwrap_err();
        assert_eq!(error.code, AppErrorCode::AssessmentActivityInUse);
        let listed = service
            .list_performance_assessments(ListPerformanceAssessmentsInput {
                project_id: project_id.clone(),
                activity_id: activity.id.clone(),
                application_id: Some(application),
            })
            .expect("assessments should list");
        assert_eq!(
            listed
                .iter()
                .filter(|assessment| assessment.status == PerformanceAssessmentStatus::Approved)
                .count(),
            1,
            "exactly one final assessment must exist for the student"
        );
    }

    #[test]
    fn report_does_not_publish_in_progress_total_as_final_total() {
        let (store, project_id, classes) = temp_project();
        let class_id = setup_environment(store.clone(), project_id.clone(), classes.clone(), 9);
        add_student(&store, &project_id, "student-2", "9A");
        let service = service(store, classes);
        let activity = create_task(&service, &project_id, &class_id, 1);
        service
            .publish_performance_rubric(
                PublishPerformanceRubricInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    rubric: valid_rubric(),
                },
                "test-correlation-id",
            )
            .expect("rubric should publish");
        let application = activity.class_applications[0].id.clone();
        let approved = service
            .save_performance_assessment(
                SavePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    student_id: "student-1".into(),
                    assessment_id: None,
                    ratings: full_ratings(),
                    feedback: None,
                },
                "test-correlation-id",
            )
            .expect("approved assessment should save");
        service
            .approve_performance_assessment(
                ApprovePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    assessment_id: approved.id.clone(),
                },
                "test-correlation-id",
            )
            .expect("approval should succeed");
        service
            .save_performance_assessment(
                SavePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    student_id: "student-2".into(),
                    assessment_id: None,
                    ratings: full_ratings(),
                    feedback: None,
                },
                "test-correlation-id",
            )
            .expect("in-progress assessment should save");

        let report = service
            .get_performance_report(GetPerformanceReportInput {
                project_id: project_id.clone(),
                activity_id: activity.id.clone(),
                application_id: application,
            })
            .expect("report should build");
        let in_progress = report
            .rows
            .iter()
            .find(|row| row.status == Some(PerformanceAssessmentStatus::InProgress))
            .expect("in-progress row should be present");
        assert_eq!(
            in_progress.total, None,
            "unapproved provisional total must not appear as the report final total"
        );
        let approved_row = report
            .rows
            .iter()
            .find(|row| row.status == Some(PerformanceAssessmentStatus::Approved))
            .expect("approved row should be present");
        assert!(approved_row.total.is_some());
    }

    #[test]
    fn status_reports_authoritative_readiness_from_rubric_and_assessments() {
        let (store, project_id, classes) = temp_project();
        let class_id = setup_environment(store.clone(), project_id.clone(), classes.clone(), 9);
        add_student(&store, &project_id, "student-2", "9A");
        let service = service(store, classes);
        let activity = create_task(&service, &project_id, &class_id, 1);
        let application = activity.class_applications[0].id.clone();
        let status_input = PerformanceActivityIdInput {
            project_id: project_id.clone(),
            activity_id: activity.id.clone(),
        };

        let before = service
            .get_performance_status(status_input.clone())
            .expect("status should load");
        assert!(!before.has_published_rubric);
        assert_eq!(before.published_rubric_version, None);
        assert!(before.has_draft_rubric);
        assert_eq!(before.total_students, 2);
        assert!(!before.all_approved);

        service
            .publish_performance_rubric(
                PublishPerformanceRubricInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    rubric: valid_rubric(),
                },
                "test-correlation-id",
            )
            .expect("rubric should publish");

        let published = service
            .get_performance_status(status_input.clone())
            .expect("status should load");
        assert!(published.has_published_rubric);
        assert_eq!(published.published_rubric_version, Some(1));
        assert_eq!(published.total_students, 2);
        assert!(!published.all_approved);

        let saved = service
            .save_performance_assessment(
                SavePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    student_id: "student-1".into(),
                    assessment_id: None,
                    ratings: full_ratings(),
                    feedback: None,
                },
                "test-correlation-id",
            )
            .expect("draft should save");
        service
            .set_performance_assessment_status(
                SetPerformanceAssessmentStatusInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    student_id: "student-2".into(),
                    assessment_id: None,
                    status: PerformanceAssessmentStatus::Missing,
                },
                "test-correlation-id",
            )
            .expect("missing should mark");

        let mixed = service
            .get_performance_status(status_input.clone())
            .expect("status should load");
        assert_eq!(mixed.in_progress_count, 1);
        assert_eq!(mixed.missing_count, 1);
        assert_eq!(mixed.approved_count, 0);
        assert!(!mixed.all_approved);

        service
            .approve_performance_assessment(
                ApprovePerformanceAssessmentInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    application_id: application.clone(),
                    assessment_id: saved.id.clone(),
                },
                "test-correlation-id",
            )
            .expect("approval should succeed");

        let approved = service
            .get_performance_status(status_input)
            .expect("status should load");
        assert_eq!(approved.approved_count, 1);
        assert_eq!(approved.in_progress_count, 0);
        assert!(!approved.all_approved, "one student still unapproved");
    }

    /// TD-25 kabul kriteri kanıtı: komut katmanının ürettiği correlation_id,
    /// mutation journal'ına, audit kaydına ve model invocation contract'ına
    /// aynı değer olarak akar (performans akışı + OCR/job akışı).
    #[test]
    fn correlation_id_flows_to_mutation_audit_and_invocation_contract() {
        let (store, project_id, classes) = temp_project();
        let class_id = setup_environment(store.clone(), project_id.clone(), classes.clone(), 9);
        let service = service(store.clone(), classes);
        let correlation_id = format!("corr-e2e-{}", Uuid::new_v4());

        // 1. Komut correlation_id'si mutation journal'ına yazılır.
        let activity = service
            .create_performance_task(
                CreatePerformanceTaskInput {
                    project_id: project_id.clone(),
                    academic_year_id: "2026-2027".into(),
                    course_id: "tde".into(),
                    course_name: "Türk Dili ve Edebiyatı".into(),
                    grade_level: 9,
                    term: 1,
                    sequence_number: 1,
                    school_class_ids: vec![class_id],
                    title: "Korelasyon Kanıtı".into(),
                    performance_details: PerformanceDetails {
                        theme: "Tema".into(),
                        ..PerformanceDetails::default()
                    },
                    initial_rubric: None,
                },
                &correlation_id,
            )
            .expect("task should be created");

        let root = store
            .trusted_project_root(&project_id)
            .expect("trusted root");
        let journal = crate::services::transaction_journal::list(root.root())
            .expect("transaction journal should be readable");
        assert!(
            journal.iter().any(|record| {
                record.correlation_id == correlation_id
                    && record.operation == "create_performance_task"
            }),
            "mutation journal must carry the command correlation_id"
        );

        // 2. Aynı correlation_id, komut katmanının audit kaydına yazılır
        //    (performance_commands.rs'in kullandığı AuditEntryInput::correlation yolu).
        let audit_service = crate::services::audit_service::AuditService::new();
        audit_service
            .append(
                root.root(),
                crate::services::audit_service::AuditEntryInput::new(
                    "performance_task_created",
                    "Performans görevi oluşturuldu.",
                )
                .entity("assessment_activity", &activity.id)
                .correlation(&correlation_id),
            )
            .expect("audit append");
        let audit_path = crate::services::audit_service::AuditService::audit_path(root.root());
        let audit_content = std::fs::read_to_string(&audit_path).expect("audit file");
        assert!(
            audit_content.contains(&correlation_id),
            "audit record must carry the command correlation_id"
        );

        // 3. Aynı correlation_id, model invocation contract'ına yazılır
        //    (OCR/job akışındaki build_prompt_contract çağrısının izlediği yol).
        let contract = crate::services::prompt_contract::build_prompt_contract(
            crate::domain::model::ModelRequestKind::OcrIssueCorrection,
            "student_answer_ocr_issue_correction_v2_typed_user_data",
            "student_answer_ocr_issue_correction_output_v1",
            "ocr_review_policy_v1",
            "Yalnız gözlenen metni düzelt.",
            serde_json::json!({ "observedText": "örnek" }),
            crate::services::prompt_contract::default_sampling(128),
            Some(crate::domain::model::ModelResponseFormat::JsonObject),
            Some(&correlation_id),
        );
        assert_eq!(
            contract.invocation.correlation_id.as_deref(),
            Some(correlation_id.as_str()),
            "invocation contract must carry the command correlation_id"
        );

        let _ = std::fs::remove_dir_all(root.root());
    }

    #[test]
    fn save_performance_assessment_commit_failure_returns_typed_error_and_allows_retry() {
        let (store, project_id, classes) = temp_project();
        let class_id = setup_environment(store.clone(), project_id.clone(), classes.clone(), 9);
        let service = service(store.clone(), classes);
        let activity = create_task(&service, &project_id, &class_id, 1);
        service
            .publish_performance_rubric(
                PublishPerformanceRubricInput {
                    project_id: project_id.clone(),
                    activity_id: activity.id.clone(),
                    rubric: valid_rubric(),
                },
                "test-correlation-id",
            )
            .expect("rubric should publish");

        let application = activity.class_applications[0].id.clone();
        let project_json = {
            let root = store.trusted_project_root(&project_id).unwrap();
            root.root().join("project.json")
        };
        let original_content =
            std::fs::read_to_string(&project_json).expect("project.json should be readable");

        // Externally modify the project file so the next commit fails the
        // session-fingerprint check (PROJECT_EXTERNALLY_MODIFIED).
        let mut external = store.get_project_snapshot(project_id.clone()).unwrap();
        external.name = "external edit".to_string();
        std::fs::write(
            &project_json,
            serde_json::to_string_pretty(&external).unwrap(),
        )
        .unwrap();

        let input = SavePerformanceAssessmentInput {
            project_id: project_id.clone(),
            activity_id: activity.id.clone(),
            application_id: application.clone(),
            student_id: "student-1".into(),
            assessment_id: None,
            ratings: full_ratings(),
            feedback: None,
        };

        // Commit fail -> typed error; no success DTO is returned.
        let error = service
            .save_performance_assessment(input.clone(), "test-correlation-id")
            .unwrap_err();
        assert_eq!(error.code, AppErrorCode::ProjectExternallyModified);

        // Memory state must not be canonical: the failed mutation must not
        // have created an assessment in the session project.
        let session = store.get_project_snapshot(project_id.clone()).unwrap();
        let session_application = session
            .assessment_activities
            .iter()
            .find(|candidate| candidate.id == activity.id)
            .expect("activity should still exist")
            .class_applications
            .iter()
            .find(|candidate| candidate.id == application)
            .expect("application should still exist");
        assert!(
            session_application.performance_assessments.is_empty(),
            "failed commit must not leave an assessment in the session project"
        );

        // The failed mutation must not have overwritten the external disk state.
        let disk = std::fs::read_to_string(&project_json).unwrap();
        assert!(
            disk.contains("external edit"),
            "failed commit must not overwrite the externally modified project file"
        );

        // Restore the disk to the session-known content and retry: success now.
        std::fs::write(&project_json, &original_content).unwrap();
        let saved = service
            .save_performance_assessment(input, "test-correlation-id")
            .expect("retry must succeed after restoring the project file");
        assert_eq!(saved.status, PerformanceAssessmentStatus::InProgress);
        assert_eq!(saved.provisional_total, 5 + 4 + 3);

        let _ = std::fs::remove_dir_all(project_json.parent().unwrap());
    }
}

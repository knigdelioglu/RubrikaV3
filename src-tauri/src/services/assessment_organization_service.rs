use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::assessment::{
    AssessmentActivity, AssessmentClassApplication, AssessmentStatus, AssessmentType,
    ClassApplication, ClassApplicationStatus, ListeningDetails, SpeakingConfigurationSnapshot,
};
use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::performance::PerformanceDetails;
use crate::domain::project::Project;
use crate::domain::student::Student;
use crate::services::project_store::ProjectStore;
use crate::services::school_class_service::{
    students_for_class, ListAssessmentClassesInput, SchoolClassService,
};

#[derive(Clone)]
pub struct AssessmentOrganizationService {
    project_store: ProjectStore,
    class_section_service: Arc<SchoolClassService>,
    mutation_lock: Arc<Mutex<()>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAssessmentActivitiesInput {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub academic_year_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub course_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grade_level: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub term: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessment_type: Option<AssessmentType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<AssessmentStatus>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAssessmentSequenceOptionsInput {
    pub project_id: String,
    pub academic_year_id: String,
    pub course_id: String,
    pub term: u8,
    pub assessment_type: AssessmentType,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentSequenceOptions {
    pub options: Vec<u32>,
    pub suggested: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAssessmentActivityInput {
    pub project_id: String,
    pub academic_year_id: String,
    pub course_id: String,
    pub course_name: String,
    pub grade_level: u32,
    pub term: u8,
    pub assessment_type: AssessmentType,
    pub sequence_number: u32,
    #[serde(alias = "classSectionIds")]
    pub school_class_ids: Vec<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaking_configuration: Option<SpeakingConfigurationSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listening_details: Option<ListeningDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performance_details: Option<PerformanceDetails>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddAssessmentClassApplicationInput {
    pub project_id: String,
    pub activity_id: String,
    #[serde(alias = "classSectionId")]
    pub school_class_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentActivityIdInput {
    pub project_id: String,
    pub activity_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassApplicationIdInput {
    pub project_id: String,
    pub activity_id: String,
    pub application_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetClassApplicationStudentsInput {
    pub project_id: String,
    pub activity_id: String,
    pub application_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAssessmentActivityInput {
    pub project_id: String,
    pub activity_id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub speaking_configuration: Option<SpeakingConfigurationSnapshot>,
    #[serde(default)]
    pub status: Option<AssessmentStatus>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentClassApplicationIdInput {
    pub project_id: String,
    pub activity_id: String,
    pub application_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachAssessmentDocumentInput {
    pub project_id: String,
    pub activity_id: String,
    pub document_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_id: Option<String>,
}

impl AssessmentOrganizationService {
    pub fn new(
        project_store: ProjectStore,
        class_section_service: Arc<SchoolClassService>,
    ) -> Self {
        Self {
            project_store,
            class_section_service,
            mutation_lock: Arc::new(Mutex::new(())),
        }
    }

    fn mutation_guard(&self) -> Result<MutexGuard<'_, ()>, AppError> {
        self.mutation_lock.lock().map_err(|error| {
            activity_error(
                AppErrorCode::UnknownError,
                "Sınav organizasyonu değişikliği başlatılamadı.",
                &format!("assessment organization mutation lock poisoned: {error}"),
            )
        })
    }

    pub fn list_activities(
        &self,
        input: ListAssessmentActivitiesInput,
    ) -> Result<Vec<AssessmentActivity>, AppError> {
        let project = self.load_project(&input.project_id)?;
        let mut activities = project
            .assessment_activities
            .into_iter()
            .filter(|activity| {
                input
                    .academic_year_id
                    .as_ref()
                    .map_or(true, |value| &activity.academic_year_id == value)
                    && input
                        .course_id
                        .as_ref()
                        .map_or(true, |value| &activity.course_id == value)
                    && input
                        .grade_level
                        .map_or(true, |value| activity.grade_level == value)
                    && input.term.map_or(true, |value| activity.term == value)
                    && input
                        .assessment_type
                        .map_or(true, |value| activity.assessment_type == value)
                    && input.status.map_or(true, |value| activity.status == value)
            })
            .collect::<Vec<_>>();
        activities.sort_by(|left, right| {
            left.academic_year_id
                .cmp(&right.academic_year_id)
                .then_with(|| left.grade_level.cmp(&right.grade_level))
                .then_with(|| left.term.cmp(&right.term))
                .then_with(|| left.course_name.cmp(&right.course_name))
                .then_with(|| {
                    left.assessment_type_label()
                        .cmp(right.assessment_type_label())
                })
                .then_with(|| left.sequence_number.cmp(&right.sequence_number))
        });
        Ok(activities)
    }

    pub fn sequence_options(
        &self,
        input: GetAssessmentSequenceOptionsInput,
    ) -> Result<AssessmentSequenceOptions, AppError> {
        let project = self.load_project(&input.project_id)?;
        let used = project
            .assessment_activities
            .iter()
            .filter(|activity| {
                activity.academic_year_id == input.academic_year_id
                    && activity.course_id == input.course_id
                    && activity.term == input.term
                    && activity.assessment_type == input.assessment_type
            })
            .map(|activity| activity.sequence_number)
            .collect::<HashSet<_>>();
        let suggested_slots = match input.assessment_type {
            AssessmentType::Listening => vec![1],
            AssessmentType::Written | AssessmentType::Speaking | AssessmentType::Performance => {
                vec![1, 2]
            }
        };
        let mut options = suggested_slots
            .into_iter()
            .filter(|slot| !used.contains(slot))
            .collect::<Vec<_>>();
        let suggested = if let Some(slot) = options.first().copied() {
            slot
        } else {
            let next = used.iter().copied().max().unwrap_or(0).saturating_add(1);
            options.push(next);
            next
        };
        Ok(AssessmentSequenceOptions { options, suggested })
    }

    pub fn create_activity(
        &self,
        input: CreateAssessmentActivityInput,
    ) -> Result<AssessmentActivity, AppError> {
        let _mutation_guard = self.mutation_guard()?;
        validate_create_input(&input)?;
        let mut project = self.load_project(&input.project_id)?;
        let activity = self.create_activity_in_project(&mut project, &input)?;
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(activity)
    }

    /// Builds the canonical activity and class applications in the supplied project.
    /// The caller owns the single atomic ProjectStore save.
    pub fn create_activity_in_project(
        &self,
        project: &mut Project,
        input: &CreateAssessmentActivityInput,
    ) -> Result<AssessmentActivity, AppError> {
        validate_create_input(input)?;
        if project.assessment_activities.iter().any(|activity| {
            activity.academic_year_id == input.academic_year_id
                && activity.course_id == input.course_id
                && activity.grade_level == input.grade_level
                && activity.term == input.term
                && activity.assessment_type == input.assessment_type
                && activity.sequence_number == input.sequence_number
        }) {
            return Err(activity_error(
                AppErrorCode::AssessmentActivityAlreadyExists,
                "Bu dönem ve sınav türü için aynı sıra numarası zaten kullanılıyor.",
                "Aynı ana sınav ikinci kez oluşturulamaz.",
            ));
        }

        let eligible =
            self.class_section_service
                .list_assessment_classes(ListAssessmentClassesInput {
                    project_id: input.project_id.clone(),
                    academic_year_id: input.academic_year_id.clone(),
                    course_id: input.course_id.clone(),
                    grade_level: input.grade_level,
                })?;
        let eligible_ids = eligible
            .iter()
            .map(|school_class| school_class.id.as_str())
            .collect::<HashSet<_>>();
        let selected_ids = input
            .school_class_ids
            .iter()
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .collect::<Vec<_>>();
        let selected_set = selected_ids.iter().collect::<HashSet<_>>();
        if selected_ids.len() != selected_set.len() {
            return Err(activity_error(
                AppErrorCode::AssessmentClassApplicationAlreadyExists,
                "Aynı sınıf bu sınava birden fazla kez seçilemez.",
                "school_class_ids contains duplicates.",
            ));
        }
        let selected_classes = project
            .school_classes
            .iter()
            .filter(|school_class| selected_ids.iter().any(|id| id == &school_class.id))
            .collect::<Vec<_>>();
        if selected_classes
            .iter()
            .any(|school_class| school_class.grade_level != Some(input.grade_level))
        {
            return Err(activity_error(
                AppErrorCode::AssessmentClassLevelMismatch,
                "Aynı sınava farklı sınıf düzeyleri seçilemez.",
                "selected school classes do not share the requested grade level.",
            ));
        }
        if selected_ids.is_empty()
            || selected_ids
                .iter()
                .any(|id| !eligible_ids.contains(id.as_str()))
        {
            return Err(activity_error(
                AppErrorCode::AssessmentClassNotEligible,
                "Seçilen sınıflardan biri bu ders için görevlendirilmemiş veya pasif.",
                "Only active teacher assignments are allowed.",
            ));
        }

        let now = chrono::Utc::now().to_rfc3339();
        let activity_id = Uuid::new_v4().to_string();
        let class_applications = selected_ids
            .into_iter()
            .map(|school_class_id| {
                let student_scope_ids = students_for_class(project, &school_class_id)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|student| student.id)
                    .collect();
                ClassApplication {
                    id: Uuid::new_v4().to_string(),
                    activity_id: activity_id.clone(),
                    school_class_id,
                    scheduled_at: None,
                    application_date: None,
                    status: ClassApplicationStatus::Scheduled,
                    notes: None,
                    document_ids: vec![],
                    student_scope_ids,
                    speaking_attempts: vec![],
                    performance_assessments: vec![],
                    created_at: now.clone(),
                    updated_at: now.clone(),
                }
            })
            .collect();
        let activity = AssessmentActivity {
            id: activity_id,
            academic_year_id: input.academic_year_id.clone(),
            course_id: input.course_id.clone(),
            course_name: input.course_name.trim().to_string(),
            title: input.title.trim().to_string(),
            grade_level: input.grade_level,
            term: input.term,
            workflow_family: input.assessment_type.workflow_family(),
            assessment_type: input.assessment_type,
            sequence_number: input.sequence_number,
            status: AssessmentStatus::Draft,
            common_document_ids: vec![],
            listening_details: input.listening_details.clone(),
            speaking_configuration: input.speaking_configuration.clone(),
            performance_details: input.performance_details.clone(),
            class_applications,
            created_at: now.clone(),
            updated_at: now,
        };
        project.assessment_activities.push(activity.clone());
        Ok(activity)
    }

    pub fn get_activity(
        &self,
        input: AssessmentActivityIdInput,
    ) -> Result<AssessmentActivity, AppError> {
        let project = self.load_project(&input.project_id)?;
        project
            .assessment_activities
            .into_iter()
            .find(|activity| activity.id == input.activity_id)
            .ok_or_else(|| {
                activity_error(
                    AppErrorCode::AssessmentActivityNotFound,
                    "Sınav bulunamadı.",
                    "activity_id not found.",
                )
            })
    }

    pub fn get_class_applications(
        &self,
        input: AssessmentActivityIdInput,
    ) -> Result<Vec<ClassApplication>, AppError> {
        Ok(self.get_activity(input)?.class_applications)
    }

    pub fn get_class_application_students(
        &self,
        input: GetClassApplicationStudentsInput,
    ) -> Result<Vec<Student>, AppError> {
        let project = self.load_project(&input.project_id)?;
        let activity = project
            .assessment_activities
            .iter()
            .find(|activity| activity.id == input.activity_id)
            .ok_or_else(|| {
                activity_error(
                    AppErrorCode::AssessmentActivityNotFound,
                    "Sınav bulunamadı.",
                    "activity_id not found.",
                )
            })?;
        let application = activity
            .class_applications
            .iter()
            .find(|application| application.id == input.application_id)
            .ok_or_else(|| {
                activity_error(
                    AppErrorCode::AssessmentClassApplicationNotFound,
                    "Sınıf uygulaması bulunamadı.",
                    "application_id not found for activity.",
                )
            })?;
        students_for_class(&project, &application.school_class_id)
    }

    pub fn update_activity(
        &self,
        input: UpdateAssessmentActivityInput,
    ) -> Result<AssessmentActivity, AppError> {
        let _mutation_guard = self.mutation_guard()?;
        let mut project = self.load_project(&input.project_id)?;
        let activity = project
            .assessment_activities
            .iter_mut()
            .find(|activity| activity.id == input.activity_id)
            .ok_or_else(|| {
                activity_error(
                    AppErrorCode::AssessmentActivityNotFound,
                    "Sınav bulunamadı.",
                    "activity_id not found.",
                )
            })?;
        let has_attempts = activity
            .class_applications
            .iter()
            .any(|application| !application.speaking_attempts.is_empty());
        if has_attempts && input.speaking_configuration.is_some() {
            return Err(activity_error(
                AppErrorCode::AssessmentActivityInUse,
                "Konuşma kaydı bulunan sınavın ortak görev ve süre politikası değiştirilemez.",
                "activity speaking configuration is frozen after attempts.",
            ));
        }
        if let Some(title) = input.title {
            activity.title = title.trim().to_string();
        }
        if input.speaking_configuration.is_some() {
            activity.speaking_configuration = input.speaking_configuration;
        }
        if let Some(status) = input.status {
            activity.status = status;
        }
        activity.workflow_family = activity.assessment_type.workflow_family();
        activity.updated_at = chrono::Utc::now().to_rfc3339();
        let updated = activity.clone();
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(updated)
    }

    pub fn add_class_application(
        &self,
        input: AddAssessmentClassApplicationInput,
    ) -> Result<AssessmentClassApplication, AppError> {
        let _mutation_guard = self.mutation_guard()?;
        let mut project = self.load_project(&input.project_id)?;
        let activity = project
            .assessment_activities
            .iter()
            .find(|activity| activity.id == input.activity_id)
            .cloned()
            .ok_or_else(|| {
                activity_error(
                    AppErrorCode::AssessmentActivityNotFound,
                    "Sınav bulunamadı.",
                    "activity_id not found.",
                )
            })?;
        if activity
            .class_applications
            .iter()
            .any(|application| application.school_class_id == input.school_class_id)
        {
            return Err(activity_error(
                AppErrorCode::AssessmentClassApplicationAlreadyExists,
                "Bu sınıf zaten bu sınava bağlı.",
                "class application uniqueness constraint.",
            ));
        }
        let eligible =
            self.class_section_service
                .list_assessment_classes(ListAssessmentClassesInput {
                    project_id: input.project_id.clone(),
                    academic_year_id: activity.academic_year_id.clone(),
                    course_id: activity.course_id.clone(),
                    grade_level: activity.grade_level,
                })?;
        if !eligible
            .iter()
            .any(|school_class| school_class.id == input.school_class_id)
        {
            return Err(activity_error(
                AppErrorCode::AssessmentClassNotEligible,
                "Bu sınıf bu sınava eklenemez.",
                "class is not an active assignment for the assessment.",
            ));
        }
        let now = chrono::Utc::now().to_rfc3339();
        let student_scope_ids = students_for_class(&project, &input.school_class_id)
            .unwrap_or_default()
            .into_iter()
            .map(|student| student.id)
            .collect();
        let application = ClassApplication {
            id: Uuid::new_v4().to_string(),
            activity_id: activity.id.clone(),
            school_class_id: input.school_class_id,
            scheduled_at: normalize_optional(input.scheduled_at),
            application_date: normalize_optional(input.application_date),
            status: ClassApplicationStatus::Scheduled,
            notes: normalize_optional(input.notes),
            document_ids: vec![],
            student_scope_ids,
            speaking_attempts: vec![],
            performance_assessments: vec![],
            created_at: now.clone(),
            updated_at: now,
        };
        let stored_activity = project
            .assessment_activities
            .iter_mut()
            .find(|candidate| candidate.id == activity.id)
            .ok_or_else(|| {
                activity_error(
                    AppErrorCode::AssessmentActivityNotFound,
                    "Sınav bulunamadı.",
                    "activity disappeared during update.",
                )
            })?;
        stored_activity.class_applications.push(application.clone());
        stored_activity.updated_at = chrono::Utc::now().to_rfc3339();
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(application)
    }

    pub fn archive_class_application(
        &self,
        input: AssessmentClassApplicationIdInput,
    ) -> Result<AssessmentClassApplication, AppError> {
        let _mutation_guard = self.mutation_guard()?;
        let mut project = self.load_project(&input.project_id)?;
        let activity = project
            .assessment_activities
            .iter_mut()
            .find(|activity| activity.id == input.activity_id)
            .ok_or_else(|| {
                activity_error(
                    AppErrorCode::AssessmentActivityNotFound,
                    "Sınav bulunamadı.",
                    "activity_id not found.",
                )
            })?;
        let application = activity
            .class_applications
            .iter_mut()
            .find(|application| application.id == input.application_id)
            .ok_or_else(|| {
                activity_error(
                    AppErrorCode::AssessmentClassApplicationNotFound,
                    "Sınıf uygulaması bulunamadı.",
                    "application_id not found.",
                )
            })?;
        application.status = ClassApplicationStatus::Archived;
        application.updated_at = chrono::Utc::now().to_rfc3339();
        let archived = application.clone();
        activity.updated_at = chrono::Utc::now().to_rfc3339();
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(archived)
    }

    pub fn remove_class_application(
        &self,
        input: ClassApplicationIdInput,
    ) -> Result<ClassApplication, AppError> {
        let _mutation_guard = self.mutation_guard()?;
        let mut project = self.load_project(&input.project_id)?;
        let activity = project
            .assessment_activities
            .iter_mut()
            .find(|activity| activity.id == input.activity_id)
            .ok_or_else(|| {
                activity_error(
                    AppErrorCode::AssessmentActivityNotFound,
                    "Sınav bulunamadı.",
                    "activity_id not found.",
                )
            })?;
        let index = activity
            .class_applications
            .iter()
            .position(|application| application.id == input.application_id)
            .ok_or_else(|| {
                activity_error(
                    AppErrorCode::AssessmentClassApplicationNotFound,
                    "Sınıf uygulaması bulunamadı.",
                    "application_id not found.",
                )
            })?;
        let application = &activity.class_applications[index];
        let has_attempts = !application.speaking_attempts.is_empty()
            || project.speaking_exams.iter().any(|exam| {
                exam.assessment_activity_id.as_deref() == Some(&activity.id)
                    && exam.attempts.iter().any(|attempt| {
                        attempt.class_application_id.as_deref() == Some(&application.id)
                    })
            });
        if has_attempts {
            return Err(activity_error(
                AppErrorCode::AssessmentClassApplicationInUse,
                "Bu sınıf uygulamasında konuşma kayıtları bulunduğu için kaldırılamaz.",
                "class application has attempts or artifacts.",
            ));
        }
        let removed = activity.class_applications.remove(index);
        activity.updated_at = chrono::Utc::now().to_rfc3339();
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(removed)
    }

    pub fn attach_document(
        &self,
        input: AttachAssessmentDocumentInput,
    ) -> Result<AssessmentActivity, AppError> {
        let _mutation_guard = self.mutation_guard()?;
        let mut project = self.load_project(&input.project_id)?;
        if !project
            .documents
            .iter()
            .any(|document| document.id == input.document_id)
        {
            return Err(activity_error(
                AppErrorCode::AssessmentDocumentNotFound,
                "Belge bulunamadı.",
                "document_id not found.",
            ));
        }
        let activity = project
            .assessment_activities
            .iter_mut()
            .find(|activity| activity.id == input.activity_id)
            .ok_or_else(|| {
                activity_error(
                    AppErrorCode::AssessmentActivityNotFound,
                    "Sınav bulunamadı.",
                    "activity_id not found.",
                )
            })?;
        if let Some(application_id) = input.application_id {
            let application = activity
                .class_applications
                .iter_mut()
                .find(|application| application.id == application_id)
                .ok_or_else(|| {
                    activity_error(
                        AppErrorCode::AssessmentClassApplicationNotFound,
                        "Sınıf uygulaması bulunamadı.",
                        "application_id not found.",
                    )
                })?;
            if !application.document_ids.contains(&input.document_id) {
                application.document_ids.push(input.document_id);
            }
            application.updated_at = chrono::Utc::now().to_rfc3339();
        } else if !activity.common_document_ids.contains(&input.document_id) {
            activity.common_document_ids.push(input.document_id);
        }
        activity.updated_at = chrono::Utc::now().to_rfc3339();
        let updated = activity.clone();
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(updated)
    }

    fn load_project(&self, project_id: &str) -> Result<Project, AppError> {
        self.project_store
            .get_project_snapshot(project_id.to_string())
    }
}

fn validate_create_input(input: &CreateAssessmentActivityInput) -> Result<(), AppError> {
    if input.academic_year_id.trim().is_empty()
        || input.course_id.trim().is_empty()
        || input.course_name.trim().is_empty()
        || input.grade_level == 0
        || !(1..=2).contains(&input.term)
        || input.sequence_number == 0
    {
        return Err(activity_error(
            AppErrorCode::AssessmentInvalidInput,
            "Sınav için eğitim yılı, ders, sınıf düzeyi, dönem ve sıra bilgileri geçerli olmalıdır.",
            "invalid assessment activity key.",
        ));
    }
    match input.assessment_type {
        AssessmentType::Performance => {
            if input.performance_details.is_none() {
                return Err(activity_error(
                    AppErrorCode::AssessmentInvalidInput,
                    "Performans görevi için görev ayrıntıları gereklidir.",
                    "performance_details is missing.",
                ));
            }
            if input.speaking_configuration.is_some() || input.listening_details.is_some() {
                return Err(activity_error(
                    AppErrorCode::AssessmentInvalidInput,
                    "Performans görevinde konuşma/dinleme ayarları kullanılamaz.",
                    "speaking/listening details supplied for performance activity.",
                ));
            }
        }
        AssessmentType::Speaking => {
            let Some(configuration) = input.speaking_configuration.as_ref() else {
                return Err(activity_error(
                    AppErrorCode::AssessmentInvalidInput,
                    "Konuşma sınavı için konuşma ayarları gereklidir.",
                    "speaking_configuration is missing.",
                ));
            };
            if !matches!(
                configuration.speaking_type.as_str(),
                "prepared" | "impromptu"
            ) || configuration.task_text.trim().is_empty()
                || configuration.min_duration_seconds == 0
                || configuration.target_duration_seconds == 0
                || configuration.max_duration_seconds == 0
                || configuration.min_duration_seconds > configuration.target_duration_seconds
                || configuration.target_duration_seconds > configuration.max_duration_seconds
            {
                return Err(activity_error(
                    AppErrorCode::AssessmentInvalidInput,
                    "Konuşma türü, görev metni ve süre aralığı geçerli olmalıdır.",
                    "invalid speaking configuration.",
                ));
            }
            if input.performance_details.is_some() {
                return Err(activity_error(
                    AppErrorCode::AssessmentInvalidInput,
                    "Performans görev ayrıntıları yalnız performans görevinde kullanılabilir.",
                    "performance_details supplied for non-performance activity.",
                ));
            }
        }
        AssessmentType::Written | AssessmentType::Listening => {
            if input.speaking_configuration.is_some() {
                return Err(activity_error(
                    AppErrorCode::AssessmentInvalidInput,
                    "Konuşma ayarları yalnız konuşma sınavında kullanılabilir.",
                    "speaking_configuration supplied for non-speaking activity.",
                ));
            }
            if input.performance_details.is_some() {
                return Err(activity_error(
                    AppErrorCode::AssessmentInvalidInput,
                    "Performans görev ayrıntıları yalnız performans görevinde kullanılabilir.",
                    "performance_details supplied for non-performance activity.",
                ));
            }
            if let Some(details) = input.listening_details.as_ref() {
                if details.play_count == Some(0) || details.duration_seconds == Some(0) {
                    return Err(activity_error(
                        AppErrorCode::AssessmentInvalidInput,
                        "Dinleme süresi ve dinletme sayısı sıfır olamaz.",
                        "invalid listening details.",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn activity_error(code: AppErrorCode, message: &str, technical_details: &str) -> AppError {
    AppError {
        code,
        message: message.to_string(),
        recoverable: true,
        suggested_action: Some(
            "Sınav organizasyonu bilgilerini kontrol edip tekrar deneyin.".to_string(),
        ),
        technical_details: Some(technical_details.to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    }
}

trait AssessmentTypeLabel {
    fn assessment_type_label(&self) -> &'static str;
}

impl AssessmentTypeLabel for AssessmentActivity {
    fn assessment_type_label(&self) -> &'static str {
        match self.assessment_type {
            AssessmentType::Written => "written",
            AssessmentType::Listening => "listening",
            AssessmentType::Speaking => "speaking",
            AssessmentType::Performance => "performance",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::services::school_class_service::{
        CreateSchoolClassInput, CreateTeachingAssignmentInput, ListSchoolClassesInput,
    };

    fn temp_project() -> (ProjectStore, String, SchoolClassService) {
        let root = std::env::temp_dir().join(format!("rubrika-assessment-{}", Uuid::new_v4()));
        let store = ProjectStore::new();
        let project = store
            .create_project(
                "Assessment organization".to_string(),
                root.to_string_lossy().to_string(),
            )
            .expect("test project should be created");
        let classes = SchoolClassService::new(store.clone());
        (store, project.id, classes)
    }

    #[test]
    fn one_activity_contains_three_class_applications() {
        let (store, project_id, classes) = temp_project();
        for section in ["A", "B", "C"] {
            classes
                .create_school_class(CreateSchoolClassInput {
                    project_id: project_id.clone(),
                    name: format!("10{section}"),
                    academic_year: Some("2026-2027".into()),
                    grade_level: Some(10),
                    section: Some(section.into()),
                    display_order: None,
                })
                .expect("class should be created");
        }
        let listed = classes
            .list_school_classes(ListSchoolClassesInput {
                project_id: project_id.clone(),
                include_archived: false,
            })
            .expect("classes should list");
        for school_class in &listed {
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
        }
        let service = AssessmentOrganizationService::new(store.clone(), Arc::new(classes));
        let activity = service
            .create_activity(CreateAssessmentActivityInput {
                project_id: project_id.clone(),
                academic_year_id: "2026-2027".into(),
                course_id: "tde".into(),
                course_name: "Türk Dili ve Edebiyatı".into(),
                title: "1. Yazılı".into(),
                grade_level: 10,
                term: 1,
                assessment_type: AssessmentType::Written,
                sequence_number: 1,
                school_class_ids: listed.iter().map(|item| item.id.clone()).collect(),
                speaking_configuration: None,
                listening_details: None,
                performance_details: None,
            })
            .expect("activity should be created");
        assert_eq!(activity.class_applications.len(), 3);
        let persisted = store
            .get_project_snapshot(project_id)
            .expect("project should load");
        assert_eq!(persisted.assessment_activities.len(), 1);
    }

    #[test]
    fn sequence_is_scoped_by_type_and_archived_classes_are_not_reused() {
        let (store, project_id, classes) = temp_project();
        let school_class = classes
            .create_school_class(CreateSchoolClassInput {
                project_id: project_id.clone(),
                name: "10A".into(),
                academic_year: Some("2026-2027".into()),
                grade_level: Some(10),
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
        let service = AssessmentOrganizationService::new(store.clone(), Arc::new(classes.clone()));
        let input = |assessment_type| CreateAssessmentActivityInput {
            project_id: project_id.clone(),
            academic_year_id: "2026-2027".into(),
            course_id: "tde".into(),
            course_name: "Türk Dili ve Edebiyatı".into(),
            title: "Sınav".into(),
            grade_level: 10,
            term: 1,
            assessment_type,
            sequence_number: 1,
            school_class_ids: vec![school_class.id.clone()],
            speaking_configuration: (assessment_type == AssessmentType::Speaking).then(|| {
                SpeakingConfigurationSnapshot {
                    speaking_type: "prepared".into(),
                    task_text: "Bir anını anlat.".into(),
                    target_duration_seconds: 180,
                    min_duration_seconds: 120,
                    max_duration_seconds: 240,
                    rubric_version: "rubric-v1".into(),
                    scoring_policy_version: "policy-v1".into(),
                    cleanup_prompt_version: "cleanup-v1".into(),
                    evaluation_prompt_version: "evaluation-v1".into(),
                    frozen_model_file_hash: None,
                    rubric_snapshot: serde_json::json!({}),
                }
            }),
            listening_details: None,
            performance_details: None,
        };
        service
            .create_activity(input(AssessmentType::Written))
            .expect("written activity should be created");
        assert_eq!(
            service
                .create_activity(input(AssessmentType::Written))
                .unwrap_err()
                .code,
            AppErrorCode::AssessmentActivityAlreadyExists
        );
        service
            .create_activity(input(AssessmentType::Listening))
            .expect("listening sequence should be independent");
        service
            .create_activity(input(AssessmentType::Speaking))
            .expect("speaking sequence should be independent");
        classes
            .archive_school_class(crate::services::school_class_service::SchoolClassIdInput {
                project_id: project_id.clone(),
                class_id: school_class.id.clone(),
            })
            .expect("class should archive");
        assert!(classes
            .list_assessment_classes(ListAssessmentClassesInput {
                project_id: project_id.clone(),
                academic_year_id: "2026-2027".into(),
                course_id: "tde".into(),
                grade_level: 10,
            })
            .expect("eligible classes should list")
            .is_empty());
        let persisted = store
            .get_project_snapshot(project_id)
            .expect("project should load");
        assert_eq!(persisted.assessment_activities.len(), 3);
        assert_eq!(
            persisted.assessment_activities[0].class_applications.len(),
            1
        );
    }

    #[test]
    fn sequence_options_are_calculated_from_existing_activity_keys() {
        let (store, project_id, classes) = temp_project();
        let school_class = classes
            .create_school_class(CreateSchoolClassInput {
                project_id: project_id.clone(),
                name: "10A".into(),
                academic_year: Some("2026-2027".into()),
                grade_level: Some(10),
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
        let service = AssessmentOrganizationService::new(store, Arc::new(classes));
        for sequence_number in [1, 2] {
            service
                .create_activity(CreateAssessmentActivityInput {
                    project_id: project_id.clone(),
                    academic_year_id: "2026-2027".into(),
                    course_id: "tde".into(),
                    course_name: "Türk Dili ve Edebiyatı".into(),
                    title: format!("{sequence_number}. Yazılı"),
                    grade_level: 10,
                    term: 1,
                    assessment_type: AssessmentType::Written,
                    sequence_number,
                    school_class_ids: vec![school_class.id.clone()],
                    speaking_configuration: None,
                    listening_details: None,
                    performance_details: None,
                })
                .expect("activity should be created");
        }
        let options = service
            .sequence_options(GetAssessmentSequenceOptionsInput {
                project_id,
                academic_year_id: "2026-2027".into(),
                course_id: "tde".into(),
                term: 1,
                assessment_type: AssessmentType::Written,
            })
            .expect("sequence options should be calculated");
        assert_eq!(options.options, vec![3]);
        assert_eq!(options.suggested, 3);
    }

    #[test]
    fn create_rejects_invalid_speaking_configuration_before_persistence() {
        let input = CreateAssessmentActivityInput {
            project_id: "project".into(),
            academic_year_id: "2026-2027".into(),
            course_id: "tde".into(),
            course_name: "Türk Dili ve Edebiyatı".into(),
            title: "Konuşma".into(),
            grade_level: 10,
            term: 1,
            assessment_type: AssessmentType::Speaking,
            sequence_number: 1,
            school_class_ids: vec!["class".into()],
            speaking_configuration: None,
            listening_details: None,
            performance_details: None,
        };
        assert_eq!(
            validate_create_input(&input).unwrap_err().code,
            AppErrorCode::AssessmentInvalidInput
        );
    }

    #[test]
    fn class_application_with_speaking_attempt_cannot_be_removed() {
        let (store, project_id, classes) = temp_project();
        let school_class = classes
            .create_school_class(CreateSchoolClassInput {
                project_id: project_id.clone(),
                name: "11A".into(),
                academic_year: Some("2026-2027".into()),
                grade_level: Some(11),
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
        let service = AssessmentOrganizationService::new(store.clone(), Arc::new(classes));
        let activity = service
            .create_activity(CreateAssessmentActivityInput {
                project_id: project_id.clone(),
                academic_year_id: "2026-2027".into(),
                course_id: "tde".into(),
                course_name: "Türk Dili ve Edebiyatı".into(),
                title: "1. Konuşma".into(),
                grade_level: 11,
                term: 1,
                assessment_type: AssessmentType::Speaking,
                sequence_number: 1,
                school_class_ids: vec![school_class.id.clone()],
                speaking_configuration: Some(SpeakingConfigurationSnapshot {
                    speaking_type: "prepared".into(),
                    task_text: "Bir anını anlat.".into(),
                    target_duration_seconds: 180,
                    min_duration_seconds: 120,
                    max_duration_seconds: 240,
                    rubric_version: "rubric-v1".into(),
                    scoring_policy_version: "policy-v1".into(),
                    cleanup_prompt_version: "cleanup-v1".into(),
                    evaluation_prompt_version: "evaluation-v1".into(),
                    frozen_model_file_hash: None,
                    rubric_snapshot: serde_json::json!({}),
                }),
                listening_details: None,
                performance_details: None,
            })
            .expect("speaking activity should be created");
        let mut project = store
            .get_project_snapshot(project_id.clone())
            .expect("project should load");
        project.assessment_activities[0].class_applications[0]
            .speaking_attempts
            .push(
                serde_json::from_value(serde_json::json!({
                    "id": "attempt-1",
                    "assessmentActivityId": activity.id,
                    "classApplicationId": activity.class_applications[0].id,
                    "schoolClassId": school_class.id,
                    "examId": activity.id,
                    "studentId": "student-1",
                    "attemptNumber": 1,
                    "state": "teacher_review",
                    "startedAt": "2026-01-01T00:00:00Z"
                }))
                .expect("attempt should deserialize"),
            );
        store.save_project(&project).expect("project should save");

        let result = service.remove_class_application(ClassApplicationIdInput {
            project_id,
            activity_id: activity.id,
            application_id: activity.class_applications[0].id.clone(),
        });
        assert_eq!(
            result.unwrap_err().code,
            AppErrorCode::AssessmentClassApplicationInUse
        );
    }
}

use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::assessment::TeachingAssignment;
use crate::domain::document::{Document, DocumentRole};
use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::project::Project;
use crate::domain::school_class::{
    normalize_school_class_name, SchoolClass, SchoolClassStatus, StudentScanBatch,
};
use crate::domain::scoring::{scoring_active_records, scoring_record_is_accepted};
use crate::domain::student::{
    student_identity_is_missing, ClassMembershipSource, PageGroupingMode, Student,
    StudentAnswerOcrStatus, StudentSubmission,
};
use crate::platform::file_access::{remove_dir_within, remove_file_within};
use crate::services::project_store::ProjectStore;
use crate::services::student_scan_service::{
    persisted_dependency_jobs, scan_submission_dependencies_with_jobs,
};
use crate::services::workflow_engine;

#[derive(Clone)]
pub struct SchoolClassService {
    project_store: ProjectStore,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSchoolClassesInput {
    pub project_id: String,
    #[serde(default)]
    pub include_archived: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSchoolClassInput {
    pub project_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub academic_year: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grade_level: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_order: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSchoolClassInput {
    pub project_id: String,
    pub class_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub academic_year: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grade_level: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_order: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchoolClassIdInput {
    pub project_id: String,
    pub class_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSchoolClassOverviewInput {
    pub project_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchoolClassOverview {
    pub school_class: SchoolClass,
    pub scan_batch_count: u32,
    pub submission_count: u32,
    pub identity_verified_count: u32,
    pub ocr_complete_count: u32,
    pub scoring_complete_count: u32,
    pub review_required_count: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchoolClassOverviewSnapshot {
    pub classes: Vec<SchoolClassOverview>,
    pub unassigned_batch_count: u32,
    pub unassigned_submission_count: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListClassStudentsInput {
    pub project_id: String,
    pub class_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateClassStudentInput {
    pub project_id: String,
    pub class_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateClassStudentInput {
    pub project_id: String,
    pub class_id: String,
    pub student_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportStudentScanBatchInput {
    pub project_id: String,
    pub class_id: String,
    pub source_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages_per_student: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grouping_mode: Option<PageGroupingMode>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportStudentScanBatchOutput {
    pub document: Document,
    pub batch: StudentScanBatch,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateStudentScanBatchInput {
    pub project_id: String,
    pub class_id: String,
    pub document_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages_per_student: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grouping_mode: Option<PageGroupingMode>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListStudentScanBatchesInput {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveStudentScanBatchInput {
    pub project_id: String,
    pub batch_id: String,
    pub target_class_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveStudentScanBatchInput {
    pub project_id: String,
    pub batch_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAssessmentClassesInput {
    pub project_id: String,
    pub academic_year_id: String,
    pub course_id: String,
    pub grade_level: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTeachingAssignmentsInput {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub academic_year_id: Option<String>,
    #[serde(default)]
    pub include_inactive: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCreateTeachingAssignmentsInput {
    pub project_id: String,
    pub academic_year_id: String,
    pub course_id: String,
    pub course_name: String,
    pub class_section_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTeachingAssignmentInput {
    pub project_id: String,
    pub academic_year_id: String,
    pub course_id: String,
    pub course_name: String,
    pub class_section_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teacher_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeachingAssignmentIdInput {
    pub project_id: String,
    pub assignment_id: String,
}

impl SchoolClassService {
    pub fn new(project_store: ProjectStore) -> Self {
        Self { project_store }
    }

    pub fn list_school_classes(
        &self,
        input: ListSchoolClassesInput,
    ) -> Result<Vec<SchoolClass>, AppError> {
        let project = self.load_project(&input.project_id)?;
        let mut classes = project
            .school_classes
            .into_iter()
            .filter(|school_class| {
                input.include_archived || school_class.status == SchoolClassStatus::Active
            })
            .collect::<Vec<_>>();
        sort_classes(&mut classes);
        Ok(classes)
    }

    /// The single source used by every assessment type when selecting classes.
    /// Assignment filtering deliberately happens here, next to the canonical class read.
    pub fn list_assessment_classes(
        &self,
        input: ListAssessmentClassesInput,
    ) -> Result<Vec<SchoolClass>, AppError> {
        let project = self.load_project(&input.project_id)?;
        let mut classes = project
            .school_classes
            .iter()
            .filter(|school_class| {
                school_class.status == SchoolClassStatus::Active
                    && class_matches_academic_year(school_class, &input.academic_year_id)
                    && school_class.grade_level == Some(input.grade_level)
                    && project.teaching_assignments.iter().any(|assignment| {
                        assignment.is_active
                            && assignment.academic_year_id == input.academic_year_id
                            && assignment.course_id == input.course_id
                            && assignment.class_section_id == school_class.id
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        sort_classes(&mut classes);
        Ok(classes)
    }

    pub fn list_teaching_assignments(
        &self,
        input: ListTeachingAssignmentsInput,
    ) -> Result<Vec<TeachingAssignment>, AppError> {
        let project = self.load_project(&input.project_id)?;
        let mut assignments = project
            .teaching_assignments
            .into_iter()
            .filter(|assignment| {
                input
                    .academic_year_id
                    .as_ref()
                    .map_or(true, |year| &assignment.academic_year_id == year)
                    && (input.include_inactive || assignment.is_active)
            })
            .collect::<Vec<_>>();
        assignments.sort_by(|left, right| {
            left.course_name
                .cmp(&right.course_name)
                .then_with(|| left.class_section_id.cmp(&right.class_section_id))
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(assignments)
    }

    pub fn batch_create_teaching_assignments(
        &self,
        input: BatchCreateTeachingAssignmentsInput,
    ) -> Result<Vec<TeachingAssignment>, AppError> {
        let mut project = self.load_project(&input.project_id)?;
        let academic_year_id = required_text(input.academic_year_id, "academic_year_id")?;
        let course_id = required_text(input.course_id, "course_id")?;
        let course_name = required_text(input.course_name, "course_name")?;

        let mut created_assignments = Vec::new();
        let now = chrono::Utc::now().to_rfc3339();

        for class_id in input.class_section_ids {
            let class_id = class_id.trim().to_string();
            if class_id.is_empty() {
                continue;
            }
            let class_index = match class_index(&project, &class_id) {
                Ok(idx) => idx,
                Err(_) => continue,
            };
            let school_class = &project.school_classes[class_index];
            if school_class.status != SchoolClassStatus::Active {
                continue;
            }

            if project.teaching_assignments.iter().any(|assignment| {
                assignment.is_active
                    && assignment.academic_year_id == academic_year_id
                    && assignment.course_id == course_id
                    && assignment.class_section_id == class_id
            }) {
                continue;
            }

            let assignment = TeachingAssignment {
                id: uuid::Uuid::new_v4().to_string(),
                academic_year_id: academic_year_id.clone(),
                course_id: course_id.clone(),
                course_name: course_name.clone(),
                class_section_id: class_id,
                teacher_id: None,
                is_active: true,
                created_at: now.clone(),
                updated_at: now.clone(),
            };
            project.teaching_assignments.push(assignment.clone());
            created_assignments.push(assignment);
        }

        if !created_assignments.is_empty() {
            self.project_store
                .commit_snapshot_cas(&project)
                .map(|_| ())?;
        }

        Ok(created_assignments)
    }

    pub fn create_teaching_assignment(
        &self,
        input: CreateTeachingAssignmentInput,
    ) -> Result<TeachingAssignment, AppError> {
        let mut project = self.load_project(&input.project_id)?;
        let academic_year_id = required_text(input.academic_year_id, "academic_year_id")?;
        let course_id = required_text(input.course_id, "course_id")?;
        let course_name = required_text(input.course_name, "course_name")?;
        let class_index = class_index(&project, &input.class_section_id)?;
        let school_class = &project.school_classes[class_index];
        if school_class.status != SchoolClassStatus::Active {
            return Err(app_error(
                AppErrorCode::SchoolClassArchived,
                "Arşivlenmiş sınıfa ders görevlendirmesi yapılamaz.",
                Some(format!("class_id={}", input.class_section_id)),
                Some("Önce sınıfı yeniden etkinleştirin.".to_string()),
            ));
        }
        if !class_matches_academic_year(school_class, &academic_year_id) {
            return Err(app_error(
                AppErrorCode::TeachingAssignmentInvalid,
                "Sınıf, seçilen eğitim yılına bağlı değil.",
                Some(format!(
                    "class_id={}; academic_year_id={academic_year_id}",
                    input.class_section_id
                )),
                Some("Kurulum → Sınıflar bölümünde aynı eğitim yılını seçin.".to_string()),
            ));
        }
        if project.teaching_assignments.iter().any(|assignment| {
            assignment.is_active
                && assignment.academic_year_id == academic_year_id
                && assignment.course_id == course_id
                && assignment.class_section_id == input.class_section_id
        }) {
            return Err(app_error(
                AppErrorCode::TeachingAssignmentAlreadyExists,
                "Bu ders ve sınıf görevlendirmesi zaten mevcut.",
                Some(format!(
                    "course_id={course_id}; class_id={}",
                    input.class_section_id
                )),
                Some("Mevcut görevlendirmeyi kullanın.".to_string()),
            ));
        }
        let now = chrono::Utc::now().to_rfc3339();
        let assignment = TeachingAssignment {
            id: Uuid::new_v4().to_string(),
            academic_year_id,
            course_id,
            course_name,
            class_section_id: input.class_section_id,
            teacher_id: normalize_optional(input.teacher_id),
            is_active: true,
            created_at: now.clone(),
            updated_at: now,
        };
        project.teaching_assignments.push(assignment.clone());
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(assignment)
    }

    pub fn archive_teaching_assignment(
        &self,
        input: TeachingAssignmentIdInput,
    ) -> Result<TeachingAssignment, AppError> {
        let mut project = self.load_project(&input.project_id)?;
        let assignment = project
            .teaching_assignments
            .iter_mut()
            .find(|assignment| assignment.id == input.assignment_id)
            .ok_or_else(|| {
                app_error(
                    AppErrorCode::TeachingAssignmentNotFound,
                    "Ders görevlendirmesi bulunamadı.",
                    Some(format!("assignment_id={}", input.assignment_id)),
                    Some("Görevlendirme listesini yenileyin.".to_string()),
                )
            })?;
        assignment.is_active = false;
        assignment.updated_at = chrono::Utc::now().to_rfc3339();
        let archived = assignment.clone();
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(archived)
    }

    pub fn create_school_class(
        &self,
        input: CreateSchoolClassInput,
    ) -> Result<SchoolClass, AppError> {
        let mut project = self.load_project(&input.project_id)?;
        let normalized_name = require_normalized_name(&input.name)?;
        ensure_active_name_unique(&project, &normalized_name, None)?;
        let now = chrono::Utc::now().to_rfc3339();
        let academic_year = normalize_optional(input.academic_year);
        let display_order = input.display_order.unwrap_or_else(|| {
            project
                .school_classes
                .iter()
                .map(|school_class| school_class.display_order)
                .max()
                .unwrap_or(0)
                .saturating_add(1)
        });
        let school_class = SchoolClass {
            id: Uuid::new_v4().to_string(),
            name: normalized_name.clone(),
            display_name: normalized_name.clone(),
            normalized_name,
            academic_year: academic_year.clone(),
            academic_year_id: academic_year,
            grade_level: input.grade_level,
            section: normalize_optional(input.section).map(|value| value.to_uppercase()),
            display_order,
            status: SchoolClassStatus::Active,
            created_at: now.clone(),
            updated_at: now,
        };
        project.school_classes.push(school_class.clone());
        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(school_class)
    }

    pub fn update_school_class(
        &self,
        input: UpdateSchoolClassInput,
    ) -> Result<SchoolClass, AppError> {
        let mut project = self.load_project(&input.project_id)?;
        let class_index = class_index(&project, &input.class_id)?;
        let next_name = input
            .name
            .as_deref()
            .map(require_normalized_name)
            .transpose()?;
        if let Some(normalized_name) = next_name.as_deref() {
            if project.school_classes[class_index].status == SchoolClassStatus::Active {
                ensure_active_name_unique(&project, normalized_name, Some(&input.class_id))?;
            }
        }

        let previous_name = project.school_classes[class_index].normalized_name.clone();
        let school_class = &mut project.school_classes[class_index];
        if let Some(normalized_name) = next_name {
            school_class.name = normalized_name.clone();
            school_class.display_name = normalized_name.clone();
            school_class.normalized_name = normalized_name;
        }
        if input.academic_year.is_some() {
            school_class.academic_year = normalize_optional(input.academic_year);
            school_class.academic_year_id = school_class.academic_year.clone();
        }
        if let Some(grade_level) = input.grade_level {
            school_class.grade_level = Some(grade_level);
        }
        if input.section.is_some() {
            school_class.section =
                normalize_optional(input.section).map(|value| value.to_uppercase());
        }
        if let Some(display_order) = input.display_order {
            school_class.display_order = display_order;
        }
        school_class.updated_at = chrono::Utc::now().to_rfc3339();
        let updated = school_class.clone();
        if previous_name != updated.normalized_name {
            for student in &mut project.students {
                if student
                    .class_name
                    .as_deref()
                    .and_then(normalize_school_class_name)
                    .as_deref()
                    == Some(previous_name.as_str())
                {
                    student.class_name = Some(updated.normalized_name.clone());
                }
            }
        }
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(updated)
    }

    pub fn archive_school_class(&self, input: SchoolClassIdInput) -> Result<SchoolClass, AppError> {
        self.set_class_status(input, SchoolClassStatus::Archived)
    }

    pub fn restore_school_class(&self, input: SchoolClassIdInput) -> Result<SchoolClass, AppError> {
        let project = self.load_project(&input.project_id)?;
        let index = class_index(&project, &input.class_id)?;
        ensure_active_name_unique(
            &project,
            &project.school_classes[index].normalized_name,
            Some(&input.class_id),
        )?;
        drop(project);
        self.set_class_status(input, SchoolClassStatus::Active)
    }

    pub fn get_school_class_overview(
        &self,
        input: GetSchoolClassOverviewInput,
    ) -> Result<SchoolClassOverviewSnapshot, AppError> {
        let project = self.load_project(&input.project_id)?;
        Ok(build_school_class_overview(&project))
    }

    pub fn list_class_students(
        &self,
        input: ListClassStudentsInput,
    ) -> Result<Vec<Student>, AppError> {
        let project = self.load_project(&input.project_id)?;
        students_for_class(&project, &input.class_id)
    }

    pub fn create_class_student(
        &self,
        input: CreateClassStudentInput,
    ) -> Result<Student, AppError> {
        let mut project = self.load_project(&input.project_id)?;
        let school_class = require_active_class(&project, &input.class_id)?.clone();
        let display_name = normalize_optional(input.display_name);
        let number = normalize_optional(input.number);
        validate_student_identity(&display_name, &number)?;
        ensure_student_number_unique(&project, &input.class_id, number.as_deref(), None)?;

        let student = Student {
            id: Uuid::new_v4().to_string(),
            display_name,
            number,
            class_name: Some(school_class.normalized_name),
            warnings: vec![],
            identity_ocr: None,
        };
        project.students.push(student.clone());
        project
            .assessment_activities
            .iter_mut()
            .for_each(|activity| {
                activity
                    .class_applications
                    .iter_mut()
                    .for_each(|application| {
                        if application.school_class_id == input.class_id
                            && application.status
                                != crate::domain::assessment::ClassApplicationStatus::Archived
                        {
                            application.student_scope_ids.push(student.id.clone());
                            application.updated_at = chrono::Utc::now().to_rfc3339();
                            activity.updated_at = application.updated_at.clone();
                        }
                    });
            });
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(student)
    }

    pub fn update_class_student(
        &self,
        input: UpdateClassStudentInput,
    ) -> Result<Student, AppError> {
        let mut project = self.load_project(&input.project_id)?;
        let school_class = require_active_class(&project, &input.class_id)?.clone();
        let student_index = project
            .students
            .iter()
            .position(|student| student.id == input.student_id)
            .ok_or_else(|| {
                student_error(
                    AppErrorCode::StudentNotFound,
                    "Öğrenci bulunamadı.",
                    "student_id not found.",
                )
            })?;
        let is_member = students_for_class(&project, &input.class_id)?
            .iter()
            .any(|student| student.id == input.student_id);
        if !is_member {
            return Err(student_error(
                AppErrorCode::StudentNotFound,
                "Öğrenci bu sınıfta kayıtlı değil.",
                "student is not a member of the requested class.",
            ));
        }
        let display_name = normalize_optional(input.display_name);
        let number = normalize_optional(input.number);
        validate_student_identity(&display_name, &number)?;
        ensure_student_number_unique(
            &project,
            &input.class_id,
            number.as_deref(),
            Some(&input.student_id),
        )?;

        let student = &mut project.students[student_index];
        student.display_name = display_name;
        student.number = number;
        student.class_name = Some(school_class.normalized_name);
        student.warnings.clear();
        let updated = student.clone();
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(updated)
    }

    pub fn import_student_scan_batch(
        &self,
        input: ImportStudentScanBatchInput,
    ) -> Result<ImportStudentScanBatchOutput, AppError> {
        let mut project = self.load_project(&input.project_id)?;
        let school_class = require_active_class(&project, &input.class_id)?.clone();
        validate_pages_per_student(input.pages_per_student)?;

        let source = std::fs::canonicalize(Path::new(&input.source_path)).map_err(|error| {
            app_error(
                AppErrorCode::DocumentImportFailed,
                "Seçilen öğrenci PDF’i okunamadı.",
                Some(error.to_string()),
                Some("Geçerli bir PDF dosyası seçin.".to_string()),
            )
        })?;
        if !source.is_file() {
            return Err(app_error(
                AppErrorCode::DocumentImportFailed,
                "Seçilen öğrenci PDF’i okunamadı.",
                Some(format!("source_path={}", input.source_path)),
                Some("Geçerli bir PDF dosyası seçin.".to_string()),
            ));
        }
        let original_file_name = source
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("student-scan.pdf")
            .to_string();
        let document_id = Uuid::new_v4().to_string();
        let trusted_root = self.project_store.trusted_project_root(&input.project_id)?;
        let safe_file_name = original_file_name
            .chars()
            .map(|character| match character {
                '/' | '\\' | '\0' => '_',
                _ => character,
            })
            .collect::<String>();
        let safe_file_name = if safe_file_name.trim().is_empty()
            || safe_file_name == "."
            || safe_file_name == ".."
        {
            "student-scan.pdf".to_string()
        } else {
            safe_file_name
        };
        let managed_path =
            trusted_root.managed(&format!("documents/{document_id}_{safe_file_name}"))?;
        let destination = trusted_root.prepare_write_target(&managed_path)?;
        copy_selected_source(&source, &destination).map_err(|error| {
            app_error(
                AppErrorCode::DocumentImportFailed,
                "Öğrenci PDF’i proje klasörüne kopyalanamadı.",
                Some(error.to_string()),
                Some("Disk alanını ve klasör izinlerini kontrol edin.".to_string()),
            )
        })?;

        let now = chrono::Utc::now().to_rfc3339();
        let document = Document {
            id: document_id.clone(),
            role: DocumentRole::StudentScan,
            file_name: original_file_name.clone(),
            stored_path: managed_path.as_str().to_string(),
            page_count: 0,
            added_at: now.clone(),
            checksum: None,
            preview: None,
        };
        let batch = new_batch(
            &school_class,
            &document,
            input.display_name,
            input.pages_per_student,
            input.grouping_mode,
            now,
        );
        project.documents.push(document.clone());
        project.student_scan_batches.push(batch.clone());
        set_legacy_active_batch(&mut project, &batch);
        project.workflow = workflow_engine::evaluate_workflow(&project);
        if let Err(error) = self.project_store.commit_snapshot_cas(&project).map(|_| ()) {
            let _ = remove_file_within(trusted_root.root(), &destination);
            return Err(error);
        }
        Ok(ImportStudentScanBatchOutput { document, batch })
    }

    pub fn create_student_scan_batch(
        &self,
        input: CreateStudentScanBatchInput,
    ) -> Result<StudentScanBatch, AppError> {
        let mut project = self.load_project(&input.project_id)?;
        let school_class = require_active_class(&project, &input.class_id)?.clone();
        validate_pages_per_student(input.pages_per_student)?;
        if project
            .student_scan_batches
            .iter()
            .any(|batch| batch.document_id == input.document_id)
        {
            return Err(app_error(
                AppErrorCode::StudentScanBatchAlreadyExists,
                "Bu PDF zaten bir öğrenci paketine bağlı.",
                Some(format!("document_id={}", input.document_id)),
                Some("Mevcut paketi açın veya farklı bir PDF seçin.".to_string()),
            ));
        }
        let document = project
            .documents
            .iter()
            .find(|document| {
                document.id == input.document_id && document.role == DocumentRole::StudentScan
            })
            .cloned()
            .ok_or_else(|| {
                app_error(
                    AppErrorCode::StudentScanNotFound,
                    "Öğrenci PDF’i bulunamadı.",
                    Some(format!("document_id={}", input.document_id)),
                    Some("Belgeler ekranını yenileyin.".to_string()),
                )
            })?;
        let batch = new_batch(
            &school_class,
            &document,
            input.display_name,
            input.pages_per_student,
            input.grouping_mode,
            chrono::Utc::now().to_rfc3339(),
        );
        project.student_scan_batches.push(batch.clone());
        set_legacy_active_batch(&mut project, &batch);
        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(batch)
    }

    pub fn list_student_scan_batches(
        &self,
        input: ListStudentScanBatchesInput,
    ) -> Result<Vec<StudentScanBatch>, AppError> {
        let project = self.load_project(&input.project_id)?;
        let mut batches = project
            .student_scan_batches
            .into_iter()
            .filter(|batch| {
                input
                    .class_id
                    .as_ref()
                    .map_or(true, |class_id| &batch.class_id == class_id)
            })
            .collect::<Vec<_>>();
        batches.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(batches)
    }

    pub fn move_student_scan_batch(
        &self,
        input: MoveStudentScanBatchInput,
    ) -> Result<StudentScanBatch, AppError> {
        let mut project = self.load_project(&input.project_id)?;
        require_active_class(&project, &input.target_class_id)?;
        let batch_index = batch_index(&project, &input.batch_id)?;
        let document_id = project.student_scan_batches[batch_index]
            .document_id
            .clone();
        let now = chrono::Utc::now().to_rfc3339();
        project.student_scan_batches[batch_index].class_id = input.target_class_id.clone();
        project.student_scan_batches[batch_index].updated_at = now.clone();

        for submission in &mut project.student_submissions {
            if submission.scan_batch_id.as_deref() == Some(input.batch_id.as_str())
                || submission.document_id == document_id
            {
                submission.class_id = Some(input.target_class_id.clone());
                submission.scan_batch_id = Some(input.batch_id.clone());
                submission.class_membership_source =
                    Some(ClassMembershipSource::InheritedFromBatch);
                submission.updated_at = Some(now.clone());
            }
        }

        let updated = project.student_scan_batches[batch_index].clone();
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(updated)
    }

    pub fn remove_student_scan_batch(
        &self,
        input: RemoveStudentScanBatchInput,
    ) -> Result<StudentScanBatch, AppError> {
        let project = self.load_project(&input.project_id)?;
        let batch = project.student_scan_batches[batch_index(&project, &input.batch_id)?].clone();
        let removed_document = project
            .documents
            .iter()
            .find(|document| document.id == batch.document_id)
            .cloned();
        let batch_id = batch.id.clone();
        let document_id = batch.document_id.clone();
        let output = self.project_store.commit_job(
            &input.project_id,
            crate::services::project_store::MutationOptions::new("remove_student_scan_batch"),
            move |current, _context| {
                let current_batch_index = batch_index(current, &batch_id)?;
                let current_batch = current.student_scan_batches[current_batch_index].clone();
                let dependent_submission_ids = current
                    .student_submissions
                    .iter()
                    .filter(|submission| {
                        submission.scan_batch_id.as_deref() == Some(batch_id.as_str())
                            || submission.document_id == document_id
                    })
                    .map(|submission| submission.id.clone())
                    .collect::<Vec<_>>();
                let jobs = persisted_dependency_jobs(current)?;
                let dependency_scan = scan_submission_dependencies_with_jobs(
                    current,
                    &dependent_submission_ids,
                    &jobs,
                );
                if !dependent_submission_ids.is_empty() || dependency_scan.is_blocked() {
                    return Err(app_error(
                        AppErrorCode::StudentScanBatchInUse,
                        "Öğrenci paketi mevcut öğrenci işlemleri nedeniyle silinemez.",
                        Some(format!(
                            "batch_id={}; submissions={}; ocr_records={}; ocr_generations={}; scoring_records={}; running_jobs={}",
                            current_batch.id,
                            dependent_submission_ids.len(),
                            dependency_scan.ocr_record_count,
                            dependency_scan.ocr_generation_count,
                            dependency_scan.scoring_record_count,
                            dependency_scan.running_job_count
                        )),
                        Some("Öğrenci, OCR ve notlandırma kayıtlarını koruyarak paketi kullanmaya devam edin.".to_string()),
                    ));
                }

                current.student_submissions.retain(|submission| {
                    !(submission.scan_batch_id.as_deref() == Some(batch_id.as_str())
                        || submission.document_id == document_id)
                });
                let referenced_student_ids = current
                    .student_submissions
                    .iter()
                    .map(|submission| submission.student_id.clone())
                    .collect::<std::collections::HashSet<_>>();
                current
                    .students
                    .retain(|student| referenced_student_ids.contains(&student.id));
                current.student_scan_batches.remove(current_batch_index);
                current.documents.retain(|document| document.id != document_id);
                if current.student_scan_document_id.as_deref() == Some(document_id.as_str()) {
                    current.student_scan_document_id = None;
                    current.student_grouping_mode = None;
                    current.student_pages_per_student = None;
                    current.student_grouping_complete_at = None;
                }
                current.workflow = workflow_engine::evaluate_workflow(current);
                Ok(current_batch)
            },
        );
        let removed_batch = match output {
            crate::services::project_store::JobCommitResult::Applied(output) => output.result,
            crate::services::project_store::JobCommitResult::Conflict(error)
            | crate::services::project_store::JobCommitResult::Rejected(error) => {
                return Err(error)
            }
            crate::services::project_store::JobCommitResult::Stale { reason } => {
                return Err(app_error(
                    AppErrorCode::SubmissionDeleteConflict,
                    "Öğrenci paketi silinemedi; bağlı veri durumu değişti.",
                    Some(reason),
                    Some("Listeyi yenileyip bağımlılıkları tekrar kontrol edin.".to_string()),
                ))
            }
            crate::services::project_store::JobCommitResult::EntityMissing => {
                return Err(app_error(
                    AppErrorCode::StudentScanBatchNotFound,
                    "Öğrenci paketi artık mevcut değil.",
                    None,
                    Some("Listeyi yenileyin.".to_string()),
                ))
            }
        };

        if let Some(document) = removed_document {
            let trusted_root = self.project_store.trusted_project_root(&input.project_id)?;
            let document_path = document.resolve_path_with_root(&trusted_root);
            let document_cleanup = match document_path {
                Ok(path) => remove_file_within(trusted_root.root(), &path)
                    .map(|_| ())
                    .map_err(|error| error.to_string()),
                Err(error) => Err(error.to_string()),
            };
            if let Err(error) = document_cleanup {
                log::warn!(
                    "Sınıf paketi kaydı kaldırıldı ancak belge artığı güvenle silinemedi: document_id={}; error={error}",
                    document.id
                );
            }
            let preview_base = trusted_root.root().join("cache").join("page_previews");
            let preview_path =
                trusted_root.managed(&format!("cache/page_previews/{}", document.id));
            let preview_dir =
                preview_path.map(|managed| trusted_root.root().join(managed.as_path()));
            let preview_cleanup = match preview_dir {
                Ok(path) => remove_dir_within(&preview_base, &path)
                    .map(|_| ())
                    .map_err(|error| error.to_string()),
                Err(error) => Err(error.to_string()),
            };
            if let Err(error) = preview_cleanup {
                log::warn!(
                    "Sınıf paketi kaydı kaldırıldı ancak önizleme artığı güvenle silinemedi: document_id={}; error={error}",
                    document.id
                );
            }
            let outputs_base = trusted_root.root().join("outputs").join("previews");
            if let Ok(managed) = trusted_root.managed(&format!("outputs/previews/{}", document.id))
            {
                let output_dir = trusted_root.root().join(managed.as_path());
                if let Err(error) = remove_dir_within(&outputs_base, &output_dir) {
                    log::warn!(
                        "Sınıf paketi kaydı kaldırıldı ancak immutable önizleme artığı silinemedi: document_id={}; error={error}",
                        document.id
                    );
                }
            }
        }
        Ok(removed_batch)
    }

    fn set_class_status(
        &self,
        input: SchoolClassIdInput,
        status: SchoolClassStatus,
    ) -> Result<SchoolClass, AppError> {
        let mut project = self.load_project(&input.project_id)?;
        let index = class_index(&project, &input.class_id)?;
        project.school_classes[index].status = status;
        project.school_classes[index].updated_at = chrono::Utc::now().to_rfc3339();
        let updated = project.school_classes[index].clone();
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

fn copy_selected_source(source: &Path, destination: &Path) -> std::io::Result<()> {
    let mut input = std::fs::File::open(source)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let result = (|| {
        let mut buffer = [0_u8; 128 * 1024];
        loop {
            let read = input.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            output.write_all(&buffer[..read])?;
        }
        output.sync_all()
    })();
    if result.is_err() {
        drop(output);
        let _ = std::fs::remove_file(destination);
    }
    result
}

pub fn students_for_class(project: &Project, class_id: &str) -> Result<Vec<Student>, AppError> {
    let school_class = require_active_class(project, class_id)?;
    let mut students = project
        .students
        .iter()
        .filter(|student| {
            student
                .class_name
                .as_deref()
                .and_then(normalize_school_class_name)
                .as_deref()
                == Some(school_class.normalized_name.as_str())
        })
        .cloned()
        .collect::<Vec<_>>();
    students.sort_by(|left, right| {
        left.number
            .as_deref()
            .unwrap_or_default()
            .cmp(right.number.as_deref().unwrap_or_default())
            .then_with(|| {
                left.display_name
                    .as_deref()
                    .unwrap_or_default()
                    .cmp(right.display_name.as_deref().unwrap_or_default())
            })
    });
    Ok(students)
}

pub fn build_school_class_overview(project: &Project) -> SchoolClassOverviewSnapshot {
    let active_scoring_records = scoring_active_records(project);
    let known_class_ids = project
        .school_classes
        .iter()
        .map(|school_class| school_class.id.as_str())
        .collect::<Vec<_>>();
    let mut classes = project
        .school_classes
        .iter()
        .cloned()
        .map(|school_class| {
            let submissions = project
                .student_submissions
                .iter()
                .filter(|submission| {
                    submission_class_id(project, submission) == Some(school_class.id.as_str())
                })
                .collect::<Vec<_>>();
            let identity_verified_count = submissions
                .iter()
                .filter(|submission| {
                    project
                        .students
                        .iter()
                        .find(|student| student.id == submission.student_id)
                        .is_some_and(|student| !student_identity_is_missing(student))
                })
                .count() as u32;
            let ocr_complete_count = submissions
                .iter()
                .filter(|submission| submission_ocr_complete(project, submission))
                .count() as u32;
            let scoring_complete_count = submissions
                .iter()
                .filter(|submission| {
                    !project.questions.is_empty()
                        && project.questions.iter().all(|question| {
                            active_scoring_records.iter().any(|record| {
                                record.submission_id == submission.id
                                    && record.question_id == question.id
                                    && scoring_record_is_accepted(record)
                            })
                        })
                })
                .count() as u32;
            let review_required_count =
                submissions
                    .iter()
                    .filter(|submission| {
                        project.student_answer_ocr_records.iter().any(|record| {
                            record.submission_id == submission.id && record.needs_review
                        }) || active_scoring_records.iter().any(|record| {
                            record.submission_id == submission.id
                                && (record.needs_review || !record.scoring_applied)
                        })
                    })
                    .count() as u32;
            SchoolClassOverview {
                scan_batch_count: project
                    .student_scan_batches
                    .iter()
                    .filter(|batch| batch.class_id == school_class.id)
                    .count() as u32,
                submission_count: submissions.len() as u32,
                identity_verified_count,
                ocr_complete_count,
                scoring_complete_count,
                review_required_count,
                school_class,
            }
        })
        .collect::<Vec<_>>();
    classes.sort_by(|left, right| {
        left.school_class
            .display_order
            .cmp(&right.school_class.display_order)
            .then_with(|| left.school_class.name.cmp(&right.school_class.name))
    });

    SchoolClassOverviewSnapshot {
        unassigned_batch_count: project
            .student_scan_batches
            .iter()
            .filter(|batch| !known_class_ids.contains(&batch.class_id.as_str()))
            .count() as u32,
        unassigned_submission_count: project
            .student_submissions
            .iter()
            .filter(|submission| {
                submission_class_id(project, submission)
                    .map_or(true, |class_id| !known_class_ids.contains(&class_id))
            })
            .count() as u32,
        classes,
    }
}

fn submission_class_id<'a>(
    project: &'a Project,
    submission: &'a StudentSubmission,
) -> Option<&'a str> {
    submission.class_id.as_deref().or_else(|| {
        submission.scan_batch_id.as_deref().and_then(|batch_id| {
            project
                .student_scan_batches
                .iter()
                .find(|batch| batch.id == batch_id)
                .map(|batch| batch.class_id.as_str())
        })
    })
}

fn submission_ocr_complete(project: &Project, submission: &StudentSubmission) -> bool {
    !project.questions.is_empty()
        && project.questions.iter().all(|question| {
            project.student_answer_ocr_records.iter().any(|record| {
                record.submission_id == submission.id
                    && record.question_id == question.id
                    && record.status == StudentAnswerOcrStatus::TeacherApproved
                    && !record.needs_review
            })
        })
}

fn new_batch(
    school_class: &SchoolClass,
    document: &Document,
    display_name: Option<String>,
    pages_per_student: Option<u32>,
    grouping_mode: Option<PageGroupingMode>,
    now: String,
) -> StudentScanBatch {
    StudentScanBatch {
        id: Uuid::new_v4().to_string(),
        class_id: school_class.id.clone(),
        document_id: document.id.clone(),
        original_file_name: document.file_name.clone(),
        display_name: normalize_optional(display_name)
            .unwrap_or_else(|| format!("{} · {}", school_class.name, document.file_name)),
        pages_per_student,
        grouping_mode,
        grouping_completed_at: None,
        created_at: now.clone(),
        updated_at: now,
    }
}

fn set_legacy_active_batch(project: &mut Project, batch: &StudentScanBatch) {
    project.student_scan_document_id = Some(batch.document_id.clone());
    project.student_grouping_mode = batch.grouping_mode.clone();
    project.student_pages_per_student = batch.pages_per_student;
    project.student_grouping_complete_at = batch.grouping_completed_at.clone();
}

fn require_active_class<'a>(
    project: &'a Project,
    class_id: &str,
) -> Result<&'a SchoolClass, AppError> {
    let school_class = project
        .school_classes
        .iter()
        .find(|school_class| school_class.id == class_id)
        .ok_or_else(|| {
            app_error(
                AppErrorCode::SchoolClassNotFound,
                "Sınıf bulunamadı.",
                Some(format!("class_id={class_id}")),
                Some("Sınıf listesini yenileyin.".to_string()),
            )
        })?;
    if school_class.status != SchoolClassStatus::Active {
        return Err(app_error(
            AppErrorCode::SchoolClassArchived,
            "Arşivlenmiş sınıfa yeni öğrenci paketi eklenemez.",
            Some(format!("class_id={class_id}")),
            Some("Sınıfı geri yükleyip tekrar deneyin.".to_string()),
        ));
    }
    Ok(school_class)
}

fn class_index(project: &Project, class_id: &str) -> Result<usize, AppError> {
    project
        .school_classes
        .iter()
        .position(|school_class| school_class.id == class_id)
        .ok_or_else(|| {
            app_error(
                AppErrorCode::SchoolClassNotFound,
                "Sınıf bulunamadı.",
                Some(format!("class_id={class_id}")),
                Some("Sınıf listesini yenileyin.".to_string()),
            )
        })
}

fn batch_index(project: &Project, batch_id: &str) -> Result<usize, AppError> {
    project
        .student_scan_batches
        .iter()
        .position(|batch| batch.id == batch_id)
        .ok_or_else(|| {
            app_error(
                AppErrorCode::StudentScanBatchNotFound,
                "Öğrenci PDF paketi bulunamadı.",
                Some(format!("batch_id={batch_id}")),
                Some("Paket listesini yenileyin.".to_string()),
            )
        })
}

fn ensure_active_name_unique(
    project: &Project,
    normalized_name: &str,
    except_class_id: Option<&str>,
) -> Result<(), AppError> {
    if project.school_classes.iter().any(|school_class| {
        school_class.status == SchoolClassStatus::Active
            && school_class.normalized_name == normalized_name
            && except_class_id != Some(school_class.id.as_str())
    }) {
        return Err(app_error(
            AppErrorCode::SchoolClassAlreadyExists,
            "Aynı adlı etkin sınıf zaten mevcut.",
            Some(format!("normalized_name={normalized_name}")),
            Some("Mevcut sınıfı seçin veya farklı bir ad kullanın.".to_string()),
        ));
    }
    Ok(())
}

fn require_normalized_name(value: &str) -> Result<String, AppError> {
    normalize_school_class_name(value).ok_or_else(|| {
        app_error(
            AppErrorCode::SchoolClassNameInvalid,
            "Sınıf adı boş veya geçersiz.",
            Some(format!("name={value:?}")),
            Some("11-A gibi geçerli bir sınıf adı girin.".to_string()),
        )
    })
}

fn validate_pages_per_student(value: Option<u32>) -> Result<(), AppError> {
    if value.is_some_and(|pages| pages == 0 || pages > 20) {
        return Err(app_error(
            AppErrorCode::StudentGroupingInvalid,
            "Öğrenci başına sayfa sayısı 1 ile 20 arasında olmalıdır.",
            value.map(|pages| format!("pages_per_student={pages}")),
            Some("Geçerli bir sayfa sayısı girin.".to_string()),
        ));
    }
    Ok(())
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn validate_student_identity(
    display_name: &Option<String>,
    number: &Option<String>,
) -> Result<(), AppError> {
    if display_name.is_none() && number.is_none() {
        return Err(student_error(
            AppErrorCode::StudentIdentityInvalid,
            "Öğrenci adı veya okul numarası girilmelidir.",
            "class student requires display_name or number.",
        ));
    }
    Ok(())
}

fn ensure_student_number_unique(
    project: &Project,
    class_id: &str,
    number: Option<&str>,
    except_student_id: Option<&str>,
) -> Result<(), AppError> {
    let Some(number) = number.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    if students_for_class(project, class_id)?
        .iter()
        .any(|student| {
            student.id != except_student_id.unwrap_or_default()
                && student.number.as_deref().map(str::trim) == Some(number)
        })
    {
        return Err(student_error(
            AppErrorCode::StudentIdentityInvalid,
            "Bu okul numarası sınıfta zaten kayıtlı.",
            "student number is already used in the class.",
        ));
    }
    Ok(())
}

fn student_error(code: AppErrorCode, message: &str, technical_details: &str) -> AppError {
    AppError {
        code,
        message: message.to_string(),
        recoverable: true,
        suggested_action: Some("Öğrenci bilgilerini kontrol edip tekrar deneyin.".to_string()),
        technical_details: Some(technical_details.to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    }
}

fn required_text(value: String, field: &str) -> Result<String, AppError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(app_error(
            AppErrorCode::TeachingAssignmentInvalid,
            "Görevlendirme için zorunlu alan eksik.",
            Some(format!("field={field}")),
            Some("Eğitim yılı, ders ve sınıf bilgilerini doldurun.".to_string()),
        ));
    }
    Ok(value)
}

fn class_matches_academic_year(school_class: &SchoolClass, academic_year_id: &str) -> bool {
    school_class
        .academic_year_id
        .as_deref()
        .or(school_class.academic_year.as_deref())
        == Some(academic_year_id)
}

fn sort_classes(classes: &mut [SchoolClass]) {
    classes.sort_by(|left, right| {
        left.display_order
            .cmp(&right.display_order)
            .then_with(|| left.name.cmp(&right.name))
    });
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
    use crate::domain::scoring::scoring_package_hash;
    use crate::domain::student::{Student, StudentSubmissionStatus};

    fn project_for_tests() -> (ProjectStore, Project) {
        let root = std::env::temp_dir().join(format!("rubrika-school-class-{}", Uuid::new_v4()));
        let store = ProjectStore::new();
        let project = store
            .create_project(
                "School class test".to_string(),
                root.to_string_lossy().to_string(),
            )
            .unwrap();
        (store, project)
    }

    fn create_class(service: &SchoolClassService, project_id: &str, name: &str) -> SchoolClass {
        service
            .create_school_class(CreateSchoolClassInput {
                project_id: project_id.to_string(),
                name: name.to_string(),
                academic_year: None,
                grade_level: None,
                section: None,
                display_order: None,
            })
            .unwrap()
    }

    #[test]
    fn class_student_service_persists_roster_and_updates_identity() {
        let (store, project) = project_for_tests();
        let service = SchoolClassService::new(store);
        let school_class = create_class(&service, &project.id, "11-A");

        let created = service
            .create_class_student(CreateClassStudentInput {
                project_id: project.id.clone(),
                class_id: school_class.id.clone(),
                display_name: Some("Ayşe Yılmaz".to_string()),
                number: Some("1042".to_string()),
            })
            .expect("class student should be created");
        assert_eq!(created.class_name.as_deref(), Some("11-A"));
        assert_eq!(
            service
                .list_class_students(ListClassStudentsInput {
                    project_id: project.id.clone(),
                    class_id: school_class.id.clone(),
                })
                .expect("class roster should list")
                .len(),
            1
        );

        let updated = service
            .update_class_student(UpdateClassStudentInput {
                project_id: project.id.clone(),
                class_id: school_class.id,
                student_id: created.id,
                display_name: Some("Ayşe Yılmaz Kaya".to_string()),
                number: Some("1043".to_string()),
            })
            .expect("class student should update");
        assert_eq!(updated.display_name.as_deref(), Some("Ayşe Yılmaz Kaya"));
        assert_eq!(updated.number.as_deref(), Some("1043"));
    }

    fn add_scan_document(store: &ProjectStore, project_id: &str, document_id: &str) {
        let mut project = store.get_project_snapshot(project_id.to_string()).unwrap();
        project.documents.push(Document {
            id: document_id.to_string(),
            role: DocumentRole::StudentScan,
            file_name: format!("{document_id}.pdf"),
            stored_path: format!("{document_id}.pdf"),
            page_count: 4,
            added_at: chrono::Utc::now().to_rfc3339(),
            checksum: None,
            preview: None,
        });
        store.save_project(&project).unwrap();
    }

    fn create_batch(
        service: &SchoolClassService,
        project_id: &str,
        class_id: &str,
        document_id: &str,
    ) -> StudentScanBatch {
        service
            .create_student_scan_batch(CreateStudentScanBatchInput {
                project_id: project_id.to_string(),
                class_id: class_id.to_string(),
                document_id: document_id.to_string(),
                display_name: None,
                pages_per_student: Some(2),
                grouping_mode: Some(PageGroupingMode::FixedPagesPerStudent),
            })
            .unwrap()
    }

    #[test]
    fn active_names_are_normalized_unique_and_restore_checks_conflicts() {
        let (store, project) = project_for_tests();
        let service = SchoolClassService::new(store);
        let archived = create_class(&service, &project.id, " 11 a ");
        assert_eq!(archived.name, "11-A");

        let duplicate = service.create_school_class(CreateSchoolClassInput {
            project_id: project.id.clone(),
            name: "11-A".to_string(),
            academic_year: None,
            grade_level: None,
            section: None,
            display_order: None,
        });
        assert_eq!(
            duplicate.unwrap_err().code,
            AppErrorCode::SchoolClassAlreadyExists
        );

        service
            .archive_school_class(SchoolClassIdInput {
                project_id: project.id.clone(),
                class_id: archived.id.clone(),
            })
            .unwrap();
        let replacement = create_class(&service, &project.id, "11A");
        let restore = service.restore_school_class(SchoolClassIdInput {
            project_id: project.id.clone(),
            class_id: archived.id.clone(),
        });
        assert_eq!(
            restore.unwrap_err().code,
            AppErrorCode::SchoolClassAlreadyExists
        );
        assert_ne!(replacement.id, archived.id);
    }

    #[test]
    fn one_document_has_one_batch_and_archived_class_rejects_new_batch() {
        let (store, project) = project_for_tests();
        let service = SchoolClassService::new(store.clone());
        let school_class = create_class(&service, &project.id, "10-B");
        add_scan_document(&store, &project.id, "scan-1");
        create_batch(&service, &project.id, &school_class.id, "scan-1");

        let duplicate = service.create_student_scan_batch(CreateStudentScanBatchInput {
            project_id: project.id.clone(),
            class_id: school_class.id.clone(),
            document_id: "scan-1".to_string(),
            display_name: None,
            pages_per_student: None,
            grouping_mode: None,
        });
        assert_eq!(
            duplicate.unwrap_err().code,
            AppErrorCode::StudentScanBatchAlreadyExists
        );

        service
            .archive_school_class(SchoolClassIdInput {
                project_id: project.id.clone(),
                class_id: school_class.id.clone(),
            })
            .unwrap();
        add_scan_document(&store, &project.id, "scan-2");
        let archived_import = service.create_student_scan_batch(CreateStudentScanBatchInput {
            project_id: project.id,
            class_id: school_class.id,
            document_id: "scan-2".to_string(),
            display_name: None,
            pages_per_student: None,
            grouping_mode: None,
        });
        assert_eq!(
            archived_import.unwrap_err().code,
            AppErrorCode::SchoolClassArchived
        );
    }

    #[test]
    fn multiple_batches_filter_by_class_and_overview_counts_them() {
        let (store, project) = project_for_tests();
        let service = SchoolClassService::new(store.clone());
        let class_a = create_class(&service, &project.id, "9-A");
        let class_b = create_class(&service, &project.id, "9-B");
        add_scan_document(&store, &project.id, "scan-a");
        add_scan_document(&store, &project.id, "scan-b");
        create_batch(&service, &project.id, &class_a.id, "scan-a");
        create_batch(&service, &project.id, &class_b.id, "scan-b");

        let filtered = service
            .list_student_scan_batches(ListStudentScanBatchesInput {
                project_id: project.id.clone(),
                class_id: Some(class_a.id.clone()),
            })
            .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].class_id, class_a.id);
        let snapshot = service
            .get_school_class_overview(GetSchoolClassOverviewInput {
                project_id: project.id,
            })
            .unwrap();
        assert_eq!(snapshot.classes.len(), 2);
        assert!(snapshot
            .classes
            .iter()
            .all(|overview| overview.scan_batch_count == 1));
    }

    #[test]
    fn moving_batch_updates_only_membership_and_preserves_scoring_hash() {
        let (store, project) = project_for_tests();
        let service = SchoolClassService::new(store.clone());
        let class_a = create_class(&service, &project.id, "8-A");
        let class_b = create_class(&service, &project.id, "8-B");
        add_scan_document(&store, &project.id, "scan-a");
        let batch = create_batch(&service, &project.id, &class_a.id, "scan-a");
        let mut project = store.get_project_snapshot(project.id.clone()).unwrap();
        project.students.push(Student {
            id: "student-1".to_string(),
            display_name: Some("Öğrenci".to_string()),
            number: Some("1".to_string()),
            class_name: Some("8-A".to_string()),
            warnings: vec![],
            identity_ocr: None,
        });
        project.student_submissions.push(StudentSubmission {
            assessment_activity_id: None,
            id: "submission-1".to_string(),
            student_id: "student-1".to_string(),
            document_id: batch.document_id.clone(),
            class_id: Some(class_a.id.clone()),
            scan_batch_id: Some(batch.id.clone()),
            class_membership_source: Some(ClassMembershipSource::TeacherOverride),
            page_numbers: vec![1, 2],
            status: StudentSubmissionStatus::Grouped,
            answer_slots: vec![],
            warnings: vec![],
            updated_at: None,
        });
        store.save_project(&project).unwrap();
        let hash_before = scoring_package_hash(&project);
        let ocr_before = serde_json::to_value(&project.student_answer_ocr_records).unwrap();
        let scoring_before = serde_json::to_value(&project.scoring_records).unwrap();

        service
            .move_student_scan_batch(MoveStudentScanBatchInput {
                project_id: project.id.clone(),
                batch_id: batch.id.clone(),
                target_class_id: class_b.id.clone(),
            })
            .unwrap();
        let moved = store.get_project_snapshot(project.id.clone()).unwrap();

        assert_eq!(moved.student_scan_batches[0].class_id, class_b.id);
        assert_eq!(moved.student_submissions[0].class_id, Some(class_b.id));
        assert_eq!(
            moved.student_submissions[0].class_membership_source,
            Some(ClassMembershipSource::InheritedFromBatch)
        );
        assert_eq!(moved.students[0].class_name.as_deref(), Some("8-A"));
        assert_eq!(scoring_package_hash(&moved), hash_before);
        assert_eq!(
            serde_json::to_value(&moved.student_answer_ocr_records).unwrap(),
            ocr_before
        );
        assert_eq!(
            serde_json::to_value(&moved.scoring_records).unwrap(),
            scoring_before
        );

        let remove = service.remove_student_scan_batch(RemoveStudentScanBatchInput {
            project_id: project.id,
            batch_id: batch.id,
        });
        assert_eq!(
            remove.unwrap_err().code,
            AppErrorCode::StudentScanBatchInUse
        );
    }

    #[test]
    fn same_school_number_in_different_classes_stays_distinct() {
        // TD-01 fixture: student identity is UUID-based and the school-number
        // uniqueness check is scoped per class. Two students with the same
        // number in different classes must both be accepted, keep distinct
        // IDs, and never leak into each other's roster.
        let (store, project) = project_for_tests();
        let service = SchoolClassService::new(store);
        let class_a = create_class(&service, &project.id, "9-A");
        let class_b = create_class(&service, &project.id, "9-B");

        let student_a = service
            .create_class_student(CreateClassStudentInput {
                project_id: project.id.clone(),
                class_id: class_a.id.clone(),
                display_name: Some("Ayşe".to_string()),
                number: Some("123".to_string()),
            })
            .expect("first class may register number 123");
        let student_b = service
            .create_class_student(CreateClassStudentInput {
                project_id: project.id.clone(),
                class_id: class_b.id.clone(),
                display_name: Some("Burak".to_string()),
                number: Some("123".to_string()),
            })
            .expect("same number in a different class is not a conflict");

        assert_ne!(student_a.id, student_b.id, "identity is UUID-based");
        assert_eq!(student_a.class_name.as_deref(), Some("9-A"));
        assert_eq!(student_b.class_name.as_deref(), Some("9-B"));

        let roster_a = service
            .list_class_students(ListClassStudentsInput {
                project_id: project.id.clone(),
                class_id: class_a.id.clone(),
            })
            .unwrap();
        let roster_b = service
            .list_class_students(ListClassStudentsInput {
                project_id: project.id.clone(),
                class_id: class_b.id.clone(),
            })
            .unwrap();
        assert_eq!(roster_a.len(), 1);
        assert_eq!(roster_b.len(), 1);
        assert_eq!(roster_a[0].id, student_a.id);
        assert_eq!(roster_b[0].id, student_b.id);

        // The per-class uniqueness scope still rejects a duplicate number
        // inside the SAME class.
        let duplicate_in_class = service.create_class_student(CreateClassStudentInput {
            project_id: project.id.clone(),
            class_id: class_a.id.clone(),
            display_name: Some("Cem".to_string()),
            number: Some("123".to_string()),
        });
        assert_eq!(
            duplicate_in_class.unwrap_err().code,
            AppErrorCode::StudentIdentityInvalid,
            "same number inside the same class stays rejected"
        );
    }
}

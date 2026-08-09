use serde::{Deserialize, Serialize};

use super::speaking::SpeakingAttempt;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentType {
    Written,
    Listening,
    Speaking,
    /// Legacy tombstone: old project JSON may still contain `"performance"`.
    /// Kept only so archived test projects deserialize; no active workflow
    /// creates or opens performance activities anymore.
    #[serde(alias = "performance")]
    LegacyPerformance,
}

impl AssessmentType {
    pub fn workflow_family(self) -> WorkflowFamily {
        match self {
            Self::Speaking => WorkflowFamily::Speaking,
            Self::LegacyPerformance => WorkflowFamily::LegacyPerformance,
            Self::Written | Self::Listening => WorkflowFamily::Written,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowFamily {
    #[default]
    Written,
    Speaking,
    /// Legacy tombstone mirroring `AssessmentType::LegacyPerformance`.
    #[serde(alias = "performance")]
    LegacyPerformance,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentStatus {
    #[default]
    Draft,
    Scheduled,
    Active,
    Completed,
    Archived,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ClassApplicationStatus {
    #[default]
    Scheduled,
    Active,
    Completed,
    Archived,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ListeningDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_document_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript_document_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub play_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instruction: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingConfigurationSnapshot {
    #[serde(default)]
    pub speaking_type: String,
    #[serde(default)]
    pub task_text: String,
    #[serde(default)]
    pub target_duration_seconds: u32,
    #[serde(default)]
    pub min_duration_seconds: u32,
    #[serde(default)]
    pub max_duration_seconds: u32,
    #[serde(default)]
    pub rubric_version: String,
    #[serde(default)]
    pub scoring_policy_version: String,
    #[serde(default)]
    pub cleanup_prompt_version: String,
    #[serde(default)]
    pub evaluation_prompt_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen_model_file_hash: Option<String>,
    #[serde(default)]
    pub rubric_snapshot: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClassApplication {
    pub id: String,
    pub activity_id: String,
    #[serde(default, alias = "classSectionId")]
    pub school_class_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_date: Option<String>,
    #[serde(default)]
    pub status: ClassApplicationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default)]
    pub document_ids: Vec<String>,
    /// Snapshot of the students in scope when the application was prepared.
    /// Runtime membership is still validated against the central SchoolClassService.
    #[serde(default)]
    pub student_scope_ids: Vec<String>,
    /// Canonical speaking attempt storage for newly created attempts.
    #[serde(default)]
    pub speaking_attempts: Vec<SpeakingAttempt>,
    pub created_at: String,
    pub updated_at: String,
}

/// Compatibility name for the first organization API revision.
pub type AssessmentClassApplication = ClassApplication;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentActivity {
    pub id: String,
    pub academic_year_id: String,
    pub course_id: String,
    pub course_name: String,
    #[serde(default)]
    pub title: String,
    pub grade_level: u32,
    pub term: u8,
    pub assessment_type: AssessmentType,
    /// Derived from assessment_type; kept in the persisted DTO for explicit UI state.
    #[serde(default)]
    pub workflow_family: WorkflowFamily,
    pub sequence_number: u32,
    #[serde(default)]
    pub status: AssessmentStatus,
    #[serde(default)]
    pub common_document_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listening_details: Option<ListeningDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaking_configuration: Option<SpeakingConfigurationSnapshot>,
    #[serde(default)]
    pub class_applications: Vec<AssessmentClassApplication>,
    pub created_at: String,
    pub updated_at: String,
}

impl AssessmentActivity {
    pub fn display_title(&self) -> String {
        let type_label = match self.assessment_type {
            AssessmentType::Written => "Yazılı Sınav",
            AssessmentType::Listening => "Dinleme Sınavı",
            AssessmentType::Speaking => "Konuşma Sınavı",
            AssessmentType::LegacyPerformance => "Performans Görevi",
        };
        format!(
            "{}. Sınıf · {}. Dönem · {}",
            self.grade_level, self.term, type_label
        )
    }

    pub fn is_speaking(&self) -> bool {
        self.assessment_type == AssessmentType::Speaking
    }

    pub fn is_same_key(&self, other: &Self) -> bool {
        self.academic_year_id == other.academic_year_id
            && self.course_id == other.course_id
            && self.grade_level == other.grade_level
            && self.term == other.term
            && self.assessment_type == other.assessment_type
            && self.sequence_number == other.sequence_number
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TeachingAssignment {
    pub id: String,
    pub academic_year_id: String,
    pub course_id: String,
    pub course_name: String,
    pub class_section_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teacher_id: Option<String>,
    #[serde(default = "default_true")]
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::{AssessmentActivity, AssessmentType, WorkflowFamily};

    #[test]
    fn listening_reuses_written_workflow() {
        assert_eq!(
            AssessmentType::Listening.workflow_family(),
            WorkflowFamily::Written
        );
        assert_eq!(
            AssessmentType::Speaking.workflow_family(),
            WorkflowFamily::Speaking
        );
        assert_eq!(
            AssessmentType::LegacyPerformance.workflow_family(),
            WorkflowFamily::LegacyPerformance
        );
    }

    #[test]
    fn legacy_performance_variant_keeps_old_serialization_compatible() {
        let activity = AssessmentActivity {
            id: "legacy-perf".into(),
            academic_year_id: "2026-2027".into(),
            course_id: "tde".into(),
            course_name: "Türk Dili ve Edebiyatı".into(),
            title: "1. Performans".into(),
            grade_level: 9,
            term: 1,
            assessment_type: AssessmentType::LegacyPerformance,
            workflow_family: WorkflowFamily::LegacyPerformance,
            sequence_number: 1,
            status: Default::default(),
            common_document_ids: vec![],
            listening_details: None,
            speaking_configuration: None,
            class_applications: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };
        let value = serde_json::to_value(&activity).unwrap();
        let value = value.as_object().unwrap();
        assert_eq!(
            value["assessmentType"].as_str().unwrap(),
            "legacy_performance"
        );
        assert_eq!(
            value["workflowFamily"].as_str().unwrap(),
            "legacy_performance"
        );
        assert!(value.get("performanceDetails").is_none());
    }

    #[test]
    fn legacy_performance_json_still_deserializes() {
        let json = r#"{
            "id": "legacy-perf",
            "academicYearId": "2026-2027",
            "courseId": "tde",
            "courseName": "TDE",
            "title": "1. Performans",
            "gradeLevel": 9,
            "term": 1,
            "assessmentType": "performance",
            "workflowFamily": "performance",
            "sequenceNumber": 1,
            "performanceDetails": { "theme": "Eski tema" },
            "classApplications": [{
                "id": "app-1",
                "activityId": "legacy-perf",
                "schoolClassId": "class-9-a",
                "status": "scheduled",
                "studentScopeIds": [],
                "speakingAttempts": [],
                "performanceAssessments": [],
                "createdAt": "2026-01-01T00:00:00Z",
                "updatedAt": "2026-01-01T00:00:00Z"
            }],
            "createdAt": "2026-01-01T00:00:00Z",
            "updatedAt": "2026-01-01T00:00:00Z"
        }"#;
        let activity: AssessmentActivity = serde_json::from_str(json).unwrap();
        assert_eq!(activity.assessment_type, AssessmentType::LegacyPerformance);
        assert_eq!(activity.workflow_family, WorkflowFamily::LegacyPerformance);
        assert!(activity.class_applications[0].speaking_attempts.is_empty());
    }

    #[test]
    fn activity_key_excludes_class_application() {
        let base = AssessmentActivity {
            id: "a".into(),
            academic_year_id: "2026-2027".into(),
            course_id: "turkce".into(),
            course_name: "Türk Dili ve Edebiyatı".into(),
            title: "1. Yazılı".into(),
            grade_level: 10,
            term: 1,
            assessment_type: AssessmentType::Written,
            workflow_family: WorkflowFamily::Written,
            sequence_number: 1,
            status: Default::default(),
            common_document_ids: vec![],
            listening_details: None,
            speaking_configuration: None,
            class_applications: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };
        let mut same = base.clone();
        same.id = "b".into();
        assert!(base.is_same_key(&same));
    }
}

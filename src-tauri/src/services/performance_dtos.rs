//! Performans komut kontratı DTO'ları.
//!
//! `PerformanceService`'in Tauri komut katmanıyla paylaştığı typed giriş/çıkış
//! tiplerinin tek sahibi bu modüldür (TD-26). Serde kontratı burada sabittir;
//! servis mantığı `performance_service` içinde kalır. Davranış değişikliği
//! yoktur — taşıma salt tip seviyesindedir.

use serde::{Deserialize, Serialize};

use crate::domain::performance::{
    CriterionRating, PerformanceAssessmentStatus, PerformanceCriterion, PerformanceDetails,
    PerformanceLevel, PerformanceRubric, PerformanceSkillArea, PerformanceWorkMode,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePerformanceTaskInput {
    pub project_id: String,
    pub academic_year_id: String,
    pub course_id: String,
    pub course_name: String,
    pub grade_level: u32,
    pub term: u8,
    pub sequence_number: u32,
    #[serde(alias = "classSectionIds")]
    pub school_class_ids: Vec<String>,
    #[serde(default)]
    pub title: String,
    pub performance_details: PerformanceDetails,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_rubric: Option<PerformanceRubric>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePerformanceTaskInput {
    pub project_id: String,
    pub activity_id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub performance_details: Option<PerformanceDetails>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPerformanceTasksInput {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub course_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub term: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub school_class_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceActivityIdInput {
    pub project_id: String,
    pub activity_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishPerformanceRubricInput {
    pub project_id: String,
    pub activity_id: String,
    pub rubric: PerformanceRubric,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavePerformanceAssessmentInput {
    pub project_id: String,
    pub activity_id: String,
    pub application_id: String,
    pub student_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessment_id: Option<String>,
    #[serde(default)]
    pub ratings: Vec<CriterionRating>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovePerformanceAssessmentInput {
    pub project_id: String,
    pub activity_id: String,
    pub application_id: String,
    pub assessment_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPerformanceAssessmentStatusInput {
    pub project_id: String,
    pub activity_id: String,
    pub application_id: String,
    pub student_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessment_id: Option<String>,
    pub status: PerformanceAssessmentStatus,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPerformanceAssessmentsInput {
    pub project_id: String,
    pub activity_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPerformanceReportInput {
    pub project_id: String,
    pub activity_id: String,
    pub application_id: String,
}

/// Raporda ölçüt bazında bir öğrenci değerlendirmesi. Düzey/puan yoksa (henüz
/// değerlendirilmedi, Missing veya NotPerformed) alanlar `None` kalır; raporda
/// sıfırla karıştırılmaz ve boş hücre/etiket olarak gösterilir (K9).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceReportCriterionScore {
    pub criterion_id: String,
    pub criterion_name: String,
    pub level_id: Option<String>,
    pub level_name: Option<String>,
    pub points: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceReportStudentRow {
    pub student_id: String,
    pub student_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub student_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<PerformanceAssessmentStatus>,
    pub criterion_scores: Vec<PerformanceReportCriterionScore>,
    /// Yalnız onaylı (Approved) satırlarda final toplam; taslak/eksik/gösterilmedi
    /// satırlarında `None` kalır (provisional/final ayrımı, TD-07).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u32>,
    /// Onaylı veya InProgress satırlarda kaydın geçici toplamı; final toplamdan
    /// ayrı taşınır, `total` onaylı satırlarda bununla aynı değeri taşır.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provisional_total: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceReportSummary {
    pub student_count: u32,
    pub assessed_count: u32,
    pub approved_count: u32,
    pub missing_count: u32,
    pub not_performed_count: u32,
    pub unrated_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceReportDto {
    pub task_title: String,
    pub course_name: String,
    pub grade_level: u32,
    pub term: u8,
    pub sequence_number: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_area: Option<PerformanceSkillArea>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_mode: Option<PerformanceWorkMode>,
    pub class_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teacher_id: Option<String>,
    pub rubric_id: String,
    pub rubric_name: String,
    pub rubric_version: u32,
    pub criteria: Vec<PerformanceCriterion>,
    pub levels: Vec<PerformanceLevel>,
    pub max_points: u32,
    pub generated_at: String,
    pub summary: PerformanceReportSummary,
    pub rows: Vec<PerformanceReportStudentRow>,
}

/// Performans görevi için authoritative readiness snapshot'ı (TD-03).
/// Adım durumları (task/assessment/results) frontend'de türetilmez; kararlar
/// bu DTO üzerinden render edilir.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceStatusDto {
    pub has_published_rubric: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_rubric_version: Option<u32>,
    pub has_draft_rubric: bool,
    pub has_task_details: bool,
    pub total_students: u32,
    pub approved_count: u32,
    pub in_progress_count: u32,
    pub missing_count: u32,
    pub not_performed_count: u32,
    pub all_approved: bool,
}

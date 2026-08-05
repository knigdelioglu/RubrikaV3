use serde::{Deserialize, Serialize};

/// TDE beceri alanı (rapor §4: okuma, dinleme/izleme, konuşma, yazma).
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PerformanceSkillArea {
    #[default]
    Reading,
    ListeningWatching,
    Speaking,
    Writing,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PerformanceWorkMode {
    #[default]
    Individual,
    Group,
}

/// Görev metadata'sı; rubrik sürümleri de burada canonical olarak saklanır
/// (K4: rubrik sürümlemenin tek sahibi PerformanceService'tir).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceDetails {
    #[serde(default)]
    pub theme: String,
    #[serde(default)]
    pub learning_outcomes: Vec<String>,
    #[serde(default)]
    pub skill_area: PerformanceSkillArea,
    #[serde(default)]
    pub task_instruction: String,
    #[serde(default)]
    pub work_mode: PerformanceWorkMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    #[serde(default)]
    pub evidence_types: Vec<String>,
    /// Rubrik sürüm geçmişi. Sürüm 0 yayınlanmamış taslağı, >= 1 yayınlanmış
    /// sürümleri temsil eder; değerlendirme kayıtları sürüm sabitler (K8).
    #[serde(default)]
    pub rubric_versions: Vec<PerformanceRubric>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceLevel {
    pub id: String,
    pub name: String,
    pub points: u32,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LevelDescription {
    pub level_id: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceCriterion {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub level_descriptions: Vec<LevelDescription>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceRubric {
    pub id: String,
    pub name: String,
    pub version: u32,
    #[serde(default)]
    pub criteria: Vec<PerformanceCriterion>,
    #[serde(default)]
    pub levels: Vec<PerformanceLevel>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PerformanceAssessmentStatus {
    #[default]
    InProgress,
    Approved,
    NotPerformed,
    Missing,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CriterionRating {
    pub criterion_id: String,
    pub level_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// K3: değerlendirme kaydı `ClassApplication.performance_assessments` altında
/// canonical olarak saklanır. `provisional_total` yalnız servis tarafından
/// hesaplanır (istemci girdisine güvenilmez).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceAssessment {
    pub id: String,
    pub student_id: String,
    pub rubric_id: String,
    pub rubric_version: u32,
    #[serde(default)]
    pub ratings: Vec<CriterionRating>,
    #[serde(default)]
    pub provisional_total: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback: Option<String>,
    #[serde(default)]
    pub status: PerformanceAssessmentStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

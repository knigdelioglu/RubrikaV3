use crate::domain::model::ModelDiagnostics;
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SpeakingExamType {
    Prepared,
    Impromptu,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SpeakingAttemptState {
    Draft,
    Recording,
    Paused,
    Finalizing,
    CleaningTranscript,
    Evaluating,
    TeacherReview,
    Approved,
    Cancelled,
    Failed,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SpeakingCriterionRole {
    Automatic,
    AiSuggested,
    TeacherOnly,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SpeakingPerformanceLevel {
    #[serde(alias = "star_3", alias = "star3")]
    VeryGood,
    #[serde(alias = "star_2", alias = "star2")]
    Good,
    #[serde(alias = "moderate")]
    Moderate,
    #[serde(alias = "star_1", alias = "star1")]
    Developing,
    #[serde(alias = "not_evaluated")]
    NotObserved,
    PerformanceNotShown,
}

impl SpeakingPerformanceLevel {
    pub fn score_for(self, max_score: f32) -> Option<f32> {
        let max_int = max_score.round() as i32;
        let points = match self {
            Self::VeryGood => match max_int {
                5 => 5,
                10 => 10,
                15 => 15,
                20 => 20,
                _ => max_int,
            },
            Self::Good | Self::Moderate => match max_int {
                5 => 4,
                10 => 7,
                15 => 11,
                20 => 14,
                _ => (max_score * 0.7).round() as i32,
            },
            Self::Developing => match max_int {
                5 => 2,
                10 => 4,
                15 => 6,
                20 => 8,
                _ => (max_score * 0.4).round() as i32,
            },
            Self::PerformanceNotShown => 0,
            Self::NotObserved => return None,
        };
        Some(points as f32)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingModelProvenance {
    pub profile_id: String,
    pub model_family: String,
    pub model_size: String,
    pub model_file_name: String,
    pub model_file_hash: Option<String>,
    pub runtime_config_fingerprint: String,
    pub prompt_version: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub finish_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invocation: Option<crate::domain::model::ModelInvocationContract>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingCleanupChange {
    pub segment_id: String,
    pub original: String,
    pub replacement: String,
    pub change_type: String,
    pub meaning_changed: bool,
    pub confidence: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SpeakingSubindicatorRole {
    Ai,
    Automatic,
    Teacher,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingLevelPolicy {
    pub id: String,
    pub points: i32,
    #[serde(default)]
    pub mandatory_requirements: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingSubindicatorPolicy {
    pub id: String,
    pub label: String,
    pub max_points: i32,
    pub role: SpeakingSubindicatorRole,
    pub levels: Vec<SpeakingLevelPolicy>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingCriterionPolicy {
    pub criterion_id: String,
    pub subindicators: Vec<SpeakingSubindicatorPolicy>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingScoringPolicy {
    pub version: String,
    pub rounding_policy: String,
    pub tie_policy: String,
    pub criteria: Vec<SpeakingCriterionPolicy>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingSubindicatorScore {
    pub subindicator_id: String,
    /// Original model selection. The backend never mutates or hides it.
    pub selected_level_id: String,
    #[serde(default)]
    pub applied_level_id: String,
    pub points: i32,
    #[serde(default)]
    pub evidence_segment_ids: Vec<String>,
    #[serde(default)]
    pub counter_evidence_segment_ids: Vec<String>,
    #[serde(default)]
    pub missing_requirements: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ceiling_reason_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ceiling_explanation: Option<String>,
    pub rationale: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingEvaluationSubindicator {
    #[serde(alias = "subindicator_id")]
    pub subindicator_id: String,
    #[serde(alias = "selected_level_id")]
    pub selected_level_id: String,
    #[serde(default, alias = "evidence_segment_ids")]
    pub evidence_segment_ids: Vec<String>,
    #[serde(
        default,
        alias = "positive_evidence_segment_ids",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub positive_evidence_segment_ids: Vec<String>,
    #[serde(default, alias = "counter_evidence_segment_ids")]
    pub counter_evidence_segment_ids: Vec<String>,
    #[serde(default, alias = "missing_requirements")]
    pub missing_requirements: Vec<String>,
    #[serde(default)]
    pub rationale: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingEvaluationCriterion {
    #[serde(alias = "criterion_id")]
    pub criterion_id: String,
    #[serde(default, alias = "subindicators")]
    pub subindicators: Vec<SpeakingEvaluationSubindicator>,
    #[serde(default, alias = "criterion_summary")]
    pub criterion_summary: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingEvaluationOutput {
    #[serde(default)]
    pub criteria: Vec<SpeakingEvaluationCriterion>,
    #[serde(default, alias = "evaluation_confidence")]
    pub evaluation_confidence: f32,
}

pub const SPEAKING_SCORING_POLICY_VERSION: &str = "speaking_scoring_policy_v2";

pub fn default_speaking_scoring_policy() -> SpeakingScoringPolicy {
    let strong_requirements = |id: &str| -> Vec<String> {
        let requirements: &[&str] = match id {
            "task_relevance" => &[
                "Konuşmanın büyük bölümü görevle doğrudan ilgili olmalıdır.",
                "Belirgin konu dışı bölüm bulunmamalıdır.",
            ],
            "main_idea" => &[
                "Temel görüş veya ana sav açıkça anlaşılmalıdır.",
                "Yalnız konuyu adlandırmak yeterli değildir.",
            ],
            "supporting_ideas" => &[
                "En az iki farklı destekleyici fikir bulunmalıdır.",
                "Fikirler açıklama, neden veya sonuç ilişkisiyle geliştirilmelidir.",
            ],
            "examples_reasons" => &[
                "En az bir somut örnek, karşılaştırma, veri veya açık gerekçe bulunmalıdır.",
                "Kanıt ana düşünceyi gerçekten desteklemelidir.",
            ],
            "content_depth" => &[
                "Fikirler en az iki ayrı yönden geliştirilmelidir.",
                "Neden, sonuç, örnek, karşılaştırma veya çıkarım bulunmalıdır.",
            ],
            "opening" => &[
                "Konu veya amaç açıkça tanıtılmalıdır.",
                "Dinleyici konuşmanın yönünü anlayabilmelidir.",
            ],
            "idea_order" => &[
                "Fikir sırası kolayca izlenebilmelidir.",
                "Bölümler arasındaki geçişin nedeni anlaşılabilmelidir.",
            ],
            "transitions" => &[
                "En az iki işlevsel geçiş bulunmalıdır.",
                "Geçişler fikirler arasındaki ilişkiyi göstermelidir.",
            ],
            "coherence" => &[
                "Ana düşünce korunmalıdır.",
                "Belirgin çelişki veya kopukluk bulunmamalıdır.",
            ],
            "conclusion" => &[
                "Açık bir sonuç veya kapanış bölümü bulunmalıdır.",
                "Kapanış ana düşünceyi, sonucu veya çıkarımı toparlamalıdır.",
            ],
            "sentence_clarity" => &[
                "Cümlelerin çok büyük bölümü ilk dinlemede anlaşılır olmalıdır.",
                "Belirgin ve tekrar eden anlatım bozuklukları bulunmamalıdır.",
            ],
            "vocabulary_range" => &[
                "Konuya uygun çeşitli kelime ve kavramlar kullanılmalıdır.",
                "Anlamı geliştiren nitelikli sözcük seçimi görülmelidir.",
            ],
            "contextual_word_use" => &[
                "Kavramlar bağlama uygun kullanılmalıdır.",
                "Gerektiğinde kavramlar doğru biçimde açıklanmalıdır.",
            ],
            "connectors" => &[
                "Neden, sonuç, karşılaştırma, örnekleme veya sıralama ilişkileri kurulmalıdır.",
                "Bağlaç çeşitliliği ve işlevi açıkça görülmelidir.",
            ],
            "repetition_control" => &[
                "Gereksiz kelime ve fikir tekrarı olmamalı veya çok sınırlı olmalıdır.",
                "Tekrarlar varsa bilinçli vurgu işlevi görmelidir.",
            ],
            _ => &[],
        };
        requirements
            .iter()
            .map(|item| (*item).to_string())
            .collect()
    };
    let levels = |maximum: i32, subindicator_id: &str| {
        let ids = ["absent", "limited", "adequate", "strong", "excellent"];
        ids.iter()
            .enumerate()
            .map(|(index, id)| SpeakingLevelPolicy {
                id: (*id).to_string(),
                points: (index as i32).min(maximum),
                mandatory_requirements: if *id == "strong" || *id == "excellent" {
                    strong_requirements(subindicator_id)
                } else {
                    vec![]
                },
            })
            .collect::<Vec<_>>()
    };
    let criterion = |criterion_id: &str, labels: &[&str], maximum: i32| SpeakingCriterionPolicy {
        criterion_id: criterion_id.to_string(),
        subindicators: labels
            .iter()
            .map(|label| SpeakingSubindicatorPolicy {
                id: label.to_string(),
                label: label.to_string(),
                max_points: maximum,
                role: SpeakingSubindicatorRole::Ai,
                levels: levels(maximum, label),
            })
            .collect(),
    };
    SpeakingScoringPolicy {
        version: SPEAKING_SCORING_POLICY_VERSION.to_string(),
        rounding_policy: "whole_points_only".to_string(),
        tie_policy: "upper_level_only_when_all_required_conditions_are_evidenced".to_string(),
        criteria: vec![
            criterion(
                "content_main_idea",
                &[
                    "task_relevance",
                    "main_idea",
                    "supporting_ideas",
                    "examples_reasons",
                    "content_depth",
                ],
                4,
            ),
            criterion(
                "speech_structure",
                &[
                    "opening",
                    "idea_order",
                    "transitions",
                    "coherence",
                    "conclusion",
                ],
                3,
            ),
            criterion(
                "turkish_language",
                &[
                    "sentence_clarity",
                    "vocabulary_range",
                    "contextual_word_use",
                    "connectors",
                    "repetition_control",
                ],
                3,
            ),
        ],
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingPerformanceDescriptor {
    pub level: SpeakingPerformanceLevel,
    pub label: String,
    pub description: String,
    pub score_ratio: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingCriterion {
    pub id: String,
    pub label: String,
    pub description: String,
    pub max_score: f32,
    pub role: SpeakingCriterionRole,
    #[serde(default)]
    pub performance_levels: Vec<SpeakingPerformanceDescriptor>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingEvidence {
    pub start_ms: u64,
    pub end_ms: u64,
    pub quote: String,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub enum SpeakingConfidence {
    High,
    Medium,
    Low,
    #[default]
    NotEvaluated,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingCriterionScore {
    pub criterion_id: String,
    pub criterion_label: String,
    pub max_score: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub automatic_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_suggested_score: Option<f32>,
    pub ai_confidence: SpeakingConfidence,
    #[serde(default)]
    pub ai_summary: String,
    #[serde(default)]
    pub subindicator_scores: Vec<SpeakingSubindicatorScore>,
    #[serde(default)]
    pub evidence: Vec<SpeakingEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teacher_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teacher_level: Option<SpeakingPerformanceLevel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teacher_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_score: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingMetrics {
    pub duration_ms: u64,
    #[serde(default)]
    pub active_speech_duration_ms: u64,
    pub word_count: u32,
    pub words_per_minute: f32,
    pub total_silence_ms: u64,
    #[serde(default)]
    pub longest_silence_ms: u64,
    #[serde(default)]
    pub silence_ratio: f32,
    pub long_pause_count: u32,
    pub filler_count: u32,
    pub repetition_count: u32,
    pub duration_score: f32,
    #[serde(default)]
    pub expected_min_duration_ms: u64,
    #[serde(default)]
    pub sample_duration_sufficient: bool,
    #[serde(default)]
    pub measurement_confidence: SpeakingConfidence,
    #[serde(default)]
    pub clipped_sample_count: u32,
    #[serde(default)]
    pub clipping_event_count: u32,
    #[serde(default)]
    pub clipping_ratio: f32,
    #[serde(default)]
    pub peak_level: f32,
    #[serde(default)]
    pub rms_level: f32,
    #[serde(default)]
    pub low_volume_ratio: f32,
    #[serde(default)]
    pub audio_quality_confidence: SpeakingConfidence,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingTranscriptSegment {
    #[serde(default)]
    pub segment_id: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleaned_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SpeakingTranscriptCleanupStatus {
    #[default]
    #[serde(alias = "pending")]
    NotStarted,
    Running,
    #[serde(alias = "succeeded")]
    Accepted,
    NeedsReview,
    Failed,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingTranscriptCleanup {
    #[serde(default)]
    pub status: SpeakingTranscriptCleanupStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript_for_scoring: Option<String>,
    #[serde(default)]
    pub model_id: String,
    #[serde(default)]
    pub prompt_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<ModelDiagnostics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    #[serde(default)]
    pub candidate: Option<String>,
    #[serde(default)]
    pub changes: Vec<SpeakingCleanupChange>,
    #[serde(default)]
    pub needs_review: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingAttempt {
    pub id: String,
    /// Canonical organization reference. Legacy attempts may omit these fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessment_activity_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_application_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub school_class_id: Option<String>,
    pub exam_id: String,
    pub student_id: String,
    pub attempt_number: u32,
    pub state: SpeakingAttemptState,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_history_id: Option<i64>,
    #[serde(default)]
    pub raw_transcript: String,
    #[serde(default)]
    pub readable_transcript: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleanup_candidate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript_for_scoring: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_transcript: Option<String>,
    #[serde(default)]
    pub cleanup_status: SpeakingTranscriptCleanupStatus,
    #[serde(default)]
    pub cleanup_changes: Vec<SpeakingCleanupChange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleanup_diagnostics: Option<ModelDiagnostics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleanup_model_provenance: Option<SpeakingModelProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluation_model_provenance: Option<SpeakingModelProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluation_input_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen_min_duration_seconds: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen_max_duration_seconds: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_scoring_policy_version: Option<String>,
    #[serde(default)]
    pub scoring_policy_version: String,
    #[serde(default)]
    pub evaluation_prompt_version: String,
    #[serde(default)]
    pub transcript_cleanup: SpeakingTranscriptCleanup,
    #[serde(default)]
    pub transcript_segments: Vec<SpeakingTranscriptSegment>,
    #[serde(default)]
    pub metrics: SpeakingMetrics,
    #[serde(default)]
    pub criterion_scores: Vec<SpeakingCriterionScore>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluation_job_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluation_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teacher_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teacher_approved_at: Option<String>,
    #[serde(default)]
    pub model_id: String,
    #[serde(default)]
    pub prompt_version: String,
    #[serde(default)]
    pub rubric_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaking_config_snapshot: Option<crate::domain::assessment::SpeakingConfigurationSnapshot>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingExam {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessment_activity_id: Option<String>,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_id: Option<String>,
    #[serde(default)]
    pub assigned_class_ids: Vec<String>,
    pub exam_type: SpeakingExamType,
    pub task_text: String,
    pub target_duration_seconds: u32,
    pub min_duration_seconds: u32,
    pub max_duration_seconds: u32,
    pub rubric_version: String,
    #[serde(default)]
    pub scoring_policy_version: String,
    #[serde(default)]
    pub cleanup_prompt_version: String,
    #[serde(default)]
    pub evaluation_prompt_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen_model_file_hash: Option<String>,
    pub rubric_label: String,
    pub criteria: Vec<SpeakingCriterion>,
    #[serde(default)]
    pub ai_evaluation_enabled: bool,
    #[serde(default)]
    pub self_assessment_enabled: bool,
    pub status: String,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_student_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_class_application_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub attempts: Vec<SpeakingAttempt>,
}

impl SpeakingExam {
    pub fn assigned_class_ids(&self) -> Vec<String> {
        if !self.assigned_class_ids.is_empty() {
            let mut ids = Vec::new();
            for id in &self.assigned_class_ids {
                if !ids.contains(id) {
                    ids.push(id.clone());
                }
            }
            ids
        } else if let Some(class_id) = &self.class_id {
            vec![class_id.clone()]
        } else {
            vec![]
        }
    }
}

pub fn prepared_rubric() -> (String, String, Vec<SpeakingCriterion>) {
    (
        "tymm-prepared-speaking-v1".to_string(),
        "TYMM Hazırlıklı Konuşma v1".to_string(),
        vec![
            criterion(
                "content_main_idea",
                "Konuya uygunluk, içerik ve ana düşünce",
                "Görevle ilişki, ana düşünce, destekleyici fikirler ve örnekler.",
                20.0,
                SpeakingCriterionRole::AiSuggested,
            ),
            criterion(
                "speech_structure",
                "Konuşma planı ve anlam bütünlüğü",
                "Giriş, gelişme, geçişler, tutarlılık ve sonuç.",
                15.0,
                SpeakingCriterionRole::AiSuggested,
            ),
            criterion(
                "turkish_language",
                "Türkçenin doğru kullanımı ve söz varlığı",
                "Cümle kuruluşu, sözcük seçimi, bağlaçlar ve anlatım.",
                15.0,
                SpeakingCriterionRole::AiSuggested,
            ),
            criterion(
                "fluency_automatic",
                "Akıcılık otomatik göstergeleri",
                "Hız, duraklama, dolgu ve tekrar ölçümleri.",
                5.0,
                SpeakingCriterionRole::Automatic,
            ),
            criterion(
                "fluency_presentation",
                "Ses, diksiyon ve telaffuz",
                "Anlaşılabilirlik, diksiyon, vurgu ve tonlama.",
                10.0,
                SpeakingCriterionRole::TeacherOnly,
            ),
            criterion(
                "preparation",
                "Hazırlık, araştırma, prova ve materyal",
                "Hazırlık belgeleri ve materyalin konuşmaya katkısı.",
                15.0,
                SpeakingCriterionRole::TeacherOnly,
            ),
            criterion(
                "body_language",
                "Beden dili, mekân ve iletişim",
                "Göz teması, jest, duruş, mekân ve dinleyiciyle iletişim.",
                10.0,
                SpeakingCriterionRole::TeacherOnly,
            ),
            criterion(
                "duration_management",
                "Süreyi yönetme",
                "Belirlenen alt ve üst sınırlar içinde konuşma.",
                5.0,
                SpeakingCriterionRole::Automatic,
            ),
            criterion(
                "self_assessment",
                "Öz değerlendirme ve gelişim hedefi",
                "Öğrencinin güçlü yönü ve sonraki hedefi.",
                5.0,
                SpeakingCriterionRole::TeacherOnly,
            ),
        ],
    )
}

pub fn impromptu_rubric() -> (String, String, Vec<SpeakingCriterion>) {
    (
        "tymm-impromptu-speaking-v1".to_string(),
        "TYMM Hazırlıksız Konuşma v1".to_string(),
        vec![
            criterion(
                "topic_thinking",
                "Konuya uygunluk ve düşünce üretme",
                "Konuyu anlamlandırma ve düşünce geliştirme.",
                20.0,
                SpeakingCriterionRole::AiSuggested,
            ),
            criterion(
                "impromptu_structure",
                "Anlık planlama, sıralama ve tutarlılık",
                "Fikirleri anlık düzenleme ve konuşmayı sürdürme.",
                20.0,
                SpeakingCriterionRole::AiSuggested,
            ),
            criterion(
                "turkish_language",
                "Türkçenin doğru kullanımı ve söz varlığı",
                "Cümle kuruluşu, sözcük seçimi ve anlatım.",
                15.0,
                SpeakingCriterionRole::AiSuggested,
            ),
            criterion(
                "fluency_automatic",
                "Akıcılık otomatik göstergeleri",
                "Hız, duraklama, dolgu ve tekrar ölçümleri.",
                5.0,
                SpeakingCriterionRole::Automatic,
            ),
            criterion(
                "fluency_presentation",
                "Ses, diksiyon ve telaffuz",
                "Anlaşılabilirlik, diksiyon, vurgu ve tonlama.",
                10.0,
                SpeakingCriterionRole::TeacherOnly,
            ),
            criterion(
                "body_language",
                "Beden dili ve dinleyiciyle etkileşim",
                "Göz teması, jest, duruş ve dinleyiciyle iletişim.",
                15.0,
                SpeakingCriterionRole::TeacherOnly,
            ),
            criterion(
                "recovery",
                "Zorlanmayı yönetme ve yeniden ifade",
                "Uzun duraklamadan sonra sürdürme ve yeniden ifade.",
                10.0,
                SpeakingCriterionRole::TeacherOnly,
            ),
            criterion(
                "duration_management",
                "Süreyi yönetme",
                "Belirlenen alt ve üst sınırlar içinde konuşma.",
                5.0,
                SpeakingCriterionRole::Automatic,
            ),
        ],
    )
}

fn criterion(
    id: &str,
    label: &str,
    description: &str,
    max_score: f32,
    role: SpeakingCriterionRole,
) -> SpeakingCriterion {
    SpeakingCriterion {
        id: id.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        max_score,
        role,
        performance_levels: performance_descriptors(description),
    }
}

fn performance_descriptors(focus: &str) -> Vec<SpeakingPerformanceDescriptor> {
    [
        (
            SpeakingPerformanceLevel::VeryGood,
            "Çok iyi",
            1.0,
            "Tutarlı, belirgin ve etkili biçimde gözlenir.",
        ),
        (
            SpeakingPerformanceLevel::Good,
            "İyi",
            0.85,
            "Genellikle etkili; küçük ve seyrek eksikler vardır.",
        ),
        (
            SpeakingPerformanceLevel::Moderate,
            "Orta",
            0.65,
            "Kısmen gözlenir; etki ve tutarlılık değişkendir.",
        ),
        (
            SpeakingPerformanceLevel::Developing,
            "Geliştirilebilir",
            0.4,
            "Sınırlı gözlenir; belirgin öğretmen desteği ve gelişim gerekir.",
        ),
        (
            SpeakingPerformanceLevel::NotObserved,
            "Gözlenmedi",
            0.0,
            "Bu oturumda güvenilir biçimde gözlenemedi; puan yerine eksik değerlendirme sayılır.",
        ),
    ]
    .into_iter()
    .map(
        |(level, label, score_ratio, suffix)| SpeakingPerformanceDescriptor {
            level,
            label: label.to_string(),
            description: format!("{focus} {suffix}"),
            score_ratio,
        },
    )
    .collect()
}

pub fn new_exam(
    title: String,
    assigned_class_ids: Vec<String>,
    exam_type: SpeakingExamType,
    task_text: String,
    target_seconds: u32,
    min_seconds: u32,
    max_seconds: u32,
) -> SpeakingExam {
    let (rubric_version, rubric_label, criteria) = match exam_type {
        SpeakingExamType::Prepared => prepared_rubric(),
        SpeakingExamType::Impromptu => impromptu_rubric(),
    };
    let mut clean_assigned_ids = Vec::new();
    for id in assigned_class_ids {
        if !id.trim().is_empty() && !clean_assigned_ids.contains(&id) {
            clean_assigned_ids.push(id);
        }
    }
    let class_id = clean_assigned_ids.first().cloned();
    SpeakingExam {
        id: uuid::Uuid::new_v4().to_string(),
        assessment_activity_id: None,
        title,
        class_id,
        assigned_class_ids: clean_assigned_ids,
        exam_type,
        task_text,
        target_duration_seconds: target_seconds,
        min_duration_seconds: min_seconds,
        max_duration_seconds: max_seconds,
        rubric_version,
        scoring_policy_version: SPEAKING_SCORING_POLICY_VERSION.to_string(),
        cleanup_prompt_version: "speaking_asr_cleanup_tr_v3".to_string(),
        evaluation_prompt_version: "speaking_rubric_evidence_tr_v3".to_string(),
        frozen_model_file_hash: None,
        rubric_label,
        criteria,
        ai_evaluation_enabled: true,
        self_assessment_enabled: false,
        status: "active".to_string(),
        created_at: Utc::now().to_rfc3339(),
        updated_at: Utc::now().to_rfc3339(),
        active_student_id: None,
        active_class_application_id: None,
        completed_at: None,
        attempts: vec![],
    }
}

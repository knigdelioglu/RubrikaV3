use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use crate::domain::analysis::{
    AnalysisCriterionSummary, AnalysisScoreBand, AnalysisStatus, AnalysisStudentSummary,
    AssessmentAnalysis, AssessmentKind,
};
use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::job::JobKind;
use crate::domain::model::{AnalysisReportRequest, SPEAKING_RUBRIC_PROFILE_ID};
use crate::domain::project::Project;
use crate::domain::scoring::{scoring_active_records, ScoringReviewStatus};
use crate::domain::speaking::SpeakingAttemptState;
use crate::jobs::job_manager::JobManager;
use crate::platform::file_access::atomic_write;
use crate::services::model_gateway::ModelGateway;
use crate::services::model_runtime_service::{
    ModelCapability, ModelRuntimeRequest, ModelRuntimeService, ModelUseCase,
};
use crate::services::project_store::ProjectStore;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishAssessmentOutput {
    pub analysis_id: String,
    pub job_id: String,
    pub status: String,
}

#[derive(Clone)]
pub struct AnalysisService {
    project_store: ProjectStore,
    model_gateway: Arc<dyn ModelGateway>,
    model_runtime_service: ModelRuntimeService,
    job_manager: Arc<JobManager>,
}

impl AnalysisService {
    pub fn new(
        project_store: ProjectStore,
        model_gateway: Arc<dyn ModelGateway>,
        model_runtime_service: ModelRuntimeService,
        job_manager: Arc<JobManager>,
    ) -> Self {
        Self {
            project_store,
            model_gateway,
            model_runtime_service,
            job_manager,
        }
    }

    pub async fn finish<R: tauri::Runtime>(
        &self,
        app: tauri::AppHandle<R>,
        project_id: String,
        kind: AssessmentKind,
        source_id: Option<String>,
    ) -> Result<FinishAssessmentOutput, AppError> {
        let mut project = self
            .project_store
            .get_project_snapshot(project_id.clone())?;
        let analysis = match kind {
            AssessmentKind::Speaking => {
                let source_id = source_id
                    .as_deref()
                    .ok_or_else(|| analysis_not_ready("Konuşma sınavı seçilmedi."))?;
                let analysis = build_speaking_analysis(&project, source_id)?;
                if let Some(exam) = project
                    .speaking_exams
                    .iter_mut()
                    .find(|exam| exam.id == source_id)
                {
                    let now = chrono::Utc::now().to_rfc3339();
                    exam.status = "completed".to_string();
                    exam.completed_at = Some(now.clone());
                    exam.updated_at = now;
                    exam.active_student_id = None;
                }
                self.project_store.save_project(&project)?;
                analysis
            }
            AssessmentKind::Written => build_written_analysis(&project)?,
        };
        self.save_analysis(&project.root_path, &analysis)?;

        let job = self.job_manager.start_job(
            &app,
            project_id,
            Some(project.root_path.clone()),
            JobKind::AssessmentAnalysis,
            3,
            "Sınav analiz grafikleri hazırlandı; Gemma raporu bekleniyor.".to_string(),
        )?;

        let service = self.clone();
        let app_for_job = app.clone();
        let job_id = job.id.clone();
        let analysis_id = analysis.id.clone();
        let project_root = project.root_path.clone();
        tokio::spawn(async move {
            service
                .generate_report(app_for_job, job_id, project_root, analysis)
                .await;
        });

        Ok(FinishAssessmentOutput {
            analysis_id,
            job_id: job.id,
            status: "queued".to_string(),
        })
    }

    pub fn get(&self, project_id: &str, analysis_id: &str) -> Result<AssessmentAnalysis, AppError> {
        let project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        let path = analysis_path(&project.root_path, analysis_id)?;
        read_analysis(&path)
    }

    pub fn list(&self, project_id: &str) -> Result<Vec<AssessmentAnalysis>, AppError> {
        let project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        let dir = analysis_dir(&project.root_path);
        if !dir.exists() {
            return Ok(vec![]);
        }
        let mut analyses = std::fs::read_dir(&dir)
            .map_err(|error| analysis_io_error("Analiz klasörü okunamadı.", &dir, error))?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("json"))
            .filter_map(|entry| read_analysis(&entry.path()).ok())
            .collect::<Vec<_>>();
        analyses.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(analyses)
    }

    async fn generate_report<R: tauri::Runtime>(
        &self,
        app: tauri::AppHandle<R>,
        job_id: String,
        project_root: String,
        mut analysis: AssessmentAnalysis,
    ) {
        let run = async {
            self.job_manager.set_running(&app, &job_id)?;
            self.job_manager.update_progress(
                &app,
                &job_id,
                1,
                3,
                "Toplu sınav ölçümleri Gemma için hazırlanıyor.".to_string(),
            )?;
            let runtime_request = ModelRuntimeRequest {
                use_case: ModelUseCase::GeneralText,
                capability: ModelCapability::Text,
                requires_mmproj: false,
                timeout_seconds: 90,
            };
            let _ = self.model_runtime_service.stop_server(None).await;
            self.model_runtime_service
                .ensure_ready(Some(SPEAKING_RUBRIC_PROFILE_ID), runtime_request)
                .await?;
            self.job_manager.update_progress(
                &app,
                &job_id,
                2,
                3,
                "Gemma 4 12B öğretmen raporunu yazıyor.".to_string(),
            )?;

            let report_result = self
                .model_gateway
                .generate_analysis_report(AnalysisReportRequest {
                    prompt: build_analysis_prompt(&analysis),
                })
                .await;
            let _ = self
                .model_runtime_service
                .stop_server(Some(SPEAKING_RUBRIC_PROFILE_ID))
                .await;
            report_result
        }
        .await;

        match run {
            Ok(result) => {
                analysis.status = AnalysisStatus::Ready;
                analysis.model_report = Some(result.report);
                analysis.model_report_error = None;
                analysis.completed_at = Some(chrono::Utc::now().to_rfc3339());
                if let Err(error) = self.save_analysis(&project_root, &analysis) {
                    let _ = self.job_manager.fail(&app, &job_id, error);
                    return;
                }
                let _ = self.job_manager.update_progress(
                    &app,
                    &job_id,
                    3,
                    3,
                    "Sınav analizi tamamlandı.".to_string(),
                );
                let _ = self.job_manager.succeed(
                    &app,
                    &job_id,
                    Some(json!({ "analysisId": analysis.id })),
                );
            }
            Err(error) => {
                let _ = self
                    .model_runtime_service
                    .stop_server(Some(SPEAKING_RUBRIC_PROFILE_ID))
                    .await;
                analysis.status = AnalysisStatus::Partial;
                analysis.model_report_error = Some(error.message.clone());
                analysis.completed_at = Some(chrono::Utc::now().to_rfc3339());
                if let Err(save_error) = self.save_analysis(&project_root, &analysis) {
                    let _ = self.job_manager.fail(&app, &job_id, save_error);
                    return;
                }
                let _ = self.job_manager.partial(
                    &app,
                    &job_id,
                    Some(json!({
                        "analysisId": analysis.id,
                        "message": "Grafikler hazır; Gemma raporu yeniden denenebilir."
                    })),
                );
            }
        }
    }

    fn save_analysis(
        &self,
        project_root: &str,
        analysis: &AssessmentAnalysis,
    ) -> Result<(), AppError> {
        let path = analysis_path(project_root, &analysis.id)?;
        let content = serde_json::to_string_pretty(analysis).map_err(|error| AppError {
            code: AppErrorCode::AnalysisFailed,
            message: "Analiz verisi hazırlanamadı.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        atomic_write(&path, &content)
            .map_err(|error| analysis_io_error("Analiz kaydedilemedi.", &path, error))
    }
}

fn build_speaking_analysis(
    project: &Project,
    exam_id: &str,
) -> Result<AssessmentAnalysis, AppError> {
    let exam = project
        .speaking_exams
        .iter()
        .find(|exam| exam.id == exam_id)
        .ok_or_else(|| analysis_not_ready("Konuşma sınavı bulunamadı."))?;
    let attempts = exam
        .attempts
        .iter()
        .filter(|attempt| {
            attempt.state == SpeakingAttemptState::Approved && attempt.final_score.is_some()
        })
        .collect::<Vec<_>>();
    if attempts.is_empty() {
        return Err(analysis_not_ready(
            "Analiz için en az bir öğrenci puanlanıp onaylanmalıdır.",
        ));
    }

    let mut students = attempts
        .iter()
        .map(|attempt| {
            let name = project
                .students
                .iter()
                .find(|student| student.id == attempt.student_id)
                .and_then(|student| student.display_name.clone())
                .unwrap_or_else(|| "Öğrenci".to_string());
            let max_score = exam
                .criteria
                .iter()
                .map(|criterion| criterion.max_score)
                .sum();
            student_summary(
                attempt.student_id.clone(),
                name,
                attempt.final_score.unwrap_or_default(),
                max_score,
            )
        })
        .collect::<Vec<_>>();
    students.sort_by(|left, right| left.display_name.cmp(&right.display_name));

    let criteria = exam
        .criteria
        .iter()
        .filter_map(|criterion| {
            let scores = attempts
                .iter()
                .filter_map(|attempt| {
                    attempt
                        .criterion_scores
                        .iter()
                        .find(|score| score.criterion_id == criterion.id)
                        .and_then(|score| score.final_score)
                })
                .collect::<Vec<_>>();
            criterion_summary(
                criterion.id.clone(),
                criterion.label.clone(),
                criterion.max_score,
                &scores,
            )
        })
        .collect();

    Ok(base_analysis(
        project,
        AssessmentKind::Speaking,
        Some(exam.id.clone()),
        exam.title.clone(),
        exam.class_id.clone(),
        criteria,
        students,
    ))
}

fn build_written_analysis(project: &Project) -> Result<AssessmentAnalysis, AppError> {
    let records = scoring_active_records(project)
        .into_iter()
        .filter(|record| {
            record.teacher_review_status != ScoringReviewStatus::Invalidated
                && (record.teacher_manual_score.is_some()
                    || (record.scoring_applied && record.awarded_score.is_some()))
        })
        .collect::<Vec<_>>();
    if records.is_empty() {
        return Err(analysis_not_ready(
            "Yazılı sınav analizi için kaydedilmiş puan bulunmuyor.",
        ));
    }

    let mut student_totals: BTreeMap<String, (String, f32, f32)> = BTreeMap::new();
    let mut question_scores: BTreeMap<(String, u32), (f32, Vec<f32>)> = BTreeMap::new();
    for record in records {
        let score = record
            .teacher_manual_score
            .or(record.awarded_score)
            .unwrap_or_default();
        let display_name = record
            .student_display_name
            .clone()
            .or_else(|| {
                project
                    .students
                    .iter()
                    .find(|student| student.id == record.student_id)
                    .and_then(|student| student.display_name.clone())
            })
            .unwrap_or_else(|| "Öğrenci".to_string());
        let entry =
            student_totals
                .entry(record.student_id.clone())
                .or_insert((display_name, 0.0, 0.0));
        entry.1 += score;
        entry.2 += record.max_score;
        question_scores
            .entry((record.question_id.clone(), record.question_number))
            .or_insert((record.max_score, vec![]))
            .1
            .push(score);
    }

    let students = student_totals
        .into_iter()
        .map(|(student_id, (display_name, score, max_score))| {
            student_summary(student_id, display_name, score, max_score)
        })
        .collect::<Vec<_>>();
    let criteria = question_scores
        .into_iter()
        .filter_map(|((question_id, question_number), (max_score, scores))| {
            criterion_summary(
                question_id,
                format!("Soru {question_number}"),
                max_score,
                &scores,
            )
        })
        .collect::<Vec<_>>();
    Ok(base_analysis(
        project,
        AssessmentKind::Written,
        project.latest_scoring_run_id.clone(),
        project.name.clone(),
        None,
        criteria,
        students,
    ))
}

fn base_analysis(
    project: &Project,
    kind: AssessmentKind,
    source_id: Option<String>,
    title: String,
    class_id: Option<String>,
    criteria: Vec<AnalysisCriterionSummary>,
    students: Vec<AnalysisStudentSummary>,
) -> AssessmentAnalysis {
    AssessmentAnalysis {
        id: Uuid::new_v4().to_string(),
        project_id: project.id.clone(),
        kind,
        source_id,
        title,
        class_id,
        status: AnalysisStatus::Generating,
        student_count: students.len() as u32,
        score_bands: score_bands(&students),
        criteria,
        students,
        model_report: None,
        model_report_error: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
    }
}

fn student_summary(
    student_id: String,
    display_name: String,
    score: f32,
    max_score: f32,
) -> AnalysisStudentSummary {
    AnalysisStudentSummary {
        student_id,
        display_name,
        score: round_one(score),
        max_score: round_one(max_score),
        percentage: percentage(score, max_score),
    }
}

fn criterion_summary(
    id: String,
    label: String,
    max_score: f32,
    scores: &[f32],
) -> Option<AnalysisCriterionSummary> {
    if scores.is_empty() {
        return None;
    }
    let average = scores.iter().sum::<f32>() / scores.len() as f32;
    Some(AnalysisCriterionSummary {
        id,
        label,
        average_score: round_one(average),
        max_score: round_one(max_score),
        percentage: percentage(average, max_score),
        sample_count: scores.len() as u32,
    })
}

fn score_bands(students: &[AnalysisStudentSummary]) -> Vec<AnalysisScoreBand> {
    [
        ("Geliştirilmeli", 0.0, 49.9),
        ("Temel", 50.0, 69.9),
        ("İyi", 70.0, 84.9),
        ("Çok iyi", 85.0, 100.0),
    ]
    .into_iter()
    .map(|(label, minimum, maximum)| AnalysisScoreBand {
        label: label.to_string(),
        minimum,
        maximum,
        count: students
            .iter()
            .filter(|student| student.percentage >= minimum && student.percentage <= maximum)
            .count() as u32,
    })
    .collect()
}

fn percentage(score: f32, max_score: f32) -> f32 {
    if max_score <= 0.0 {
        0.0
    } else {
        round_one((score / max_score * 100.0).clamp(0.0, 100.0))
    }
}

fn round_one(value: f32) -> f32 {
    (value * 10.0).round() / 10.0
}

fn build_analysis_prompt(analysis: &AssessmentAnalysis) -> String {
    let kind = match analysis.kind {
        AssessmentKind::Written => "yazılı",
        AssessmentKind::Speaking => "konuşma",
    };
    let criteria = analysis
        .criteria
        .iter()
        .map(|criterion| {
            format!(
                "- {}: %{:.1} (ortalama {:.1}/{:.1}, {} öğrenci)",
                criterion.label,
                criterion.percentage,
                criterion.average_score,
                criterion.max_score,
                criterion.sample_count
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let bands = analysis
        .score_bands
        .iter()
        .map(|band| format!("- {}: {} öğrenci", band.label, band.count))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Sen deneyimli bir ölçme-değerlendirme uzmanısın. Aşağıdaki anonim ve toplu {kind} sınavı \
         ölçümlerine dayanarak öğretmen için kısa, kanıta bağlı bir Türkçe rapor yaz.\n\
         Zorunlu başlıklar: Genel görünüm, Güçlü alanlar, Gelişim alanları, Sonraki ders için 3 somut öneri.\n\
         Veri dışı kişilik çıkarımı, tanı, öğrenci ismi, uydurma neden veya uydurma puan üretme.\n\
         Sınav: {}\nÖğrenci sayısı: {}\nRubrik/soru ölçümleri:\n{}\nBaşarı dağılımı:\n{}",
        analysis.title, analysis.student_count, criteria, bands
    )
}

fn analysis_dir(project_root: &str) -> PathBuf {
    Path::new(project_root).join("outputs").join("analysis")
}

fn analysis_path(project_root: &str, analysis_id: &str) -> Result<PathBuf, AppError> {
    if analysis_id.is_empty()
        || analysis_id.contains('/')
        || analysis_id.contains('\\')
        || analysis_id.contains("..")
    {
        return Err(AppError {
            code: AppErrorCode::PermissionDenied,
            message: "Analiz kimliği güvenli değil.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(format!("analysis_id={analysis_id:?}")),
            correlation_id: Uuid::new_v4().to_string(),
        });
    }
    Ok(analysis_dir(project_root).join(format!("{analysis_id}.json")))
}

fn read_analysis(path: &Path) -> Result<AssessmentAnalysis, AppError> {
    let content = std::fs::read_to_string(path)
        .map_err(|error| analysis_io_error("Analiz bulunamadı.", path, error))?;
    serde_json::from_str(&content).map_err(|error| AppError {
        code: AppErrorCode::AnalysisFailed,
        message: "Analiz dosyası geçersiz.".to_string(),
        recoverable: true,
        suggested_action: Some("Sınav analizini yeniden oluşturun.".to_string()),
        technical_details: Some(format!("path={}; error={error}", path.display())),
        correlation_id: Uuid::new_v4().to_string(),
    })
}

fn analysis_io_error(message: &str, path: &Path, error: std::io::Error) -> AppError {
    AppError {
        code: AppErrorCode::AnalysisFailed,
        message: message.to_string(),
        recoverable: true,
        suggested_action: Some("Disk alanı ve proje klasörü izinlerini kontrol edin.".to_string()),
        technical_details: Some(format!("path={}; error={error}", path.display())),
        correlation_id: Uuid::new_v4().to_string(),
    }
}

fn analysis_not_ready(message: &str) -> AppError {
    AppError {
        code: AppErrorCode::AnalysisNotReady,
        message: message.to_string(),
        recoverable: true,
        suggested_action: Some(
            "Öğrenci değerlendirmelerini tamamlayıp puanları kaydedin.".to_string(),
        ),
        technical_details: None,
        correlation_id: Uuid::new_v4().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{criterion_summary, score_bands, student_summary};

    #[test]
    fn criterion_summary_uses_only_observed_scores() {
        let summary = criterion_summary("c1".into(), "İçerik".into(), 20.0, &[10.0, 20.0]).unwrap();
        assert_eq!(summary.average_score, 15.0);
        assert_eq!(summary.percentage, 75.0);
        assert_eq!(summary.sample_count, 2);
    }

    #[test]
    fn score_bands_cover_boundary_values() {
        let students = vec![
            student_summary("1".into(), "A".into(), 49.9, 100.0),
            student_summary("2".into(), "B".into(), 50.0, 100.0),
            student_summary("3".into(), "C".into(), 85.0, 100.0),
        ];
        let bands = score_bands(&students);
        assert_eq!(bands.iter().map(|band| band.count).sum::<u32>(), 3);
        assert_eq!(bands[0].count, 1);
        assert_eq!(bands[1].count, 1);
        assert_eq!(bands[3].count, 1);
    }
}

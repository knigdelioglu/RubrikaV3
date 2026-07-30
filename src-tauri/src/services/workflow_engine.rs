use crate::domain::document::DocumentRole;
use crate::domain::document::PdfPreviewStatus;
use crate::domain::job::{JobSnapshot, JobStatus};
use crate::domain::project::{ExamPackageFreezeStatus, Project};
use crate::domain::question::{is_question_text_ready, TextFieldStatus};
use crate::domain::rubric::{is_rubric_confirmed, validate_rubric_state, RubricStatus};
use crate::domain::scoring::scoring_readiness;
use crate::domain::student::student_identity_is_missing;
use crate::domain::workflow::{BlockingReason, WorkflowAction, WorkflowSnapshot, WorkflowStage};

use crate::domain::workflow::{WorkflowReadiness, WorkflowStep, WorkflowSummary};

pub fn evaluate_workflow(project: &Project) -> WorkflowSnapshot {
    evaluate_workflow_with_context(
        project,
        &crate::domain::model::ModelStatus::default(),
        false,
        false,
    )
}

pub fn evaluate_workflow_with_context(
    project: &Project,
    model_status: &crate::domain::model::ModelStatus,
    question_text_job_active: bool,
    student_answer_ocr_job_active: bool,
) -> WorkflowSnapshot {
    let (current_stage, blocking_reasons, next_actions, text) = evaluate_workflow_inner(
        project,
        model_status,
        question_text_job_active,
        student_answer_ocr_job_active,
    );

    let current_stage_label = get_stage_label(&current_stage);

    let exam_source_docs: Vec<_> = project
        .documents
        .iter()
        .filter(|d| d.role == DocumentRole::ExamSource)
        .collect();
    let student_scan_docs: Vec<_> = project
        .documents
        .iter()
        .filter(|d| d.role == DocumentRole::StudentScan)
        .collect();

    let preview_ready = !exam_source_docs.is_empty()
        && exam_source_docs.iter().all(|d| {
            matches!(
                d.preview.as_ref().map(|p| &p.status),
                Some(PdfPreviewStatus::Ready)
            ) || check_preview_cache_valid(&project.root_path, &d.id)
        });
    let student_scan_preview_ready = !student_scan_docs.is_empty()
        && student_scan_docs.iter().all(|d| {
            matches!(
                d.preview.as_ref().map(|p| &p.status),
                Some(PdfPreviewStatus::Ready)
            ) || check_preview_cache_valid(&project.root_path, &d.id)
        });

    let question_ready = !project.questions.is_empty()
        && project.questions.iter().all(|q| {
            matches!(
                q.question_text.status,
                TextFieldStatus::Suggested | TextFieldStatus::Confirmed | TextFieldStatus::Edited
            )
        });
    let question_partial = !project.questions.is_empty()
        && !question_ready
        && project.questions.iter().any(|q| {
            matches!(
                q.question_text.status,
                TextFieldStatus::Suggested | TextFieldStatus::Confirmed | TextFieldStatus::Edited
            )
        });
    let rubric_ready = !project.questions.is_empty()
        && project.questions.iter().all(|q| {
            matches!(
                q.rubric.status,
                RubricStatus::Suggested
                    | RubricStatus::Imported
                    | RubricStatus::Manual
                    | RubricStatus::Confirmed
            )
        });
    let rubric_partial = !project.questions.is_empty()
        && !rubric_ready
        && project.questions.iter().any(|q| {
            matches!(
                q.rubric.status,
                RubricStatus::Suggested
                    | RubricStatus::Imported
                    | RubricStatus::Manual
                    | RubricStatus::Confirmed
            )
        });

    let student_scan_progress =
        student_scan_preview_progress(&project.root_path, &student_scan_docs);
    let student_answer_ocr_total = project.student_submissions.len() * project.questions.len();
    let student_answer_ocr_records = project.student_answer_ocr_records.len();
    let student_answer_ocr_reviewed = project
        .student_answer_ocr_records
        .iter()
        .filter(|record| {
            matches!(
                record.status,
                crate::domain::student::StudentAnswerOcrStatus::TeacherApproved
            )
        })
        .count();
    let student_answer_ocr_all_reviewed = student_answer_ocr_total > 0
        && student_answer_ocr_records == student_answer_ocr_total
        && student_answer_ocr_reviewed == student_answer_ocr_total;
    let scoring_state = scoring_readiness(project);
    let scoring_complete = scoring_state.expected_records > 0
        && scoring_state.scoring_record_count == scoring_state.expected_records;
    let scoring_running = matches!(
        project.workflow.current_stage,
        WorkflowStage::ScoringRunning
    ) && !scoring_complete;

    let steps = vec![
        WorkflowStep {
            code: "pdf_preview_render".to_string(),
            label: "Sınav PDF önizlemesi".to_string(),
            status: if preview_ready {
                "succeeded".to_string()
            } else {
                "pending".to_string()
            },
            message: if preview_ready {
                "Hazır".to_string()
            } else {
                "Bekleniyor".to_string()
            },
            current: None,
            total: None,
        },
        WorkflowStep {
            code: "student_scan_preview_render".to_string(),
            label: "Öğrenci PDF önizlemesi".to_string(),
            status: if student_scan_docs.is_empty() {
                "pending".to_string()
            } else if student_scan_preview_ready {
                "succeeded".to_string()
            } else if student_scan_docs.iter().any(|document| {
                matches!(
                    document.preview.as_ref().map(|preview| &preview.status),
                    Some(PdfPreviewStatus::Queued | PdfPreviewStatus::Running)
                )
            }) {
                "running".to_string()
            } else {
                "partial".to_string()
            },
            message: if student_scan_docs.is_empty() {
                "Henüz yüklenmedi".to_string()
            } else if student_scan_preview_ready {
                "Hazır".to_string()
            } else if student_scan_docs.iter().any(|document| {
                matches!(
                    document.preview.as_ref().map(|preview| &preview.status),
                    Some(PdfPreviewStatus::Queued | PdfPreviewStatus::Running)
                )
            }) {
                "Önizleme oluşturuluyor".to_string()
            } else {
                "Önizleme kısmi".to_string()
            },
            current: student_scan_progress.as_ref().map(|(current, _)| *current),
            total: student_scan_progress.as_ref().map(|(_, total)| *total),
        },
        WorkflowStep {
            code: "question_text_extraction".to_string(),
            label: "Soru metinleri".to_string(),
            status: if question_ready {
                "succeeded".to_string()
            } else if question_partial {
                "partial".to_string()
            } else {
                "pending".to_string()
            },
            message: if question_ready {
                "Hazır".to_string()
            } else if question_partial {
                "Kısmi".to_string()
            } else {
                "Bekleniyor".to_string()
            },
            current: None,
            total: None,
        },
        WorkflowStep {
            code: "rubric_pdf_import".to_string(),
            label: "Rubrikler".to_string(),
            status: if rubric_ready {
                "succeeded".to_string()
            } else if rubric_partial {
                "partial".to_string()
            } else {
                "pending".to_string()
            },
            message: if rubric_ready {
                "Hazır".to_string()
            } else if rubric_partial {
                "Kısmi".to_string()
            } else {
                "Bekleniyor".to_string()
            },
            current: None,
            total: None,
        },
        WorkflowStep {
            code: "student_answer_ocr".to_string(),
            label: "Öğrenci cevap OCR’ı".to_string(),
            status: if student_answer_ocr_job_active {
                "running".to_string()
            } else if student_answer_ocr_records == 0 {
                "pending".to_string()
            } else if student_answer_ocr_all_reviewed {
                "succeeded".to_string()
            } else {
                "partial".to_string()
            },
            message: if student_answer_ocr_job_active {
                "Çalışıyor".to_string()
            } else if student_answer_ocr_records == 0 {
                "Başlatılmadı".to_string()
            } else if student_answer_ocr_all_reviewed {
                "Onaylandı".to_string()
            } else {
                "Kontrol bekliyor".to_string()
            },
            current: Some(student_answer_ocr_reviewed as u32),
            total: Some(student_answer_ocr_total as u32),
        },
        WorkflowStep {
            code: "scoring".to_string(),
            label: "Notlandırma".to_string(),
            status: if scoring_running {
                "running".to_string()
            } else if scoring_state.stale_record_count > 0 {
                "partial".to_string()
            } else if scoring_complete && scoring_state.needs_review_record_count == 0 {
                "succeeded".to_string()
            } else if scoring_state.scoring_record_count > 0 {
                "partial".to_string()
            } else {
                "pending".to_string()
            },
            message: if scoring_running {
                "Çalışıyor".to_string()
            } else if scoring_state.scoring_record_count == 0 {
                "Başlatılmadı".to_string()
            } else if scoring_complete && scoring_state.needs_review_record_count == 0 {
                "Tamamlandı".to_string()
            } else if scoring_state.needs_review_record_count > 0 {
                "Öğretmen kontrolü bekliyor".to_string()
            } else if scoring_state.stale_record_count > 0 {
                "Notlar yeniden çalıştırılmalı".to_string()
            } else {
                "Tamamlandı".to_string()
            },
            current: Some(scoring_state.approved_record_count as u32),
            total: Some(scoring_state.expected_records as u32),
        },
    ];

    let exam_package_freeze = exam_package_is_ready_to_freeze(project);
    let student_intake = !student_scan_docs.is_empty()
        && student_scan_preview_ready
        && student_grouping_is_complete(project);
    let scoring = scoring_state.ready
        || scoring_complete
        || matches!(
            project.workflow.current_stage,
            WorkflowStage::ScoringRunning
        );

    WorkflowSnapshot {
        current_stage,
        current_stage_label,
        blocking_reasons,
        next_actions,
        summary: WorkflowSummary {
            text,
            steps,
            readiness: WorkflowReadiness {
                exam_package_freeze,
                student_intake,
                scoring,
            },
        },
    }
}

fn exam_package_is_ready_to_freeze(project: &Project) -> bool {
    let expected_count = project
        .expected_question_count
        .unwrap_or(project.questions.len() as u32);
    expected_count > 0
        && project.questions.len() == expected_count as usize
        && (1..=expected_count).all(|number| {
            project.questions.iter().any(|question| {
                question.number == number
                    && is_question_text_ready(&question.question_text)
                    && matches!(
                        question.rubric.status,
                        RubricStatus::Suggested
                            | RubricStatus::Imported
                            | RubricStatus::Manual
                            | RubricStatus::Confirmed
                    )
                    && validate_rubric_state(&question.rubric, Some(&question.answer_type)).valid
                    && question.rubric.max_score.is_some()
            })
        })
}

fn get_stage_label(stage: &WorkflowStage) -> String {
    match stage {
        WorkflowStage::DocumentsMissing => "Belgeler Eksik",
        WorkflowStage::PdfPreviewMissing => "PDF Önizleme Eksik",
        WorkflowStage::PdfPreviewReady => "PDF Önizleme Hazır",
        WorkflowStage::PdfPreviewReadyQuestionTextMissing => "Soru Metni Eksik",
        WorkflowStage::ExamPackageBuildReady => "Sınav Paketi Hazır",
        WorkflowStage::ExamPackageBuildRunning => "Sınav Paketi Oluşturuluyor",
        WorkflowStage::ExamPackageReviewNeeded => "Sınav Paketi İnceleme Gerekiyor",
        WorkflowStage::ExamPackageIncomplete => "Sınav Paketi Eksik",
        WorkflowStage::ExamPackageReadyForQep => "Sınav Paketi QEP İçin Hazır",
        WorkflowStage::QuestionTextMissing => "Soru Metni Eksik",
        WorkflowStage::QuestionTextExtractionRunning => "Soru Metni Çıkarılıyor",
        WorkflowStage::QuestionTextSuggested => "Soru Metni Önerildi",
        WorkflowStage::QuestionTextConfirmed => "Soru Metni Onaylandı",
        WorkflowStage::RubricMissing => "Rubrik Eksik",
        WorkflowStage::RubricSuggested => "Rubrik Önerildi",
        WorkflowStage::RubricImportedNeedsReview => "Rubrik İnceleme Bekliyor",
        WorkflowStage::RubricInvalid => "Rubrik Geçersiz",
        WorkflowStage::RubricConfirmed => "Rubrik Onaylandı",
        WorkflowStage::StudentScansMissing => "Öğrenci Kağıtları Eksik",
        WorkflowStage::StudentScanPreviewMissing => "Öğrenci Kağıtları Önizleme Eksik",
        WorkflowStage::StudentGroupingMissing => "Öğrenci Gruplama Eksik",
        WorkflowStage::StudentGroupingReady => "Öğrenci Gruplama Hazır",
        WorkflowStage::CropMissing => "Kırpma Eksik",
        WorkflowStage::OcrReady => "OCR Hazır",
        WorkflowStage::OcrRunning => "OCR Çalışıyor",
        WorkflowStage::ReviewRequired => "İnceleme Gerekiyor",
        WorkflowStage::StudentAnswerOcrRunning => "Öğrenci Cevap OCR’ı Çalışıyor",
        WorkflowStage::StudentAnswerOcrReviewNeeded => "Öğrenci Cevap OCR’ı Kontrol Bekliyor",
        WorkflowStage::StudentAnswerOcrReadyForScoring => "Öğrenci Cevap OCR’ı Onaylandı",
        WorkflowStage::QepMissing => "QEP Eksik",
        WorkflowStage::QepReady => "QEP Hazır",
        WorkflowStage::QepFrozen => "QEP Donduruldu",
        WorkflowStage::ScoringReady => "Puanlama Hazır",
        WorkflowStage::ScoringRunning => "Puanlama Çalışıyor",
        WorkflowStage::ScoringDone => "Puanlama Tamamlandı",
        WorkflowStage::AnalysisReady => "Analiz Hazır",
    }
    .to_string()
}

pub fn evaluate_workflow_inner(
    project: &Project,
    model_status: &crate::domain::model::ModelStatus,
    question_text_job_active: bool,
    student_answer_ocr_job_active: bool,
) -> (
    WorkflowStage,
    Vec<BlockingReason>,
    Vec<WorkflowAction>,
    Option<String>,
) {
    let exam_package_frozen = project
        .exam_package_freeze
        .as_ref()
        .is_some_and(|freeze| freeze.freeze_status == ExamPackageFreezeStatus::Frozen);

    if !exam_package_frozen
        && matches!(
            project.workflow.current_stage,
            WorkflowStage::ExamPackageBuildReady
                | WorkflowStage::ExamPackageBuildRunning
                | WorkflowStage::ExamPackageReviewNeeded
                | WorkflowStage::ExamPackageIncomplete
                | WorkflowStage::ExamPackageReadyForQep
        )
    {
        return (
            project.workflow.current_stage.clone(),
            project.workflow.blocking_reasons.clone(),
            project.workflow.next_actions.clone(),
            project.workflow.summary.text.clone(),
        );
    }

    let mut blocking_reasons = Vec::new();
    let mut next_actions = Vec::new();

    let has_exam_source = project
        .documents
        .iter()
        .any(|d| d.role == DocumentRole::ExamSource);
    if !has_exam_source {
        blocking_reasons.push(BlockingReason::ExamSourceMissing);
        next_actions.push(WorkflowAction {
            code: "import_exam_source_pdf".to_string(),
            label: "Orijinal sınav PDF'i yükle".to_string(),
            enabled: true,
            disabled_reason: None,
            command: Some("import_exam_source_pdf".to_string()),
            requires: None,
        });
        return (
            WorkflowStage::DocumentsMissing,
            blocking_reasons,
            next_actions,
            Some("Orijinal sınav PDF'i bekleniyor.".to_string()),
        );
    }

    let exam_source_docs: Vec<_> = project
        .documents
        .iter()
        .filter(|document| document.role == DocumentRole::ExamSource)
        .collect();
    let all_exam_source_previews_ready = exam_source_docs.iter().all(|document| {
        matches!(
            document.preview.as_ref().map(|preview| &preview.status),
            Some(PdfPreviewStatus::Ready)
        ) || check_preview_cache_valid(&project.root_path, &document.id)
    });

    if !all_exam_source_previews_ready {
        blocking_reasons.push(BlockingReason::PdfPreviewMissing);
        next_actions.push(WorkflowAction {
            code: "start_pdf_preview_render".to_string(),
            label: "PDF sayfa önizlemelerini oluştur".to_string(),
            enabled: true,
            disabled_reason: None,
            command: Some("start_pdf_preview_render".to_string()),
            requires: None,
        });
        next_actions.push(WorkflowAction {
            code: "open_pdf_preview_page".to_string(),
            label: "PDF önizleme sayfasını aç".to_string(),
            enabled: true,
            disabled_reason: None,
            command: Some("open_pdf_preview_page".to_string()),
            requires: None,
        });
        return (
            WorkflowStage::PdfPreviewMissing,
            blocking_reasons,
            next_actions,
            Some("Sınav PDF'i var ama sayfa önizlemeleri henüz hazır değil.".to_string()),
        );
    }

    if question_text_job_active {
        blocking_reasons.push(BlockingReason::QuestionTextMissing);
        next_actions.push(WorkflowAction {
            code: "open_question_text_page".to_string(),
            label: "Önerileri İncele".to_string(),
            enabled: true,
            disabled_reason: None,
            command: Some("open_question_text_page".to_string()),
            requires: None,
        });
        return (
            WorkflowStage::QuestionTextExtractionRunning,
            blocking_reasons,
            next_actions,
            Some("Soru metni çıkarımı çalışıyor.".to_string()),
        );
    }

    let all_question_text_ready = project
        .questions
        .iter()
        .all(|q| is_question_text_ready(&q.question_text));
    let any_question_text_suggested = project
        .questions
        .iter()
        .any(|q| q.question_text.status == TextFieldStatus::Suggested);
    let any_question_text_missing = project.questions.iter().any(|q| {
        q.question_text.status == TextFieldStatus::Missing
            || q.question_text.status == TextFieldStatus::Failed
    });

    if project.questions.is_empty()
        || (any_question_text_missing && !any_question_text_suggested)
        || project
            .questions
            .iter()
            .all(|q| q.question_text.status == TextFieldStatus::Missing)
    {
        blocking_reasons.push(BlockingReason::QuestionTextMissing);

        let model_server_running = model_status.server_running && model_status.health_ok;

        if !model_server_running {
            blocking_reasons.push(BlockingReason::ModelServerNotRunning);
        }

        next_actions.push(WorkflowAction {
            code: "start_question_text_extraction".to_string(),
            label: "Soru metnini çıkar".to_string(),
            enabled: model_server_running,
            disabled_reason: if model_server_running {
                None
            } else {
                Some("Soru metni çıkarımı için Gemma model sunucusu çalışmalıdır.".to_string())
            },
            command: Some("start_question_text_extraction".to_string()),
            requires: None,
        });

        if !model_server_running {
            for action in &model_status.suggested_actions {
                next_actions.push(WorkflowAction {
                    code: action.code.clone(),
                    label: action.label.clone(),
                    enabled: true,
                    disabled_reason: None,
                    command: Some(action.code.clone()),
                    requires: None,
                });
            }
        }

        let summary = if !model_server_running {
            if model_status.start_requires_mode_change {
                "Gemma model sunucusu çalışmıyor.\nModeli uygulama dışından başlatabilir veya profili yönetilen moda alabilirsiniz.".to_string()
            } else if model_status.can_start_from_app {
                "Gemma model sunucusu çalışmıyor.\nModeli uygulama içinden başlatabilirsiniz."
                    .to_string()
            } else {
                "Gemma model sunucusu çalışmıyor.\nBu profil harici modda. Modeli dışarıdan başlatın veya Model Status ekranından yönetilen moda alın.".to_string()
            }
        } else {
            "PDF önizlemeleri hazır. Soru metni çıkarımı başlatılabilir.".to_string()
        };

        return (
            WorkflowStage::PdfPreviewReadyQuestionTextMissing,
            blocking_reasons,
            next_actions,
            Some(summary),
        );
    }

    if !all_question_text_ready && any_question_text_suggested {
        if any_question_text_missing {
            blocking_reasons.push(BlockingReason::QuestionTextMissing);
            blocking_reasons.push(BlockingReason::ReviewRequired);
        } else {
            blocking_reasons.push(BlockingReason::ReviewRequired);
        }

        next_actions.push(WorkflowAction {
            code: "open_question_text_page".to_string(),
            label: "Önerileri İncele".to_string(),
            enabled: true,
            disabled_reason: None,
            command: Some("open_question_text_page".to_string()),
            requires: None,
        });
        next_actions.push(WorkflowAction {
            code: "confirm_all_question_texts".to_string(),
            label: "Önerileri Onayla".to_string(),
            enabled: !any_question_text_missing,
            disabled_reason: if any_question_text_missing {
                Some("Eksik sorular manuel kontrol gerektiriyor.".to_string())
            } else {
                None
            },
            command: Some("confirm_all_question_texts".to_string()),
            requires: None,
        });
        return (
            WorkflowStage::QuestionTextSuggested,
            blocking_reasons,
            next_actions,
            Some("Soru metinleri onay bekliyor.".to_string()),
        );
    }

    let rubric_validations: Vec<_> = project
        .questions
        .iter()
        .map(|question| validate_rubric_state(&question.rubric, Some(&question.answer_type)))
        .collect();
    let any_rubric_missing = project
        .questions
        .iter()
        .any(|question| question.rubric.status == RubricStatus::Missing);
    let any_rubric_invalid =
        project
            .questions
            .iter()
            .zip(rubric_validations.iter())
            .any(|(question, validation)| {
                question.rubric.status == RubricStatus::Invalid
                    || question.rubric.status == RubricStatus::Legacy
                    || !validation.valid
            });
    let any_rubric_pending_review = project.questions.iter().any(|question| {
        matches!(
            question.rubric.status,
            RubricStatus::Suggested | RubricStatus::Imported | RubricStatus::Manual
        )
    });
    let all_rubrics_confirmed = project
        .questions
        .iter()
        .all(|question| is_rubric_confirmed(&question.rubric, Some(&question.answer_type)));
    let scoring_state = scoring_readiness(project);
    let scoring_complete = scoring_state.expected_records > 0
        && scoring_state.scoring_record_count == scoring_state.expected_records;

    let has_rubric_pdf = project.documents.iter().any(|doc| {
        doc.role == crate::domain::document::DocumentRole::Rubric
            || doc.role == crate::domain::document::DocumentRole::AnswerKey
    });

    if any_rubric_missing {
        blocking_reasons.push(BlockingReason::RubricMissing);

        if has_rubric_pdf {
            let model_server_running = model_status.server_running && model_status.health_ok;
            if !model_server_running {
                blocking_reasons.push(BlockingReason::ModelServerNotRunning);
            }

            next_actions.push(WorkflowAction {
                code: "start_rubric_pdf_import".to_string(),
                label: "Rubrik PDF'inden içe aktar".to_string(),
                enabled: model_server_running,
                disabled_reason: if model_server_running {
                    None
                } else {
                    Some(
                        "Rubrik PDF’inden bilgi çıkarmak için Gemma model sunucusu çalışmalıdır."
                            .to_string(),
                    )
                },
                command: Some("start_rubric_pdf_import".to_string()),
                requires: None,
            });

            if !model_server_running {
                for action in &model_status.suggested_actions {
                    next_actions.push(WorkflowAction {
                        code: action.code.clone(),
                        label: action.label.clone(),
                        enabled: true,
                        disabled_reason: None,
                        command: Some(action.code.clone()),
                        requires: None,
                    });
                }
            }

            let summary = if !model_server_running {
                if model_status.start_requires_mode_change {
                    "Gemma model sunucusu çalışmıyor.\nModeli uygulama dışından başlatabilir veya profili yönetilen moda alabilirsiniz.".to_string()
                } else if model_status.can_start_from_app {
                    "Gemma model sunucusu çalışmıyor.\nModeli uygulama içinden başlatabilirsiniz."
                        .to_string()
                } else {
                    "Gemma model sunucusu çalışmıyor.\nBu profil harici modda. Modeli dışarıdan başlatın veya Model Status ekranından yönetilen moda alın.".to_string()
                }
            } else {
                "Rubrik PDF'i yüklü. Henüz içe aktarılmadı.".to_string()
            };

            return (
                WorkflowStage::RubricMissing,
                blocking_reasons,
                next_actions,
                Some(summary),
            );
        } else {
            next_actions.push(WorkflowAction {
                code: "open_rubric_preparation_page".to_string(),
                label: "Rubrik hazırlığını aç".to_string(),
                enabled: true,
                disabled_reason: None,
                command: Some("open_rubric_preparation_page".to_string()),
                requires: None,
            });
            return (
                WorkflowStage::RubricMissing,
                blocking_reasons,
                next_actions,
                Some("Cevap anahtarı / rubrik bekleniyor.".to_string()),
            );
        }
    }

    if any_rubric_invalid {
        blocking_reasons.push(BlockingReason::RubricInvalid);
        if rubric_validations.iter().any(|validation| {
            validation
                .issues
                .iter()
                .any(|issue| issue.code == "RUBRIC_PLACEHOLDER_DETECTED")
        }) {
            blocking_reasons.push(BlockingReason::PlaceholderDataDetected);
        }
        next_actions.push(WorkflowAction {
            code: "validate_rubrics".to_string(),
            label: "Rubrikleri doğrula".to_string(),
            enabled: true,
            disabled_reason: None,
            command: Some("validate_rubrics".to_string()),
            requires: None,
        });
        next_actions.push(WorkflowAction {
            code: "open_rubric_preparation_page".to_string(),
            label: "Rubrik hazırlığını aç".to_string(),
            enabled: true,
            disabled_reason: None,
            command: Some("open_rubric_preparation_page".to_string()),
            requires: None,
        });
        return (
            WorkflowStage::RubricInvalid,
            blocking_reasons,
            next_actions,
            Some("Bazı rubrikler geçersiz. Düzenleme gerekiyor.".to_string()),
        );
    }

    if any_rubric_pending_review && !all_rubrics_confirmed {
        blocking_reasons.push(BlockingReason::ReviewRequired);
        next_actions.push(WorkflowAction {
            code: "open_rubric_preparation_page".to_string(),
            label: "Rubrik hazırlığını aç".to_string(),
            enabled: true,
            disabled_reason: None,
            command: Some("open_rubric_preparation_page".to_string()),
            requires: None,
        });
        next_actions.push(WorkflowAction {
            code: "confirm_all_rubrics".to_string(),
            label: "Sınav Paketini Onayla".to_string(),
            enabled: true,
            disabled_reason: None,
            command: Some("confirm_all_rubrics".to_string()),
            requires: None,
        });
        return (
            WorkflowStage::RubricImportedNeedsReview,
            blocking_reasons,
            next_actions,
            Some("Rubrikler öğretmen incelemesi bekliyor.".to_string()),
        );
    }

    if all_rubrics_confirmed && !exam_package_frozen {
        blocking_reasons.push(BlockingReason::QepNotFrozen);
        next_actions.push(WorkflowAction {
            code: "confirm_all_rubrics".to_string(),
            label: "Sınav paketini dondur".to_string(),
            enabled: true,
            disabled_reason: None,
            command: Some("confirm_all_rubrics".to_string()),
            requires: None,
        });
        next_actions.push(WorkflowAction {
            code: "open_exam_package_review_page".to_string(),
            label: "Paket ve dondurma bölümünü aç".to_string(),
            enabled: true,
            disabled_reason: None,
            command: Some("open_exam_package_review_page".to_string()),
            requires: None,
        });
        return (
            WorkflowStage::QepReady,
            blocking_reasons,
            next_actions,
            Some("Sınav paketi dondurulmaya hazır.".to_string()),
        );
    }

    if all_rubrics_confirmed && !project.questions.is_empty() {
        let student_scan_docs = project
            .documents
            .iter()
            .filter(|document| document.role == DocumentRole::StudentScan)
            .collect::<Vec<_>>();
        if student_scan_docs.is_empty() {
            blocking_reasons.push(BlockingReason::StudentScanNotFound);
            next_actions.push(WorkflowAction {
                code: "import_student_scan_pdf".to_string(),
                label: "Öğrenci cevap PDF’i yükle".to_string(),
                enabled: true,
                disabled_reason: None,
                command: Some("import_student_scan_pdf".to_string()),
                requires: None,
            });
            next_actions.push(WorkflowAction {
                code: "open_student_scans_page".to_string(),
                label: "Öğrenci PDF’leri sayfasını aç".to_string(),
                enabled: true,
                disabled_reason: None,
                command: Some("open_student_scans_page".to_string()),
                requires: None,
            });
            return (
                WorkflowStage::StudentScansMissing,
                blocking_reasons,
                next_actions,
                Some("Öğrenci cevap PDF’i bekleniyor.".to_string()),
            );
        }

        let preview_missing = student_scan_docs.iter().any(|document| {
            !matches!(
                document.preview.as_ref().map(|preview| &preview.status),
                Some(PdfPreviewStatus::Ready)
            ) && !check_preview_cache_valid(&project.root_path, &document.id)
        });
        if preview_missing {
            blocking_reasons.push(BlockingReason::StudentScanPreviewNotReady);
            next_actions.push(WorkflowAction {
                code: "start_student_scan_preview_render".to_string(),
                label: "Öğrenci PDF önizlemelerini oluştur".to_string(),
                enabled: true,
                disabled_reason: None,
                command: Some("start_student_scan_preview_render".to_string()),
                requires: None,
            });
            next_actions.push(WorkflowAction {
                code: "open_student_scans_page".to_string(),
                label: "Öğrenci PDF’leri sayfasını aç".to_string(),
                enabled: true,
                disabled_reason: None,
                command: Some("open_student_scans_page".to_string()),
                requires: None,
            });
            return (
                WorkflowStage::StudentScanPreviewMissing,
                blocking_reasons,
                next_actions,
                Some("Öğrenci cevap PDF önizlemeleri hazır değil.".to_string()),
            );
        }

        let has_submissions = !project.student_submissions.is_empty();
        let all_batches_have_submissions = project.student_scan_batches.is_empty()
            || project.student_scan_batches.iter().all(|batch| {
                project
                    .student_submissions
                    .iter()
                    .any(|submission| submission.scan_batch_id.as_deref() == Some(&batch.id))
            });
        let expected_groups = if project.student_scan_batches.is_empty() {
            let pages_per_student = project.student_pages_per_student.unwrap_or(0);
            let total_pages = student_scan_docs
                .first()
                .map_or(0, |document| document.page_count);
            if pages_per_student > 0 {
                total_pages.div_ceil(pages_per_student)
            } else {
                project.student_submissions.len() as u32
            }
        } else {
            project
                .student_scan_batches
                .iter()
                .map(|batch| {
                    let pages = project
                        .documents
                        .iter()
                        .find(|document| document.id == batch.document_id)
                        .map_or(0, |document| document.page_count);
                    batch
                        .pages_per_student
                        .filter(|value| *value > 0)
                        .map_or(0, |value| pages.div_ceil(value))
                })
                .sum()
        };
        let any_submission_incomplete = has_submissions
            && project
                .student_submissions
                .iter()
                .any(|submission| submission.page_numbers.is_empty());
        if !has_submissions || !all_batches_have_submissions {
            blocking_reasons.push(BlockingReason::StudentGroupingNotReady);
            next_actions.push(WorkflowAction {
                code: "create_student_page_groups".to_string(),
                label: "Öğrencileri sayfalara göre grupla".to_string(),
                enabled: true,
                disabled_reason: None,
                command: Some("create_student_page_groups".to_string()),
                requires: None,
            });
            next_actions.push(WorkflowAction {
                code: "open_student_grouping_page".to_string(),
                label: "Öğrenci gruplama sayfasını aç".to_string(),
                enabled: true,
                disabled_reason: None,
                command: Some("open_student_grouping_page".to_string()),
                requires: None,
            });
            return (
                WorkflowStage::StudentGroupingMissing,
                blocking_reasons,
                next_actions,
                Some("Öğrenci sayfa gruplaması bekleniyor.".to_string()),
            );
        }

        if any_submission_incomplete {
            blocking_reasons.push(BlockingReason::StudentGroupingNotReady);
            next_actions.push(WorkflowAction {
                code: "open_student_grouping_page".to_string(),
                label: "Öğrenci gruplama sayfasını aç".to_string(),
                enabled: true,
                disabled_reason: None,
                command: Some("open_student_grouping_page".to_string()),
                requires: None,
            });
            next_actions.push(WorkflowAction {
                code: "mark_student_grouping_complete".to_string(),
                label: "Gruplamayı tamamla".to_string(),
                enabled: false,
                disabled_reason: Some("Her grupta en az bir sayfa olmalı.".to_string()),
                command: Some("mark_student_grouping_complete".to_string()),
                requires: None,
            });
            return (
                WorkflowStage::StudentGroupingMissing,
                blocking_reasons,
                next_actions,
                Some("Öğrenci grupları eksik.".to_string()),
            );
        }

        if !student_grouping_is_complete(project) && !scoring_state.ready {
            next_actions.push(WorkflowAction {
                code: "mark_student_grouping_complete".to_string(),
                label: "Gruplamayı tamamla".to_string(),
                enabled: true,
                disabled_reason: None,
                command: Some("mark_student_grouping_complete".to_string()),
                requires: None,
            });
            next_actions.push(WorkflowAction {
                code: "open_student_grouping_page".to_string(),
                label: "Öğrenci gruplama sayfasını aç".to_string(),
                enabled: true,
                disabled_reason: None,
                command: Some("open_student_grouping_page".to_string()),
                requires: None,
            });
            return (
                WorkflowStage::StudentGroupingReady,
                blocking_reasons,
                next_actions,
                Some(if expected_groups > 0 {
                    format!(
                        "Öğrenci gruplaması tamamlanabilir: {}/{} grup hazır.",
                        project.student_submissions.len(),
                        expected_groups
                    )
                } else {
                    "Öğrenci gruplaması tamamlanabilir.".to_string()
                }),
            );
        }

        let ocr_total = project.student_submissions.len() * project.questions.len();
        let ocr_records = project.student_answer_ocr_records.len();
        let ocr_all_reviewed = ocr_total > 0
            && ocr_records == ocr_total
            && project.student_answer_ocr_records.iter().all(|record| {
                matches!(
                    record.status,
                    crate::domain::student::StudentAnswerOcrStatus::TeacherApproved
                )
            });

        if student_answer_ocr_job_active {
            return (
                WorkflowStage::StudentAnswerOcrRunning,
                blocking_reasons,
                vec![WorkflowAction {
                    code: "open_student_answer_ocr_page".to_string(),
                    label: "OCR sonuçlarını aç".to_string(),
                    enabled: true,
                    disabled_reason: None,
                    command: Some("open_student_answer_ocr_page".to_string()),
                    requires: None,
                }],
                Some("Öğrenci cevap OCR’ı çalışıyor.".to_string()),
            );
        }

        if ocr_records == 0 {
            next_actions.push(WorkflowAction {
                code: "start_student_answer_ocr".to_string(),
                label: "Öğrenci Cevap OCR’ını Başlat".to_string(),
                enabled: true,
                disabled_reason: None,
                command: Some("start_student_answer_ocr".to_string()),
                requires: None,
            });
            next_actions.push(WorkflowAction {
                code: "open_student_answer_ocr_page".to_string(),
                label: "OCR sonuçlarını aç".to_string(),
                enabled: true,
                disabled_reason: None,
                command: Some("open_student_answer_ocr_page".to_string()),
                requires: None,
            });
            return (
                WorkflowStage::OcrReady,
                blocking_reasons,
                next_actions,
                Some("Öğrenci cevap OCR’ı hazır.".to_string()),
            );
        }

        if ocr_all_reviewed {
            let identity_invalid = project.students.iter().any(student_identity_is_missing);
            if identity_invalid {
                blocking_reasons.push(BlockingReason::StudentIdentityInvalid);
                next_actions.push(WorkflowAction {
                    code: "open_student_identity_page".to_string(),
                    label: "Öğrenci kimliğini doğrula".to_string(),
                    enabled: true,
                    disabled_reason: None,
                    command: Some("open_student_identity_page".to_string()),
                    requires: None,
                });
                return (
                    WorkflowStage::StudentAnswerOcrReadyForScoring,
                    blocking_reasons,
                    next_actions,
                    Some("Öğrenci kimliği doğrulaması bekleniyor.".to_string()),
                );
            }
            if matches!(
                project.workflow.current_stage,
                WorkflowStage::ScoringRunning
            ) && !scoring_complete
            {
                next_actions.push(WorkflowAction {
                    code: "open_scoring_page".to_string(),
                    label: "Notlandırma sayfasını aç".to_string(),
                    enabled: true,
                    disabled_reason: None,
                    command: Some("open_scoring_page".to_string()),
                    requires: None,
                });
                return (
                    WorkflowStage::ScoringRunning,
                    blocking_reasons,
                    next_actions,
                    Some("Notlandırma çalışıyor.".to_string()),
                );
            }
            if scoring_state.scoring_record_count > 0
                && scoring_state.needs_review_record_count == 0
                && scoring_state.stale_record_count == 0
            {
                next_actions.push(WorkflowAction {
                    code: "open_scoring_page".to_string(),
                    label: "Notlandırma sayfasını aç".to_string(),
                    enabled: true,
                    disabled_reason: None,
                    command: Some("open_scoring_page".to_string()),
                    requires: None,
                });
                return (
                    WorkflowStage::ScoringDone,
                    blocking_reasons,
                    next_actions,
                    Some("Notlandırma tamamlandı.".to_string()),
                );
            }
            next_actions.push(WorkflowAction {
                code: "open_scoring_page".to_string(),
                label: "Notlandırma sayfasını aç".to_string(),
                enabled: true,
                disabled_reason: None,
                command: Some("open_scoring_page".to_string()),
                requires: None,
            });
            next_actions.push(WorkflowAction {
                code: "start_scoring_job".to_string(),
                label: "Notlandırmayı başlat".to_string(),
                enabled: true,
                disabled_reason: None,
                command: Some("start_scoring_job".to_string()),
                requires: None,
            });
            return (
                WorkflowStage::ScoringReady,
                blocking_reasons,
                next_actions,
                Some("Notlandırma hazır.".to_string()),
            );
        }

        next_actions.push(WorkflowAction {
            code: "open_student_answer_ocr_page".to_string(),
            label: "OCR sonuçlarını kontrol et".to_string(),
            enabled: true,
            disabled_reason: None,
            command: Some("open_student_answer_ocr_page".to_string()),
            requires: None,
        });
        next_actions.push(WorkflowAction {
            code: "rerun_student_answer_ocr".to_string(),
            label: "OCR’ı Yeniden Çalıştır".to_string(),
            enabled: true,
            disabled_reason: None,
            command: Some("start_student_answer_ocr".to_string()),
            requires: None,
        });
        return (
            WorkflowStage::StudentAnswerOcrReviewNeeded,
            blocking_reasons,
            next_actions,
            Some("Öğrenci cevap OCR sonuçları öğretmen kontrolü bekliyor.".to_string()),
        );
    }

    (
        WorkflowStage::RubricMissing,
        blocking_reasons,
        next_actions,
        Some("Rubrik hazırlığı bekleniyor.".to_string()),
    )
}

fn student_grouping_is_complete(project: &Project) -> bool {
    if project.student_scan_batches.is_empty() {
        return project.student_grouping_complete_at.is_some()
            && !project.student_submissions.is_empty();
    }

    project.student_scan_batches.iter().all(|batch| {
        batch.grouping_completed_at.is_some()
            && project.student_submissions.iter().any(|submission| {
                submission.scan_batch_id.as_deref() == Some(batch.id.as_str())
                    && !submission.page_numbers.is_empty()
            })
    })
}

fn check_preview_cache_valid(root_path: &str, document_id: &str) -> bool {
    let path = std::path::Path::new(root_path)
        .join("cache")
        .join("page_previews")
        .join(document_id)
        .join("page_previews.json");

    if !path.exists() || !path.is_file() {
        return false;
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let index: serde_json::Value = match serde_json::from_str(&content) {
        Ok(val) => val,
        Err(_) => return false,
    };

    let pages = match index.get("pages").and_then(|p| p.as_array()) {
        Some(p) => p,
        None => return false,
    };

    if pages.is_empty() {
        return false;
    }

    for page in pages {
        let image_path_str = match page.get("imagePath").and_then(|p| p.as_str()) {
            Some(s) => s,
            None => return false,
        };
        let img_path = std::path::Path::new(image_path_str);
        if !img_path.exists() || !img_path.is_file() {
            return false;
        }
    }

    true
}

fn student_scan_preview_progress(
    root_path: &str,
    student_scan_docs: &[&crate::domain::document::Document],
) -> Option<(u32, u32)> {
    if student_scan_docs.is_empty() {
        return None;
    }
    let mut current = 0;
    let mut total = 0;
    for document in student_scan_docs {
        total += document.page_count;
        let metadata_path = std::path::Path::new(root_path)
            .join("cache")
            .join("page_previews")
            .join(&document.id)
            .join("page_previews.json");
        let Some(index) = std::fs::read_to_string(&metadata_path)
            .ok()
            .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
        else {
            continue;
        };
        current += index
            .get("pages")
            .and_then(|value| value.as_array())
            .map_or(0, |pages| pages.len() as u32);
        if document.page_count == 0 {
            total += index
                .get("pageCount")
                .and_then(|value| value.as_u64())
                .map_or(0, |value| value as u32);
        }
    }
    Some((current, total))
}

pub fn has_active_question_text_job(jobs: &[JobSnapshot]) -> bool {
    jobs.iter().any(|job| {
        job.kind == crate::domain::job::JobKind::QuestionTextExtraction
            && matches!(job.status, JobStatus::Queued | JobStatus::Running)
    })
}

pub fn has_active_student_answer_ocr_job(jobs: &[JobSnapshot]) -> bool {
    jobs.iter().any(|job| {
        job.kind == crate::domain::job::JobKind::StudentAnswerOcr
            && matches!(job.status, JobStatus::Queued | JobStatus::Running)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::document::Document;
    use crate::domain::project::Project;
    use crate::domain::question::{AnswerType, Question, TextFieldSource, TextFieldState};
    use crate::domain::rubric::{RubricState, RubricStatus};
    use crate::domain::student::{
        Student, StudentAnswerOcrRecord, StudentAnswerOcrStatus, StudentAnswerSlot,
        StudentAnswerSlotStatus, StudentSubmission, StudentSubmissionStatus,
    };

    fn empty_project() -> Project {
        Project {
            expected_question_count: None,
            exam_package_freeze: None,
            id: "test".into(),
            name: "test".into(),
            created_at: "".into(),
            updated_at: "".into(),
            root_path: "".into(),
            sections: vec![],
            students: vec![],
            school_classes: vec![],
            student_scan_batches: vec![],
            student_submissions: vec![],
            student_answer_ocr_records: vec![],
            student_answer_crop_template: Default::default(),
            student_identity_crop_template: None,
            student_scan_document_id: None,
            student_grouping_mode: None,
            student_pages_per_student: None,
            student_grouping_complete_at: None,
            documents: vec![],
            questions: vec![],
            scoring_records: vec![],
            speaking_exams: vec![],
            latest_scoring_run_id: None,
            workflow: WorkflowSnapshot {
                current_stage: WorkflowStage::DocumentsMissing,
                blocking_reasons: vec![],
                next_actions: vec![],
                current_stage_label: "Test".to_string(),
                summary: crate::domain::workflow::WorkflowSummary::default(),
            },
        }
    }

    #[test]
    fn test_documents_missing() {
        let p = empty_project();
        let snap = evaluate_workflow(&p);
        assert_eq!(snap.current_stage, WorkflowStage::DocumentsMissing);
    }

    #[test]
    fn test_pdf_preview_missing() {
        let mut p = empty_project();
        p.documents.push(Document {
            id: "1".into(),
            role: DocumentRole::ExamSource,
            file_name: "test.pdf".into(),
            stored_path: "".into(),
            page_count: 1,
            added_at: "".into(),
            checksum: None,
            preview: None,
        });

        let snap = evaluate_workflow(&p);
        assert_eq!(snap.current_stage, WorkflowStage::PdfPreviewMissing);
    }

    #[test]
    fn test_question_text_suggested() {
        let mut p = empty_project();
        p.documents.push(Document {
            id: "1".into(),
            role: DocumentRole::ExamSource,
            file_name: "test.pdf".into(),
            stored_path: "".into(),
            page_count: 1,
            added_at: "".into(),
            checksum: None,
            preview: Some(crate::domain::document::PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: None,
                page_count: Some(1),
                job_id: None,
                error_message: None,
            }),
        });

        p.questions.push(Question {
            id: "q1".into(),
            number: 1,
            max_score: 10.0,
            answer_type: AnswerType::GeneralText,
            question_text: TextFieldState {
                value: "suggested text".into(),
                source: TextFieldSource::ExamPdf,
                status: TextFieldStatus::Suggested,
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
        });

        let snap = evaluate_workflow(&p);
        assert_eq!(snap.current_stage, WorkflowStage::QuestionTextSuggested);
    }

    #[test]
    fn test_preview_ready_question_text_missing() {
        let mut p = empty_project();
        p.documents.push(Document {
            id: "1".into(),
            role: DocumentRole::ExamSource,
            file_name: "test.pdf".into(),
            stored_path: "".into(),
            page_count: 1,
            added_at: "".into(),
            checksum: None,
            preview: Some(crate::domain::document::PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: None,
                page_count: Some(1),
                job_id: None,
                error_message: None,
            }),
        });

        let snap = evaluate_workflow(&p);
        assert_eq!(
            snap.current_stage,
            WorkflowStage::PdfPreviewReadyQuestionTextMissing
        );
        assert!(snap
            .next_actions
            .iter()
            .any(|action| action.code == "start_question_text_extraction"));
    }

    fn question_text_and_rubric_ready_project() -> Project {
        let mut project = empty_project();
        project.documents.push(Document {
            id: "exam".into(),
            role: DocumentRole::ExamSource,
            file_name: "exam.pdf".into(),
            stored_path: "exam.pdf".into(),
            page_count: 1,
            added_at: "".into(),
            checksum: None,
            preview: Some(crate::domain::document::PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: None,
                page_count: Some(1),
                job_id: None,
                error_message: None,
            }),
        });
        project.questions.push(Question {
            id: "q1".into(),
            number: 1,
            max_score: 0.0,
            answer_type: AnswerType::GeneralText,
            question_text: TextFieldState {
                value: "Question".into(),
                source: TextFieldSource::Manual,
                status: TextFieldStatus::Confirmed,
                confidence: None,
                warnings: vec![],
                updated_at: None,
            },
            rubric: RubricState {
                status: RubricStatus::Confirmed,
                source: None,
                max_score: Some(10.0),
                expected_answer: Some("Answer".into()),
                criteria: vec![crate::domain::rubric::RubricCriterion {
                    id: "c1".into(),
                    label: "Kriter".into(),
                    description: "Açıklama".into(),
                    points: 10.0,
                }],
                partial_credit_hints: vec![],
                zero_score_conditions: vec![],
                common_mistakes: vec![],
                warnings: vec![],
                updated_at: None,
            },
            crop_template: None,
        });
        project
    }

    fn frozen_ready_project() -> Project {
        let mut project = question_text_and_rubric_ready_project();
        project.exam_package_freeze = Some(crate::domain::project::ExamPackageFreeze {
            exam_package_version: 1,
            freeze_status: ExamPackageFreezeStatus::Frozen,
            frozen_at: "now".into(),
            frozen_by: Some("teacher".into()),
            source_hash: "source".into(),
            rubric_hash: "rubric".into(),
            question_text_hash: "question".into(),
            invalidated_at: None,
            invalidation_reason: None,
        });
        project.workflow.current_stage = WorkflowStage::ExamPackageReviewNeeded;
        project.workflow.current_stage_label = "Sınav Paketi İnceleme Gerekiyor".to_string();
        project.workflow.blocking_reasons = vec![BlockingReason::ReviewRequired];
        project.workflow.next_actions = vec![];
        project.workflow.summary = crate::domain::workflow::WorkflowSummary::default();
        project
    }

    fn student_answer_ocr_ready_project() -> Project {
        let mut project = frozen_ready_project();
        project.students.push(Student {
            id: "student-1".into(),
            display_name: Some("Öğrenci 1".into()),
            number: Some("1".into()),
            class_name: None,
            warnings: vec![],
            identity_ocr: None,
        });
        project.documents.push(Document {
            id: "student_scan".into(),
            role: DocumentRole::StudentScan,
            file_name: "students.pdf".into(),
            stored_path: "students.pdf".into(),
            page_count: 1,
            added_at: "".into(),
            checksum: None,
            preview: Some(crate::domain::document::PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: None,
                page_count: Some(1),
                job_id: None,
                error_message: None,
            }),
        });
        project.student_scan_document_id = Some("student_scan".into());
        project.student_grouping_complete_at = Some("now".into());
        project.student_pages_per_student = Some(1);
        project.student_submissions.push(StudentSubmission {
            id: "submission-1".into(),
            student_id: "student-1".into(),
            document_id: "student_scan".into(),
            class_id: None,
            scan_batch_id: None,
            class_membership_source: None,
            page_numbers: vec![1],
            status: StudentSubmissionStatus::OcrConfirmed,
            answer_slots: vec![],
            warnings: vec![],
            updated_at: Some("now".into()),
        });
        let now = chrono::Utc::now();
        project
            .student_answer_ocr_records
            .push(StudentAnswerOcrRecord {
                id: "ocr-1".into(),
                submission_id: "submission-1".into(),
                question_id: "q1".into(),
                question_number: 1,
                source_page_numbers: vec![1],
                source_image_refs: vec![],
                crop_refs: vec![],
                full_page_preview_refs: vec![],
                answer_text: "Cevap".into(),
                structured_answer: None,
                confidence: Some(0.92),
                status: StudentAnswerOcrStatus::TeacherApproved,
                needs_review: false,
                review_reasons: vec![],
                warnings: vec![],
                model_name: Some("gemma".into()),
                prompt_version: "student_answer_ocr_v1".into(),
                created_at: now,
                updated_at: now,
                teacher_corrected_text: None,
                teacher_reviewed_at: Some(now),
                parse_diagnostics: None,
                render_diagnostics: None,
                ..Default::default()
            });
        project
    }

    #[test]
    fn test_confirmed_rubrics_wait_for_explicit_package_freeze() {
        let p = question_text_and_rubric_ready_project();
        let snap = evaluate_workflow(&p);
        assert_eq!(snap.current_stage, WorkflowStage::QepReady);
        assert!(snap
            .next_actions
            .iter()
            .any(|action| action.code == "confirm_all_rubrics"));
    }

    #[test]
    fn test_student_scans_missing_after_package_is_frozen() {
        let p = frozen_ready_project();
        let snap = evaluate_workflow(&p);
        assert_eq!(snap.current_stage, WorkflowStage::StudentScansMissing);
        assert!(snap
            .next_actions
            .iter()
            .any(|action| action.code == "import_student_scan_pdf"));
    }

    #[test]
    fn freeze_readiness_rejects_suggested_question_text() {
        let mut project = question_text_and_rubric_ready_project();
        project.questions[0].question_text.status = TextFieldStatus::Suggested;

        let snapshot = evaluate_workflow(&project);

        assert!(!snapshot.summary.readiness.exam_package_freeze);
    }

    #[test]
    fn freeze_readiness_rejects_invalid_rubric_content() {
        let mut project = question_text_and_rubric_ready_project();
        project.questions[0].rubric.status = RubricStatus::Suggested;
        project.questions[0].rubric.max_score = None;

        let snapshot = evaluate_workflow(&project);

        assert!(!snapshot.summary.readiness.exam_package_freeze);
    }

    #[test]
    fn test_frozen_exam_package_ignores_stale_review_stage() {
        let p = frozen_ready_project();
        let snap = evaluate_workflow(&p);
        assert_eq!(snap.current_stage, WorkflowStage::StudentScansMissing);
        assert_ne!(snap.current_stage, WorkflowStage::ExamPackageReviewNeeded);
    }

    #[test]
    fn test_frozen_exam_package_advances_through_student_intake() {
        let mut with_preview_missing = frozen_ready_project();
        with_preview_missing.documents.push(Document {
            id: "scan".into(),
            role: DocumentRole::StudentScan,
            file_name: "scan.pdf".into(),
            stored_path: "scan.pdf".into(),
            page_count: 2,
            added_at: "".into(),
            checksum: None,
            preview: Some(crate::domain::document::PdfPreviewState {
                status: PdfPreviewStatus::Missing,
                rendered_at: None,
                page_count: None,
                job_id: None,
                error_message: None,
            }),
        });
        with_preview_missing.student_scan_document_id = Some("scan".into());
        assert_eq!(
            evaluate_workflow(&with_preview_missing).current_stage,
            WorkflowStage::StudentScanPreviewMissing
        );

        let mut with_grouping_missing = frozen_ready_project();
        with_grouping_missing.documents.push(Document {
            id: "scan".into(),
            role: DocumentRole::StudentScan,
            file_name: "scan.pdf".into(),
            stored_path: "scan.pdf".into(),
            page_count: 2,
            added_at: "".into(),
            checksum: None,
            preview: Some(crate::domain::document::PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: None,
                page_count: Some(2),
                job_id: None,
                error_message: None,
            }),
        });
        with_grouping_missing.student_scan_document_id = Some("scan".into());
        assert_eq!(
            evaluate_workflow(&with_grouping_missing).current_stage,
            WorkflowStage::StudentGroupingMissing
        );

        let mut ready_for_ocr = frozen_ready_project();
        ready_for_ocr.documents.push(Document {
            id: "scan".into(),
            role: DocumentRole::StudentScan,
            file_name: "scan.pdf".into(),
            stored_path: "scan.pdf".into(),
            page_count: 2,
            added_at: "".into(),
            checksum: None,
            preview: Some(crate::domain::document::PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: None,
                page_count: Some(2),
                job_id: None,
                error_message: None,
            }),
        });
        ready_for_ocr.student_scan_document_id = Some("scan".into());
        ready_for_ocr.students.push(Student {
            id: "student-1".into(),
            display_name: Some("Ali Veli".into()),
            number: Some("12".into()),
            class_name: None,
            warnings: vec![],
            identity_ocr: None,
        });
        ready_for_ocr.student_submissions.push(StudentSubmission {
            id: "submission-1".into(),
            student_id: "student-1".into(),
            document_id: "scan".into(),
            class_id: None,
            scan_batch_id: None,
            class_membership_source: None,
            page_numbers: vec![1, 2],
            status: StudentSubmissionStatus::ReadyForOcr,
            answer_slots: vec![StudentAnswerSlot {
                question_id: "q1".into(),
                question_number: 1,
                status: StudentAnswerSlotStatus::Empty,
                text: None,
                confidence: None,
                warnings: vec![],
            }],
            warnings: vec![],
            updated_at: None,
        });
        ready_for_ocr.student_grouping_complete_at = Some("now".into());
        assert_eq!(
            evaluate_workflow(&ready_for_ocr).current_stage,
            WorkflowStage::OcrReady
        );
    }

    #[test]
    fn test_student_scan_preview_missing_after_import() {
        let mut p = frozen_ready_project();
        p.documents.push(Document {
            id: "scan".into(),
            role: DocumentRole::StudentScan,
            file_name: "scan.pdf".into(),
            stored_path: "scan.pdf".into(),
            page_count: 2,
            added_at: "".into(),
            checksum: None,
            preview: Some(crate::domain::document::PdfPreviewState {
                status: PdfPreviewStatus::Missing,
                rendered_at: None,
                page_count: None,
                job_id: None,
                error_message: None,
            }),
        });
        p.student_scan_document_id = Some("scan".into());
        let snap = evaluate_workflow(&p);
        assert_eq!(snap.current_stage, WorkflowStage::StudentScanPreviewMissing);
        assert!(snap
            .next_actions
            .iter()
            .any(|action| action.code == "start_student_scan_preview_render"));
    }

    #[test]
    fn test_student_grouping_missing_when_preview_ready_but_no_submissions() {
        let mut p = frozen_ready_project();
        p.documents.push(Document {
            id: "scan".into(),
            role: DocumentRole::StudentScan,
            file_name: "scan.pdf".into(),
            stored_path: "scan.pdf".into(),
            page_count: 2,
            added_at: "".into(),
            checksum: None,
            preview: Some(crate::domain::document::PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: None,
                page_count: Some(2),
                job_id: None,
                error_message: None,
            }),
        });
        p.student_scan_document_id = Some("scan".into());
        let snap = evaluate_workflow(&p);
        assert_eq!(snap.current_stage, WorkflowStage::StudentGroupingMissing);
    }

    #[test]
    fn test_ocr_ready_after_grouping_complete() {
        let mut p = frozen_ready_project();
        p.documents.push(Document {
            id: "scan".into(),
            role: DocumentRole::StudentScan,
            file_name: "scan.pdf".into(),
            stored_path: "scan.pdf".into(),
            page_count: 2,
            added_at: "".into(),
            checksum: None,
            preview: Some(crate::domain::document::PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: None,
                page_count: Some(2),
                job_id: None,
                error_message: None,
            }),
        });
        p.student_scan_document_id = Some("scan".into());
        let student = Student {
            id: "student-1".into(),
            display_name: Some("Ali Veli".into()),
            number: Some("12".into()),
            class_name: None,
            warnings: vec![],
            identity_ocr: None,
        };
        p.students.push(student.clone());
        p.student_submissions.push(StudentSubmission {
            id: "submission-1".into(),
            student_id: student.id.clone(),
            document_id: "scan".into(),
            class_id: None,
            scan_batch_id: None,
            class_membership_source: None,
            page_numbers: vec![1, 2],
            status: StudentSubmissionStatus::ReadyForOcr,
            answer_slots: vec![StudentAnswerSlot {
                question_id: "q1".into(),
                question_number: 1,
                status: StudentAnswerSlotStatus::Empty,
                text: None,
                confidence: None,
                warnings: vec![],
            }],
            warnings: vec![],
            updated_at: None,
        });
        p.student_grouping_complete_at = Some("now".into());
        let snap = evaluate_workflow(&p);
        assert_eq!(snap.current_stage, WorkflowStage::OcrReady);
        assert!(snap
            .next_actions
            .iter()
            .any(|action| action.code == "start_student_answer_ocr"));
    }

    #[test]
    fn test_ocr_ready_without_student_identity() {
        let mut p = frozen_ready_project();
        p.documents.push(Document {
            id: "exam".into(),
            role: DocumentRole::ExamSource,
            file_name: "exam.pdf".into(),
            stored_path: "exam.pdf".into(),
            page_count: 2,
            added_at: "".into(),
            checksum: None,
            preview: Some(crate::domain::document::PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: None,
                page_count: Some(2),
                job_id: None,
                error_message: None,
            }),
        });
        p.documents.push(Document {
            id: "scan".into(),
            role: DocumentRole::StudentScan,
            file_name: "scan.pdf".into(),
            stored_path: "scan.pdf".into(),
            page_count: 2,
            added_at: "".into(),
            checksum: None,
            preview: Some(crate::domain::document::PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: None,
                page_count: Some(2),
                job_id: None,
                error_message: None,
            }),
        });
        p.student_scan_document_id = Some("scan".into());
        p.student_pages_per_student = Some(2);
        p.student_submissions.push(StudentSubmission {
            id: "submission-1".into(),
            student_id: "student-1".into(),
            document_id: "scan".into(),
            class_id: None,
            scan_batch_id: None,
            class_membership_source: None,
            page_numbers: vec![1, 2],
            status: StudentSubmissionStatus::Grouped,
            answer_slots: vec![StudentAnswerSlot {
                question_id: "q1".into(),
                question_number: 1,
                status: StudentAnswerSlotStatus::Empty,
                text: None,
                confidence: None,
                warnings: vec![],
            }],
            warnings: vec![],
            updated_at: None,
        });
        p.students.push(Student {
            id: "student-1".into(),
            display_name: None,
            number: None,
            class_name: None,
            warnings: vec![],
            identity_ocr: None,
        });
        p.student_grouping_complete_at = Some("now".into());
        let snap = evaluate_workflow(&p);
        assert_eq!(snap.current_stage, WorkflowStage::OcrReady);

        p.student_answer_ocr_records.push(StudentAnswerOcrRecord {
            id: "record-1".into(),
            submission_id: "submission-1".into(),
            question_id: "q1".into(),
            question_number: 1,
            source_page_numbers: vec![1],
            source_image_refs: vec![],
            crop_refs: vec![],
            full_page_preview_refs: vec![],
            answer_text: "cevap".into(),
            structured_answer: None,
            confidence: Some(1.0),
            status: StudentAnswerOcrStatus::TeacherApproved,
            needs_review: false,
            review_reasons: vec![],
            warnings: vec![],
            model_name: None,
            prompt_version: "test".into(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            teacher_corrected_text: None,
            teacher_reviewed_at: Some(chrono::Utc::now()),
            parse_diagnostics: None,
            render_diagnostics: None,
            ..Default::default()
        });
        let snap = evaluate_workflow(&p);
        assert!(snap
            .blocking_reasons
            .contains(&BlockingReason::StudentIdentityInvalid));
    }

    #[test]
    fn test_model_server_closed_workflow() {
        let mut p = empty_project();
        p.documents.push(Document {
            id: "1".into(),
            role: DocumentRole::ExamSource,
            file_name: "test.pdf".into(),
            stored_path: "".into(),
            page_count: 1,
            added_at: "".into(),
            checksum: None,
            preview: Some(crate::domain::document::PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: None,
                page_count: Some(1),
                job_id: None,
                error_message: None,
            }),
        });

        let mut model_status = crate::domain::model::ModelStatus {
            server_running: false,
            ..Default::default()
        };
        model_status
            .suggested_actions
            .push(crate::domain::model::ModelSuggestedAction {
                code: "start_model_server".to_string(),
                label: "Model Server’ı Başlat".to_string(),
            });
        model_status
            .suggested_actions
            .push(crate::domain::model::ModelSuggestedAction {
                code: "open_model_status_page".to_string(),
                label: "Model durumunu aç".to_string(),
            });

        let snap = evaluate_workflow_with_context(&p, &model_status, false, false);
        assert_eq!(
            snap.current_stage,
            WorkflowStage::PdfPreviewReadyQuestionTextMissing
        );
        assert!(snap
            .blocking_reasons
            .iter()
            .any(|reason| reason == &BlockingReason::ModelServerNotRunning));
        assert!(snap
            .next_actions
            .iter()
            .any(|action| action.code == "start_model_server"));
        assert!(snap
            .next_actions
            .iter()
            .any(|action| action.code == "open_model_status_page"));

        let extract_action = snap
            .next_actions
            .iter()
            .find(|action| action.code == "start_question_text_extraction")
            .expect("extraction action");
        assert!(!extract_action.enabled);
        assert!(extract_action.disabled_reason.is_some());
    }

    fn question_text_ready_rubric_missing_project() -> Project {
        let mut project = empty_project();
        project.documents.push(Document {
            id: "exam".into(),
            role: DocumentRole::ExamSource,
            file_name: "exam.pdf".into(),
            stored_path: "exam.pdf".into(),
            page_count: 1,
            added_at: "".into(),
            checksum: None,
            preview: Some(crate::domain::document::PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: None,
                page_count: Some(1),
                job_id: None,
                error_message: None,
            }),
        });
        project.questions.push(Question {
            id: "q1".into(),
            number: 1,
            max_score: 10.0,
            answer_type: AnswerType::GeneralText,
            question_text: TextFieldState {
                value: "Question".into(),
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
        });
        project
    }

    #[test]
    fn test_question_text_job_active_keeps_running_stage() {
        let mut p = empty_project();
        p.documents.push(Document {
            id: "1".into(),
            role: DocumentRole::ExamSource,
            file_name: "test.pdf".into(),
            stored_path: "".into(),
            page_count: 1,
            added_at: "".into(),
            checksum: None,
            preview: Some(crate::domain::document::PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: None,
                page_count: Some(1),
                job_id: None,
                error_message: None,
            }),
        });

        let model_status = crate::domain::model::ModelStatus {
            server_running: true,
            health_ok: true,
            ..Default::default()
        };

        let snap = evaluate_workflow_with_context(&p, &model_status, true, false);
        assert_eq!(
            snap.current_stage,
            WorkflowStage::QuestionTextExtractionRunning
        );
        assert_eq!(
            snap.summary.text.as_deref(),
            Some("Soru metni çıkarımı çalışıyor.")
        );
        assert!(snap
            .next_actions
            .iter()
            .any(|action| action.code == "open_question_text_page"));
    }

    #[test]
    fn test_inactive_question_text_job_does_not_keep_running_stage() {
        let p = question_text_ready_rubric_missing_project();

        let model_status = crate::domain::model::ModelStatus {
            server_running: true,
            health_ok: true,
            ..Default::default()
        };

        let snap = evaluate_workflow_with_context(&p, &model_status, false, false);
        assert_eq!(snap.current_stage, WorkflowStage::RubricMissing);
        assert!(snap
            .blocking_reasons
            .iter()
            .all(|reason| reason != &BlockingReason::QuestionTextMissing));
        assert!(snap
            .next_actions
            .iter()
            .any(|action| action.code == "open_rubric_preparation_page"));
    }

    #[test]
    fn test_has_active_question_text_job_ignores_inactive_jobs() {
        let jobs = vec![
            crate::domain::job::JobSnapshot {
                id: "failed".into(),
                project_id: "p".into(),
                project_root_path: None,
                kind: crate::domain::job::JobKind::QuestionTextExtraction,
                status: crate::domain::job::JobStatus::Failed,
                progress: crate::domain::job::JobProgress {
                    current: 1,
                    total: 1,
                    message: "done".into(),
                },
                started_at: None,
                finished_at: None,
                last_message: None,
                result: None,
                error: None,
                created_at: "1".into(),
                updated_at: "1".into(),
            },
            crate::domain::job::JobSnapshot {
                id: "other".into(),
                project_id: "p".into(),
                project_root_path: None,
                kind: crate::domain::job::JobKind::PdfPreviewRender,
                status: crate::domain::job::JobStatus::Running,
                progress: crate::domain::job::JobProgress {
                    current: 1,
                    total: 2,
                    message: "preview".into(),
                },
                started_at: None,
                finished_at: None,
                last_message: None,
                result: None,
                error: None,
                created_at: "2".into(),
                updated_at: "2".into(),
            },
        ];

        assert!(!has_active_question_text_job(&jobs));
    }

    #[test]
    fn test_no_suggested_question_text_no_inspect_suggestions() {
        let mut p = empty_project();
        p.documents.push(Document {
            id: "1".into(),
            role: DocumentRole::ExamSource,
            file_name: "test.pdf".into(),
            stored_path: "".into(),
            page_count: 1,
            added_at: "".into(),
            checksum: None,
            preview: Some(crate::domain::document::PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: None,
                page_count: Some(1),
                job_id: None,
                error_message: None,
            }),
        });

        let model_status = crate::domain::model::ModelStatus {
            server_running: true,
            health_ok: true,
            ..Default::default()
        };

        let snap = evaluate_workflow_with_context(&p, &model_status, false, false);
        assert!(!snap
            .next_actions
            .iter()
            .any(|action| action.code == "open_question_text_page"));
    }

    #[test]
    fn test_suggested_question_text_shows_inspect_suggestions() {
        let mut p = empty_project();
        p.documents.push(Document {
            id: "1".into(),
            role: DocumentRole::ExamSource,
            file_name: "test.pdf".into(),
            stored_path: "".into(),
            page_count: 1,
            added_at: "".into(),
            checksum: None,
            preview: Some(crate::domain::document::PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: None,
                page_count: Some(1),
                job_id: None,
                error_message: None,
            }),
        });
        p.questions.push(Question {
            id: "q1".into(),
            number: 1,
            max_score: 10.0,
            answer_type: AnswerType::GeneralText,
            question_text: TextFieldState {
                value: "Suggested".into(),
                source: TextFieldSource::ExamPdf,
                status: TextFieldStatus::Suggested,
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
        });

        let model_status = crate::domain::model::ModelStatus {
            server_running: true,
            health_ok: true,
            ..Default::default()
        };

        let snap = evaluate_workflow_with_context(&p, &model_status, false, false);
        assert_eq!(snap.current_stage, WorkflowStage::QuestionTextSuggested);
        let inspect_action = snap
            .next_actions
            .iter()
            .find(|action| action.code == "open_question_text_page")
            .expect("inspect action");
        assert_eq!(inspect_action.label, "Önerileri İncele");
        assert!(inspect_action.enabled);
    }

    fn rubric_missing_pdf_uploaded_project() -> Project {
        let mut project = empty_project();
        project.documents.push(Document {
            id: "exam".into(),
            role: DocumentRole::ExamSource,
            file_name: "exam.pdf".into(),
            stored_path: "exam.pdf".into(),
            page_count: 1,
            added_at: "".into(),
            checksum: None,
            preview: Some(crate::domain::document::PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: None,
                page_count: Some(1),
                job_id: None,
                error_message: None,
            }),
        });
        project.documents.push(Document {
            id: "rubric_pdf".into(),
            role: DocumentRole::Rubric,
            file_name: "rubric.pdf".into(),
            stored_path: "rubric.pdf".into(),
            page_count: 1,
            added_at: "".into(),
            checksum: None,
            preview: None,
        });
        project.questions.push(Question {
            id: "q1".into(),
            number: 1,
            max_score: 0.0,
            answer_type: AnswerType::GeneralText,
            question_text: TextFieldState {
                value: "Question".into(),
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
        });
        project
    }

    #[test]
    fn test_rubric_pdf_import_model_server_closed_workflow() {
        let p = rubric_missing_pdf_uploaded_project();
        let mut model_status = crate::domain::model::ModelStatus {
            server_running: false,
            health_ok: false,
            ..Default::default()
        };
        model_status
            .suggested_actions
            .push(crate::domain::model::ModelSuggestedAction {
                code: "start_model_server".to_string(),
                label: "Model Server’ı Başlat".to_string(),
            });

        let snap = evaluate_workflow_with_context(&p, &model_status, false, false);

        assert_eq!(snap.current_stage, WorkflowStage::RubricMissing);
        assert!(snap
            .blocking_reasons
            .iter()
            .any(|reason| reason == &BlockingReason::ModelServerNotRunning));
        assert!(snap
            .blocking_reasons
            .iter()
            .any(|reason| reason == &BlockingReason::RubricMissing));

        let import_action = snap
            .next_actions
            .iter()
            .find(|action| action.code == "start_rubric_pdf_import")
            .expect("action exists");
        assert!(!import_action.enabled);
        assert!(import_action
            .disabled_reason
            .as_ref()
            .unwrap()
            .contains("model sunucusu çalışmalıdır"));

        assert!(snap
            .next_actions
            .iter()
            .any(|action| action.code == "start_model_server"));
    }

    #[test]
    fn test_rubric_pdf_import_model_server_open_workflow() {
        let p = rubric_missing_pdf_uploaded_project();
        let model_status = crate::domain::model::ModelStatus {
            server_running: true,
            health_ok: true,
            ..Default::default()
        };

        let snap = evaluate_workflow_with_context(&p, &model_status, false, false);

        assert_eq!(snap.current_stage, WorkflowStage::RubricMissing);
        assert!(!snap
            .blocking_reasons
            .iter()
            .any(|reason| reason == &BlockingReason::ModelServerNotRunning));
        assert!(snap
            .blocking_reasons
            .iter()
            .any(|reason| reason == &BlockingReason::RubricMissing));

        let import_action = snap
            .next_actions
            .iter()
            .find(|action| action.code == "start_rubric_pdf_import")
            .expect("action exists");
        assert!(import_action.enabled);
        assert!(import_action.disabled_reason.is_none());
    }

    #[test]
    fn test_student_answer_ocr_ready_for_scoring_after_review() {
        let p = student_answer_ocr_ready_project();
        let snap = evaluate_workflow(&p);

        assert_eq!(snap.current_stage, WorkflowStage::ScoringReady);
        assert!(snap
            .next_actions
            .iter()
            .any(|action| action.code == "open_scoring_page" && action.enabled));
        assert!(snap
            .next_actions
            .iter()
            .any(|action| action.code == "start_scoring_job" && action.enabled));
        assert!(snap
            .summary
            .steps
            .iter()
            .any(|step| step.code == "student_answer_ocr" && step.status == "succeeded"));
    }

    #[test]
    fn test_student_answer_ocr_stays_in_review_until_all_records_are_approved() {
        let mut p = student_answer_ocr_ready_project();
        p.student_answer_ocr_records[0].status = StudentAnswerOcrStatus::ReviewNeeded;
        p.student_answer_ocr_records[0].needs_review = true;

        let snap = evaluate_workflow(&p);

        assert_eq!(
            snap.current_stage,
            WorkflowStage::StudentAnswerOcrReviewNeeded
        );
        assert_ne!(
            snap.current_stage,
            WorkflowStage::StudentAnswerOcrReadyForScoring
        );
        assert!(snap
            .next_actions
            .iter()
            .any(|action| action.code == "rerun_student_answer_ocr"));
    }
}

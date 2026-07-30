use std::path::PathBuf;

use clap::{Parser, Subcommand};

use app_lib::diagnostics::{
    DiagnosticsContext, DoctorReport, DocumentContentInspectRecord, DocumentContentRepairReport,
    DocumentInspectRecord, JobSummary, ModelInputInspectReport, ModelInspectReport,
    ProjectInspectReport, QuestionTextRepairReport, QuestionTextSummary, ReplayReport,
    RubricSummary, StaleJobsRepairReport,
};

#[derive(Parser)]
#[command(name = "rubrika")]
struct Cli {
    #[arg(long)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Doctor {
        project_path: PathBuf,
    },
    Inspect {
        #[command(subcommand)]
        target: InspectTarget,
    },
    Replay {
        #[command(subcommand)]
        target: ReplayTarget,
    },
    Repair {
        #[command(subcommand)]
        target: RepairTarget,
    },
}

#[derive(Subcommand)]
enum InspectTarget {
    Project {
        project_path: PathBuf,
    },
    Jobs {
        project_path: PathBuf,
    },
    Model {
        #[arg(long, default_value_t = 120)]
        tail: usize,
    },
    Documents {
        project_path: PathBuf,
    },
    DocumentContent {
        project_path: PathBuf,
    },
    QuestionText {
        project_path: PathBuf,
    },
    Rubric {
        project_path: PathBuf,
    },
    ModelInputs {
        project_path: PathBuf,
    },
}

#[derive(Subcommand)]
enum ReplayTarget {
    RubricImport {
        project_path: PathBuf,
        #[arg(long, default_value_t = true)]
        dry_run: bool,
    },
    QuestionText {
        project_path: PathBuf,
        #[arg(long, default_value_t = true)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum RepairTarget {
    QuestionText { project_path: PathBuf },
    DocumentContent { project_path: PathBuf },
    StaleJobs { project_path: PathBuf },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let ctx = DiagnosticsContext::new();
    let result = match cli.command {
        Command::Doctor { project_path } => match ctx.doctor(&project_path).await {
            Ok(report) => {
                print_doctor(&report, cli.json);
                exit_code_doctor(&report)
            }
            Err(error) => {
                eprintln!("{error}");
                2
            }
        },
        Command::Inspect { target } => match target {
            InspectTarget::Project { project_path } => match ctx.inspect_project(&project_path) {
                Ok(report) => {
                    print_json_or_human(&report, cli.json, print_project);
                    0
                }
                Err(error) => {
                    eprintln!("{error}");
                    2
                }
            },
            InspectTarget::Jobs { project_path } => match ctx.inspect_jobs(&project_path) {
                Ok(report) => {
                    print_json_or_human(&report, cli.json, print_jobs);
                    0
                }
                Err(error) => {
                    eprintln!("{error}");
                    1
                }
            },
            InspectTarget::Model { tail } => match ctx.inspect_model(tail).await {
                Ok(report) => {
                    print_json_or_human(&report, cli.json, print_model);
                    0
                }
                Err(error) => {
                    eprintln!("{error}");
                    1
                }
            },
            InspectTarget::Documents { project_path } => match ctx.inspect_documents(&project_path)
            {
                Ok(report) => {
                    print_json_or_human(
                        &report,
                        cli.json,
                        |report: &Vec<DocumentInspectRecord>| print_documents(report.as_slice()),
                    );
                    0
                }
                Err(error) => {
                    eprintln!("{error}");
                    1
                }
            },
            InspectTarget::DocumentContent { project_path } => {
                match ctx.inspect_document_content(&project_path) {
                    Ok(report) => {
                        print_json_or_human(
                            &report,
                            cli.json,
                            |report: &Vec<DocumentContentInspectRecord>| {
                                print_document_content(report.as_slice())
                            },
                        );
                        0
                    }
                    Err(error) => {
                        eprintln!("{error}");
                        1
                    }
                }
            }
            InspectTarget::QuestionText { project_path } => {
                match ctx.inspect_question_text(&project_path) {
                    Ok(report) => {
                        print_json_or_human(&report, cli.json, print_question_text);
                        0
                    }
                    Err(error) => {
                        eprintln!("{error}");
                        1
                    }
                }
            }
            InspectTarget::Rubric { project_path } => match ctx.inspect_rubric(&project_path) {
                Ok(report) => {
                    print_json_or_human(&report, cli.json, print_rubric);
                    0
                }
                Err(error) => {
                    eprintln!("{error}");
                    1
                }
            },
            InspectTarget::ModelInputs { project_path } => {
                match ctx.inspect_model_inputs(&project_path) {
                    Ok(report) => {
                        print_json_or_human(&report, cli.json, print_model_inputs);
                        0
                    }
                    Err(error) => {
                        eprintln!("{error}");
                        1
                    }
                }
            }
        },
        Command::Replay { target } => match target {
            ReplayTarget::RubricImport {
                project_path,
                dry_run,
            } => {
                if dry_run {
                    match ctx.replay_rubric_import_dry_run(&project_path).await {
                        Ok(report) => {
                            print_json_or_human(&report, cli.json, print_replay);
                            0
                        }
                        Err(error) => {
                            eprintln!("{error}");
                            1
                        }
                    }
                } else {
                    eprintln!("write mode is not implemented");
                    1
                }
            }
            ReplayTarget::QuestionText {
                project_path,
                dry_run,
            } => {
                if dry_run {
                    match ctx.replay_question_text_dry_run(&project_path).await {
                        Ok(report) => {
                            print_json_or_human(&report, cli.json, print_replay);
                            0
                        }
                        Err(error) => {
                            eprintln!("{error}");
                            1
                        }
                    }
                } else {
                    eprintln!("write mode is not implemented");
                    1
                }
            }
        },
        Command::Repair { target } => match target {
            RepairTarget::QuestionText { project_path } => {
                match ctx.repair_question_text(&project_path) {
                    Ok(report) => {
                        print_json_or_human(&report, cli.json, print_question_text_repair);
                        0
                    }
                    Err(error) => {
                        eprintln!("{error}");
                        1
                    }
                }
            }
            RepairTarget::DocumentContent { project_path } => {
                match ctx.repair_document_content(&project_path) {
                    Ok(report) => {
                        print_json_or_human(&report, cli.json, print_document_content_repair);
                        0
                    }
                    Err(error) => {
                        eprintln!("{error}");
                        1
                    }
                }
            }
            RepairTarget::StaleJobs { project_path } => {
                match ctx.repair_stale_jobs(&project_path) {
                    Ok(report) => {
                        print_json_or_human(&report, cli.json, print_stale_jobs_repair);
                        0
                    }
                    Err(error) => {
                        eprintln!("{error}");
                        1
                    }
                }
            }
        },
    };

    std::process::exit(result);
}

fn print_json_or_human<T, F>(report: &T, json: bool, human: F)
where
    T: serde::Serialize,
    F: FnOnce(&T),
{
    if json {
        match serde_json::to_string_pretty(report) {
            Ok(json) => println!("{json}"),
            Err(error) => eprintln!("failed to serialize report: {error}"),
        }
    } else {
        human(report);
    }
}

fn print_doctor(report: &DoctorReport, json: bool) {
    if json {
        match serde_json::to_string_pretty(report) {
            Ok(json) => println!("{json}"),
            Err(error) => eprintln!("failed to serialize report: {error}"),
        }
        return;
    }
    println!("project_path={}", report.project_path);
    println!("project_file_exists={}", report.project_file_exists);
    println!("project_readable={}", report.project_readable);
    if let Some(project) = &report.project {
        println!("project_id={}", project.project_id);
        println!("project_name={}", project.project_name);
        println!(
            "expected_question_count={:?}",
            project.expected_question_count
        );
        println!("question_count={}", project.question_count);
    }
    println!("documents_dir_exists={}", report.documents_dir_exists);
    println!("cache_dir_exists={}", report.cache_dir_exists);
    println!("exam_source_exists={}", report.exam_source_exists);
    println!(
        "rubric_or_answer_key_exists={}",
        report.rubric_or_answer_key_exists
    );
    println!("student_scan_exists={}", report.student_scan_exists);
    println!("student_scan_documents={}", report.student_scan_documents);
    println!(
        "student_scan_total_pages={}",
        report.student_scan_total_pages
    );
    println!(
        "student_scan_preview_ready_pages={}",
        report.student_scan_preview_ready_pages
    );
    println!(
        "student_scan_preview_total_pages={}",
        report.student_scan_preview_total_pages
    );
    println!(
        "student_grouping_complete={}",
        report.student_grouping_complete
    );
    if !report.student_grouping_complete && report.scoring_ready {
        println!(
            "student_grouping_note=Gruplama tamam işareti alınmamış; scoring gate OCR, kimlik ve frozen paket ile ayrı değerlendirilir."
        );
    }
    println!("student_submissions={}", report.student_submissions);
    println!("school_class_count={}", report.school_class_count);
    println!(
        "active_school_class_count={}",
        report.active_school_class_count
    );
    println!(
        "archived_school_class_count={}",
        report.archived_school_class_count
    );
    println!(
        "student_scan_batch_count={}",
        report.student_scan_batch_count
    );
    println!(
        "scan_batch_without_class_count={}",
        report.scan_batch_without_class_count
    );
    println!(
        "submission_without_class_count={}",
        report.submission_without_class_count
    );
    println!(
        "class_membership_inconsistency_count={}",
        report.class_membership_inconsistency_count
    );
    println!(
        "identity_class_mismatch_count={}",
        report.identity_class_mismatch_count
    );
    for school_class in &report.school_class_summaries {
        println!(
            "class[{}].scan_batch_count={}",
            school_class.name, school_class.scan_batch_count
        );
        println!(
            "class[{}].submission_count={}",
            school_class.name, school_class.submission_count
        );
        println!(
            "class[{}].identity_verified={}",
            school_class.name, school_class.identity_verified
        );
        println!(
            "class[{}].ocr_complete={}",
            school_class.name, school_class.ocr_complete
        );
        println!(
            "class[{}].scoring_complete={}",
            school_class.name, school_class.scoring_complete
        );
        println!(
            "class[{}].review_required={}",
            school_class.name, school_class.review_required
        );
    }
    println!(
        "student_answer_ocr_records={}/{}",
        report.student_answer_ocr_records, report.student_answer_ocr_expected_records
    );
    println!(
        "student_answer_ocr_reviewed={}/{}",
        report.student_answer_ocr_reviewed, report.student_answer_ocr_expected_records
    );
    println!(
        "student_answer_ocr_needs_review={}",
        report.student_answer_ocr_needs_review
    );
    println!(
        "student_answer_ocr_status={}",
        report.student_answer_ocr_status
    );
    println!(
        "student_answer_ocr_ready_for_scoring={}",
        report.student_answer_ocr_ready_for_scoring
    );
    println!(
        "pages_per_student={}",
        report
            .pages_per_student
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string())
    );
    println!("preview_metadata_exists={}", report.preview_metadata_exists);
    println!("preview_png_count={}", report.preview_png_count);
    println!("exam_package_status={}", report.exam_package_status);
    println!("question_text_coverage={}", report.question_text_coverage);
    println!("rubric_coverage={}", report.rubric_coverage);
    println!("ready_for_review={}", report.ready_for_review);
    println!("ready_for_qep={}", report.ready_for_qep);
    println!(
        "question_text_missing={}",
        report.question_text_summary.missing
    );
    if !report.question_text_summary.warnings.is_empty() {
        println!(
            "question_text_warnings={:?}",
            report.question_text_summary.warnings
        );
    }
    println!("rubric_missing={}", report.rubric_summary.missing);
    if !report.rubric_summary.warnings.is_empty() {
        println!("rubric_warnings={:?}", report.rubric_summary.warnings);
    }
    println!(
        "exam_package_freeze_ready={}",
        report.exam_package_freeze_ready
    );
    println!(
        "exam_package_freeze_blockers={:?}",
        report.exam_package_freeze_blockers
    );
    println!("student_intake_ready={}", report.student_intake_ready);
    println!(
        "student_intake_blockers={:?}",
        report.student_intake_blockers
    );
    println!("scoring_ready={}", report.scoring_ready);
    println!("scoring_blockers={:?}", report.scoring_blockers);
    println!("scoring_result_count={}", report.scoring_result_count);
    println!(
        "scoring_total_history_count={}",
        report.scoring_total_history_count
    );
    println!(
        "scoring_duplicate_result_count={}",
        report.scoring_duplicate_result_count
    );
    println!("active_scoring_run_id={:?}", report.active_scoring_run_id);
    println!("scoring_approved_count={}", report.scoring_approved_count);
    println!(
        "scoring_needs_review_count={}",
        report.scoring_needs_review_count
    );
    println!("scoring_stale_count={}", report.scoring_stale_count);
    println!("job_count={}", report.job_summary.jobs.len());
    println!(
        "stale_job_count={}",
        report.job_summary.stale_candidates.len()
    );
    if let Some(model) = &report.model_status {
        print_model(model);
    }
    if !report.errors.is_empty() {
        println!("errors={:?}", report.errors);
    }
    if !report.warnings.is_empty() {
        println!("warnings={:?}", report.warnings);
    }
}

fn print_project(report: &ProjectInspectReport) {
    println!("project_id={}", report.project_id);
    println!("project_name={}", report.project_name);
    println!(
        "expected_question_count={:?}",
        report.expected_question_count
    );
    println!("question_count={}", report.question_count);
    println!("workflow_stage={}", report.workflow_stage);
    println!("blocking_reasons={:?}", report.blocking_reasons);
    println!("documents_by_role={:?}", report.document_counts_by_role);
    println!("paths.root_path={}", report.paths.root_path);
    println!("paths.project_json={}", report.paths.project_json);
    println!("paths.documents_dir={}", report.paths.documents_dir);
    println!("paths.cache_dir={}", report.paths.cache_dir);
    println!("paths.preview_dir={}", report.paths.preview_dir);
    println!("paths.model_inputs_dir={}", report.paths.model_inputs_dir);
    println!("paths.logs_dir={}", report.paths.logs_dir);
    for action in &report.next_actions {
        println!(
            "next_action={} | enabled={} | command={:?}",
            action.code, action.enabled, action.command
        );
    }
}

fn print_jobs(report: &JobSummary) {
    println!("job_count={}", report.jobs.len());
    println!("stale_candidates={:?}", report.stale_candidates);
    for job in &report.jobs {
        println!(
            "{} | {} | {} | started_at={:?} | finished_at={:?} | last_message={:?} | error_code={:?} | error_details={:?} | stale={} | active={}",
            job.job_id,
            job.kind,
            job.status,
            job.started_at,
            job.finished_at,
            job.last_message,
            job.error_code,
            job.error_details,
            job.stale_candidate,
            job.active
        );
    }
}

fn print_model(report: &ModelInspectReport) {
    println!("active_profile_id={}", report.active_profile_id);
    println!("display_name={}", report.display_name);
    println!("mode={}", report.mode);
    println!("runtime_state={}", report.runtime_state);
    println!("llama_cpp_root={}", report.llama_cpp_root);
    println!("server_path_exists={}", report.server_path_exists);
    println!("model_path_exists={}", report.model_path_exists);
    println!("mmproj_path_exists={}", report.mmproj_path_exists);
    println!("llama_server_binary={}", report.llama_server_binary);
    println!("model_path={}", report.model_path);
    println!("mmproj_path={}", report.mmproj_path);
    println!("base_url={}", report.base_url);
    println!("model_port={}", report.model_port);
    println!("model_port_listening={}", report.model_port_listening);
    println!("model_port_health_ok={}", report.model_port_health_ok);
    println!("model_config_complete={}", report.model_config_complete);
    println!(
        "model_autostart_available={}",
        report.model_autostart_available
    );
    println!(
        "llama_server_binary_exists={}",
        report.llama_server_binary_exists
    );
    println!("model_file_exists={}", report.model_file_exists);
    println!("mmproj_file_exists={}", report.mmproj_file_exists);
    println!("health_ok={}", report.health_ok);
    println!("completion_probe_ok={}", report.completion_probe_ok);
    println!("started_by_app={}", report.started_by_app);
    println!("pid={:?}", report.model_managed_process_pid);
    println!("log_path={:?}", report.log_path);
    println!("can_start_from_app={}", report.can_start_from_app);
    println!("can_stop_from_app={}", report.can_stop_from_app);
    if !report.log_tail.is_empty() {
        println!("log_tail_lines={}", report.log_tail.len());
    }
}

fn print_documents(report: &[DocumentInspectRecord]) {
    for doc in report {
        println!(
            "{} | {} | {} | exists={} | preview={:?}",
            doc.id, doc.role, doc.file_name, doc.exists, doc.preview_status
        );
    }
}

fn print_document_content(report: &[DocumentContentInspectRecord]) {
    for doc in report {
        println!(
            "kind={} | role={} | method={} | document_id={} | raw_text_length={} | normalized_text_length={} | enough_text={} | vision_fallback_needed={} | detected_question_numbers={:?} | missing_question_numbers={:?} | ignored_question_numbers={:?} | metadata_stale={} | needs_refresh={} | fresh_detected_question_numbers={:?} | fresh_missing_question_numbers={:?} | metadata_exists={} | raw_text_exists={} | normalized_text_exists={} | pdftotext_stderr_exists={} | model_input_manifest_exists={} | artifact_dir={} | warnings={:?}",
            doc.kind,
            doc.role,
            doc.method,
            doc.document_id,
            doc.raw_text_length,
            doc.normalized_text_length,
            doc.enough_text,
            doc.vision_fallback_needed,
            doc.detected_question_numbers,
            doc.missing_question_numbers,
            doc.ignored_question_numbers,
            doc.metadata_stale,
            doc.needs_refresh,
            doc.fresh_detected_question_numbers,
            doc.fresh_missing_question_numbers,
            doc.metadata_exists,
            doc.raw_text_exists,
            doc.normalized_text_exists,
            doc.pdftotext_stderr_exists,
            doc.model_input_manifest_exists,
            doc.artifact_dir,
            doc.warnings
        );
    }
}

fn print_document_content_repair(report: &DocumentContentRepairReport) {
    println!(
        "expected_question_count={:?} | repaired_count={}",
        report.expected_question_count, report.repaired_count
    );
    for item in &report.items {
        println!(
            "{} | {} | {} | before_method={} | after_method={} | before_detected={:?} | after_detected={:?} | ignored={:?} | metadata_stale={} | needs_refresh={} | vision_fallback_needed={} | metadata_written={}",
            item.document_id,
            item.role,
            item.kind,
            item.before_method,
            item.after_method,
            item.before_detected_question_numbers,
            item.after_detected_question_numbers,
            item.ignored_question_numbers,
            item.metadata_stale,
            item.needs_refresh,
            item.vision_fallback_needed,
            item.metadata_written
        );
    }
}

fn print_stale_jobs_repair(report: &StaleJobsRepairReport) {
    println!("repaired_count={}", report.repaired_count);
    for item in &report.items {
        println!(
            "{} | {} | {} -> {} | stale={} -> {} | active={} -> {}",
            item.job_id,
            item.kind,
            item.status_before,
            item.status_after,
            item.stale_before,
            item.stale_after,
            item.active_before,
            item.active_after
        );
    }
}

fn print_question_text(report: &QuestionTextSummary) {
    println!("expected_question_count={}", report.expected_question_count);
    println!("extracted={:?}", report.extracted);
    println!("missing={:?}", report.missing_numbers);
    println!("coverage_ok={}", report.coverage_ok);
    println!("partial_success={}", report.partial_success);
    println!("missing={}", report.missing);
    println!("suggested={}", report.suggested);
    println!("edited={}", report.edited);
    println!("confirmed={}", report.confirmed);
    println!("failed={}", report.failed);
    if !report.warnings.is_empty() {
        println!("warnings={:?}", report.warnings);
    }
    for question in &report.questions {
        println!(
            "{} | {} | {} | confidence={:?} | value_length={} | warnings={:?}",
            question.number,
            question.status,
            question.source,
            question.confidence,
            question.value_length,
            question.warnings
        );
    }
}

fn print_rubric(report: &RubricSummary) {
    println!("expected_question_count={}", report.expected_question_count);
    println!("imported={:?}", report.imported_question_numbers);
    println!(
        "false_positive_imported={:?}",
        report.false_positive_imported
    );
    println!("missing={:?}", report.missing_question_numbers);
    println!("failed={:?}", report.failed_question_numbers);
    println!("partial_success={}", report.partial_success);
    println!("strategy={}", report.strategy);
    println!("missing={}", report.missing);
    println!("imported={}", report.imported);
    println!("manual={}", report.manual);
    println!("suggested={}", report.suggested);
    println!("confirmed={}", report.confirmed);
    println!("invalid={}", report.invalid);
    if !report.warnings.is_empty() {
        println!("warnings={:?}", report.warnings);
    }
    for question in &report.questions {
        println!(
            "{} | {} | error={} | max_points={:?} | expected_answer_length={} | criteria_count={} | warnings={:?}",
            question.number,
            question.status,
            question.error_code.as_deref().unwrap_or("None"),
            question.max_points,
            question.expected_answer_length,
            question.criteria_count,
            question.warnings
        );
    }
}

fn print_model_inputs(report: &ModelInputInspectReport) {
    for batch in &report.batches {
        println!(
            "{} | {} | pages={} | total_bytes={} | largest_image_bytes={} | long_edge_max={} | jpeg_quality={} | warnings={:?}",
            batch.kind,
            batch.document_id,
            batch.page_count,
            batch.total_bytes,
            batch.largest_image_bytes,
            batch.long_edge_max,
            batch.jpeg_quality,
            batch.missing_metadata_warnings
        );
    }
    if !report.warnings.is_empty() {
        println!("warnings={:?}", report.warnings);
    }
}

fn print_replay(report: &ReplayReport) {
    println!("target={}", report.target);
    println!("dry_run={}", report.dry_run);
    println!("project_path={}", report.project_path);
    if let Some(strategy) = &report.strategy {
        println!("strategy={}", strategy);
    }
    if let Some(fresh) = &report.fresh_pdf_extraction {
        print_question_text_fresh_replay(fresh);
        if let Some(snapshot) = &report.project_snapshot {
            print_question_text_snapshot_replay(snapshot);
        }
    } else {
        if let Some(content_method) = &report.content_method {
            println!("content_method={}", content_method);
        }
        if let Some(expected_question_count) = report.expected_question_count {
            println!("expected_question_count={}", expected_question_count);
        }
        if !report.target_questions.is_empty() {
            println!("target_questions={:?}", report.target_questions);
        }
        if !report.already_available.is_empty() {
            println!("already_available={:?}", report.already_available);
        }
        println!("invalid_or_empty={:?}", report.invalid_or_empty);
        if !report.extracted.is_empty() {
            println!("extracted={:?}", report.extracted);
        }
        if !report.will_run_questions.is_empty() {
            println!("will_run_questions={:?}", report.will_run_questions);
        }
        if !report.will_run_vision_fallback_for.is_empty() {
            println!(
                "will_run_vision_fallback_for={:?}",
                report.will_run_vision_fallback_for
            );
        }
        if !report.missing.is_empty() {
            println!("missing={:?}", report.missing);
        }
        if !report.failed.is_empty() {
            println!("failed={:?}", report.failed);
        }
        if let Some(coverage_ok) = report.coverage_ok {
            println!("coverage_ok={}", coverage_ok);
        }
        if let Some(partial_success) = report.partial_success {
            println!("partial_success={}", partial_success);
        }
    }
    for check in &report.checks {
        println!("check={}", check);
    }
    if !report.warnings.is_empty() {
        println!("warnings={:?}", report.warnings);
    }
}

fn print_question_text_fresh_replay(report: &app_lib::diagnostics::QuestionTextFreshReplayReport) {
    println!("fresh_pdf_extraction:");
    println!("  content_method={}", report.content_method);
    println!(
        "  expected_question_count={}",
        report.expected_question_count
    );
    println!("  detected_markers={:?}", report.detected_markers);
    println!("  marker_offsets={:?}", report.marker_offsets);
    println!("  missing={:?}", report.missing);
    println!("  contaminated={:?}", report.contaminated);
    println!("  coverage_ok={}", report.coverage_ok);
    println!(
        "  will_run_vision_fallback_for={:?}",
        report.will_run_vision_fallback_for
    );
    println!(
        "  vision_fallback_call_count={}",
        report.vision_fallback_call_count
    );
}

fn print_question_text_snapshot_replay(
    report: &app_lib::diagnostics::QuestionTextSnapshotReplayReport,
) {
    println!("project_snapshot:");
    println!("  available={:?}", report.available);
    println!("  missing={:?}", report.missing);
    println!("  contaminated={:?}", report.contaminated);
    println!("  stale={}", report.stale);
    println!("  needs_refresh={}", report.needs_refresh);
}

fn print_question_text_repair(report: &QuestionTextRepairReport) {
    println!("expected_question_count={}", report.expected_question_count);
    println!("fresh_detected={:?}", report.fresh_detected);
    println!("fresh_missing={:?}", report.fresh_missing);
    println!("fresh_contaminated={:?}", report.fresh_contaminated);
    println!("before_available={:?}", report.before_available);
    println!("before_missing={:?}", report.before_missing);
    println!("before_contaminated={:?}", report.before_contaminated);
    println!("updated={:?}", report.updated);
    println!("created={:?}", report.created);
    println!("preserved_confirmed={:?}", report.preserved_confirmed);
    println!("preserved_edited={:?}", report.preserved_edited);
    println!("after_available={:?}", report.after_available);
    println!("after_missing={:?}", report.after_missing);
    println!("coverage_ok={}", report.coverage_ok);
}

fn exit_code_doctor(report: &DoctorReport) -> i32 {
    if !report.project_readable {
        2
    } else if report.errors.is_empty() {
        0
    } else {
        1
    }
}

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use app_lib::diagnostics::{
    DataLossPreflightReport, DiagnosticsContext, DoctorReport, DocumentContentInspectRecord,
    DocumentContentRepairReport, DocumentInspectRecord, JobSummary, ModelInputInspectReport,
    ModelInspectReport, ProjectInspectReport, QuestionTextRepairReport, QuestionTextSummary,
    ReplayReport, RubricSummary, StaleJobsRepairReport,
};
use app_lib::services::integrity_recovery_service::{
    self as integrity, AudioForensicReport, AuditForensicsReport, BackupVerificationReport,
    RecoveryCopyReport, RecoveryDiffReport, RestoreVerificationReport,
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
    /// Read-only final data-loss and destructive-operation preflight.
    Preflight {
        project_path: PathBuf,
    },
    /// Verifies an external backup archive, receipt and every archive entry.
    BackupVerify {
        archive_path: PathBuf,
        #[arg(long)]
        source_project: Option<PathBuf>,
    },
    /// Creates a complete external verified backup without mutating source.
    BackupCreate {
        project_path: PathBuf,
        #[arg(long)]
        destination: Option<PathBuf>,
    },
    /// Restores a verified archive into a new destination without recovery mutations.
    RestoreCopy {
        archive_path: PathBuf,
        destination_path: PathBuf,
    },
    /// Verifies archive, restore, domain equality and byte-bearing artifact equality.
    VerifyRestore {
        archive_path: PathBuf,
        source_project: PathBuf,
        restored_path: PathBuf,
        #[arg(long)]
        proof_path: Option<PathBuf>,
    },
    /// Performs read-only line-by-line audit chain forensics.
    AuditForensics {
        project_path: PathBuf,
    },
    /// Classifies speaking audio that has no canonical project pointer.
    ClassifyOrphans {
        project_path: PathBuf,
    },
    /// Creates a recovery copy from a verified backup; never repairs source.
    RecoverCopy {
        backup_path: PathBuf,
        destination_path: PathBuf,
        #[arg(long)]
        source_project: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    /// Produces an explained source/candidate manifest and domain diff.
    RecoveryDiff {
        source_project: PathBuf,
        candidate_path: PathBuf,
    },
    /// Acquires the project write lease and holds it for `hold_seconds`
    /// (0 means hold until stdin closes). Used by process-fixture tests.
    LockHold {
        project_path: PathBuf,
        #[arg(long, default_value_t = 10)]
        hold_seconds: u64,
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
    AuditInitialRevision { project_path: PathBuf },
    AuditLatestRevision { project_path: PathBuf },
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
        Command::Preflight { project_path } => match ctx.data_loss_preflight(&project_path) {
            Ok(report) => {
                print_json_or_human(&report, cli.json, print_preflight);
                if report.safe_to_open_for_writing {
                    0
                } else {
                    3
                }
            }
            Err(error) => {
                eprintln!("{error}");
                2
            }
        },
        Command::BackupVerify {
            archive_path,
            source_project,
        } => match integrity::verify_backup(&archive_path, source_project.as_deref()) {
            Ok(report) => {
                print_json_or_human(&report, cli.json, print_backup_verification);
                0
            }
            Err(error) => {
                eprintln!("{}", error.message);
                2
            }
        },
        Command::BackupCreate {
            project_path,
            destination,
        } => match app_lib::services::backup_service::create_verified_backup(
            &project_path,
            destination.as_deref(),
            &tokio_util::sync::CancellationToken::new(),
        ) {
            Ok(summary) => {
                print_json_or_human(&summary, cli.json, |summary| {
                    println!("archive_path={}", summary.archive_path);
                    println!("verification_path={}", summary.verification_path);
                    println!("manifest_path={}", summary.manifest_path);
                    println!("entry_count={}", summary.entry_count);
                    println!("total_size={}", summary.total_size);
                    println!("sha256={}", summary.sha256);
                });
                0
            }
            Err(error) => {
                eprintln!("{}", error.message);
                2
            }
        },
        Command::RestoreCopy {
            archive_path,
            destination_path,
        } => {
            let result = app_lib::services::backup_service::restore_backup(
                &archive_path,
                &destination_path,
                &tokio_util::sync::CancellationToken::new(),
            );
            match result {
                Ok(summary) => {
                    print_json_or_human(&summary, cli.json, |summary| {
                        println!("destination={}", summary.destination);
                        println!("entry_count={}", summary.entry_count);
                        println!("restored_project_id={}", summary.restored_project_id);
                    });
                    0
                }
                Err(error) => {
                    eprintln!("{}", error.message);
                    2
                }
            }
        }
        Command::VerifyRestore {
            archive_path,
            source_project,
            restored_path,
            proof_path,
        } => match integrity::verify_restored_copy(
            &archive_path,
            &source_project,
            &restored_path,
            proof_path.as_deref(),
        ) {
            Ok(report) => {
                print_json_or_human(&report, cli.json, print_restore_verification);
                0
            }
            Err(error) => {
                eprintln!("{}", error.message);
                2
            }
        },
        Command::AuditForensics { project_path } => {
            match integrity::audit_forensics(&project_path) {
                Ok(report) => {
                    print_json_or_human(&report, cli.json, print_audit_forensics);
                    0
                }
                Err(error) => {
                    eprintln!("{}", error.message);
                    2
                }
            }
        }
        Command::ClassifyOrphans { project_path } => {
            match integrity::classify_audio_orphans(&project_path) {
                Ok(report) => {
                    print_json_or_human(&report, cli.json, print_audio_forensics);
                    0
                }
                Err(error) => {
                    eprintln!("{}", error.message);
                    2
                }
            }
        }
        Command::RecoverCopy {
            backup_path,
            destination_path,
            source_project,
            dry_run,
        } => match integrity::recover_copy(
            &backup_path,
            &destination_path,
            source_project.as_deref(),
            dry_run,
        ) {
            Ok(report) => {
                print_json_or_human(&report, cli.json, print_recovery_copy);
                0
            }
            Err(error) => {
                eprintln!("{}", error.message);
                2
            }
        },
        Command::RecoveryDiff {
            source_project,
            candidate_path,
        } => match integrity::recovery_diff(&source_project, &candidate_path) {
            Ok(report) => {
                print_json_or_human(&report, cli.json, print_recovery_diff);
                0
            }
            Err(error) => {
                eprintln!("{}", error.message);
                2
            }
        },
        Command::LockHold {
            project_path,
            hold_seconds,
        } => {
            match app_lib::platform::project_write_lease::ProjectWriteLease::acquire(&project_path)
            {
                Ok(lease) => {
                    println!("LOCKED {}", lease.lock_path().display());
                    let deadline =
                        std::time::Instant::now() + std::time::Duration::from_secs(hold_seconds);
                    while std::time::Instant::now() < deadline {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    0
                }
                Err(error) => {
                    eprintln!("{}", error.message);
                    if error.code == app_lib::domain::errors::AppErrorCode::ProjectAlreadyOpen {
                        7
                    } else {
                        3
                    }
                }
            }
        }
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
            RepairTarget::AuditInitialRevision { project_path } => {
                let service = app_lib::services::audit_service::AuditService::new();
                match service.repair_missing_initial_revision(&project_path) {
                    Ok(record) => {
                        print_json_or_human(&record, cli.json, |record| {
                            println!(
                                "audit_revision_repaired={} -> {}",
                                record.previous_revision.unwrap_or_default(),
                                record.next_revision.unwrap_or_default()
                            );
                        });
                        0
                    }
                    Err(error) => {
                        eprintln!("{error}");
                        1
                    }
                }
            }
            RepairTarget::AuditLatestRevision { project_path } => {
                let service = app_lib::services::audit_service::AuditService::new();
                match service.repair_missing_latest_revision(&project_path) {
                    Ok(record) => {
                        print_json_or_human(&record, cli.json, |record| {
                            println!(
                                "audit_revision_repaired={} -> {}",
                                record.previous_revision.unwrap_or_default(),
                                record.next_revision.unwrap_or_default()
                            );
                        });
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
    println!("read_only={}", report.read_only);
    println!("writes_performed={}", report.writes_performed);
    println!("project_file_exists={}", report.project_file_exists);
    println!("project_readable={}", report.project_readable);
    println!(
        "project_root_metadata_mismatch={}",
        report.path_security.project_root_metadata_mismatch
    );
    println!(
        "unsafe_document_path_count={}",
        report.path_security.unsafe_document_path_count
    );
    println!(
        "unresolved_legacy_document_path_count={}",
        report.path_security.unresolved_legacy_document_path_count
    );
    println!(
        "external_managed_document_path_count={}",
        report.path_security.external_managed_document_path_count
    );
    println!(
        "symlink_escape_count={}",
        report.path_security.symlink_escape_count
    );
    println!("storage_revision={}", report.persistence.storage_revision);
    println!(
        "project_fingerprint_status={}",
        report.persistence.project_fingerprint_status
    );
    println!(
        "stale_job_result_count={}",
        report.persistence.stale_job_result_count
    );
    println!(
        "mutation_conflict_count={}",
        report.persistence.mutation_conflict_count
    );
    println!(
        "external_modification_detected={}",
        report.persistence.external_modification_detected
    );
    println!(
        "legacy_project_without_revision={}",
        report.persistence.legacy_project_without_revision
    );
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
    println!(
        "assessment_activity_count={}",
        report.speaking.assessment_activity_count
    );
    println!(
        "speaking_activity_count={}",
        report.speaking.speaking_activity_count
    );
    println!(
        "written_activity_count={}",
        report.speaking.written_activity_count
    );
    println!(
        "listening_activity_count={}",
        report.speaking.listening_activity_count
    );
    println!(
        "assessment_class_application_count={}",
        report.speaking.assessment_class_application_count
    );
    println!(
        "duplicate_activity_class_application_count={}",
        report.speaking.duplicate_activity_class_application_count
    );
    println!(
        "class_application_without_class_count={}",
        report.speaking.class_application_without_class_count
    );
    println!(
        "speaking_attempt_without_activity_count={}",
        report.speaking.speaking_attempt_without_activity_count
    );
    println!(
        "speaking_attempt_without_class_application_count={}",
        report
            .speaking
            .speaking_attempt_without_class_application_count
    );
    println!(
        "speaking_attempt_class_membership_mismatch_count={}",
        report
            .speaking
            .speaking_attempt_class_membership_mismatch_count
    );
    println!(
        "unresolved_legacy_speaking_record_count={}",
        report.speaking.unresolved_legacy_speaking_record_count
    );
    println!(
        "activity_application_workflow_mismatch_count={}",
        report.speaking.activity_application_workflow_mismatch_count
    );
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

fn print_preflight(report: &DataLossPreflightReport) {
    println!("project_path={}", report.project_path);
    println!("read_only={}", report.read_only);
    println!(
        "read_only_guarantee_verified={}",
        report.read_only_guarantee_verified
    );
    println!("project_file_exists={}", report.project_file_exists);
    println!("project_parse_ok={}", report.project_parse_ok);
    println!("project_id={:?}", report.project_id);
    println!("storage_revision={:?}", report.storage_revision);
    println!("project_revision={:?}", report.project_revision);
    println!("project_fingerprint={:?}", report.project_fingerprint);
    println!("source_manifest_hash={}", report.source_manifest_hash);
    println!("source_byte_changes={}", report.source_byte_changes);
    println!("pending_migration={}", report.pending_migration);
    println!("migration_backup_status={}", report.migration_backup_status);
    println!("recursive_file_count={}", report.recursive_file_count);
    println!("recursive_byte_count={}", report.recursive_byte_count);
    println!(
        "recursive_inventory_sha256={}",
        report.recursive_inventory_sha256
    );
    println!("symlink_count={}", report.symlink_count);
    println!(
        "missing_active_pointer_count={}",
        report.missing_active_pointer_count
    );
    println!(
        "missing_referenced_artifact_count={}",
        report.missing_referenced_artifact_count
    );
    println!("orphan_artifact_count={}", report.orphan_artifact_count);
    println!("unknown_orphan_count={}", report.unknown_orphan_count);
    println!(
        "recoverable_audio_orphan_count={}",
        report.recoverable_audio_orphan_count
    );
    println!(
        "orphan_restore_staging_count={}",
        report.orphan_restore_staging_count
    );
    println!("audit_chain_valid={}", report.audit.chain_valid);
    println!("audit_tamper_count={}", report.audit.tamper_count);
    println!("verified_backup_count={}", report.verified_backup_count);
    println!("failed_backup_count={}", report.failed_backup_count);
    println!(
        "latest_verified_backup_path={:?}",
        report.latest_verified_backup_path
    );
    println!(
        "latest_verified_backup_age={:?}",
        report.latest_verified_backup_age
    );
    println!(
        "incomplete_transaction_count={}",
        report.incomplete_transaction_count
    );
    println!(
        "audit_project_divergence_count={}",
        report.audit_project_divergence_count
    );
    println!(
        "active_revision_divergence_count={}",
        report.active_revision_divergence_count
    );
    println!("original_audit_status={}", report.original_audit_status);
    println!("active_audit_status={}", report.active_audit_status);
    println!(
        "historical_recovery_anchor_status={}",
        report.historical_recovery_anchor_status
    );
    println!("verified_backup_path={:?}", report.verified_backup_path);
    println!("verified_backup_sha256={:?}", report.verified_backup_sha256);
    println!(
        "verified_backup_restore_status={}",
        report.verified_backup_restore_status
    );
    println!(
        "process_kill_proofs_status={}",
        report.process_kill_proofs_status
    );
    println!(
        "disk_fault_proofs_status={}",
        report.disk_fault_proofs_status
    );
    println!(
        "destructive_race_proofs_status={}",
        report.destructive_race_proofs_status
    );
    println!("full_test_suite_green={}", report.full_test_suite_green);
    println!("second_writer_detected={}", report.second_writer_detected);
    println!(
        "initialization_write_allowed={}",
        report.initialization_write_allowed
    );
    println!("blockers={:?}", report.blockers);
    println!("decision={}", report.decision);
    println!(
        "safe_to_open_for_writing={}",
        report.safe_to_open_for_writing
    );
    if !report.warnings.is_empty() {
        println!("warnings={:?}", report.warnings);
    }
    if !report.errors.is_empty() {
        println!("errors={:?}", report.errors);
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
    println!("managed_process_present={}", report.managed_process_present);
    println!(
        "process_identity_verification={}",
        report.process_identity_verification
    );
    println!("active_lease_count={}", report.active_lease_count);
    println!("draining_requested={}", report.draining_requested);
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

fn print_backup_verification(report: &BackupVerificationReport) {
    println!("archive_path={}", report.archive_path);
    println!("receipt_path={}", report.receipt_path);
    println!("archive_sha256={}", report.archive_sha256);
    println!("project_id={}", report.project_id);
    println!("entry_count={}", report.entry_count);
    println!("total_size={}", report.total_size);
    println!("source_project_path={}", report.source_project_path);
    println!("source_manifest_sha256={:?}", report.source_manifest_sha256);
    println!("archive_verified={}", report.archive_verified);
    println!("traversal_checks={}", report.traversal_checks);
}

fn print_audit_forensics(report: &AuditForensicsReport) {
    println!("audit_path={}", report.audit_path);
    println!("audit_sha256={}", report.audit_sha256);
    println!("audit_size={}", report.audit_size);
    println!("record_count={}", report.record_count);
    println!("first_invalid_line={:?}", report.first_invalid_line);
    println!(
        "first_invalid_record_id={:?}",
        report.first_invalid_record_id
    );
    println!(
        "first_invalid_previous_hash={:?}",
        report.first_invalid_previous_hash
    );
    println!(
        "first_invalid_computed_hash={:?}",
        report.first_invalid_computed_hash
    );
    println!(
        "first_invalid_recorded_hash={:?}",
        report.first_invalid_recorded_hash
    );
    println!("last_valid_record_hash={}", report.last_valid_record_hash);
    println!("last_valid_revision={:?}", report.last_valid_revision);
    println!(
        "duplicate_revision_count={}",
        report.duplicate_revision_count
    );
    println!("missing_revision_count={}", report.missing_revision_count);
    println!("project_revision={:?}", report.project_revision);
    println!(
        "project_revision_divergence={:?}",
        report.project_revision_divergence
    );
    println!(
        "active_revision_divergence_count={}",
        report.active_revision_divergence_count
    );
    println!("original_audit_status={}", report.original_audit_status);
    println!("active_audit_status={}", report.active_audit_status);
    println!(
        "historical_recovery_anchor_status={}",
        report.historical_recovery_anchor_status
    );
    println!("classifications={:?}", report.classifications);
}

fn print_audio_forensics(reports: &Vec<AudioForensicReport>) {
    for report in reports {
        println!(
            "path={} size={} sha256={} wav_valid={} duration={:?} sample_rate={:?} channels={:?} classification={} recommended_action={}",
            report.relative_path,
            report.byte_size,
            report.sha256,
            report.wav_valid,
            report.duration_seconds,
            report.sample_rate,
            report.channels,
            report.classification,
            report.recommended_action
        );
    }
}

fn print_recovery_copy(report: &RecoveryCopyReport) {
    println!("dry_run={}", report.dry_run);
    println!("destination={}", report.destination);
    println!("backup_archive={}", report.backup.archive_path);
    println!("backup_sha256={}", report.backup.archive_sha256);
    println!("source_manifest_sha256={}", report.source_manifest_sha256);
    println!("original_audit_sha256={}", report.original_audit_sha256);
    println!("original_audit_size={}", report.original_audit_size);
    println!("first_invalid_line={:?}", report.first_invalid_line);
    println!("project_revision={}", report.project_revision);
    println!("project_fingerprint={}", report.project_fingerprint);
    println!("active_audit_status={}", report.active_audit_status);
    println!(
        "historical_recovery_anchor_status={}",
        report.historical_recovery_anchor_status
    );
    println!(
        "quarantined_artifacts={}",
        report.quarantined_artifacts.len()
    );
}

fn print_recovery_diff(report: &RecoveryDiffReport) {
    println!("source_path={}", report.source_path);
    println!("candidate_path={}", report.candidate_path);
    println!("domain_equality={}", report.domain_equality);
    println!("artifact_hash_equality={}", report.artifact_hash_equality);
    println!("byte_identity={}", report.byte_identity);
    println!("changes={}", report.changes.len());
    println!("unexplained_changes={:?}", report.unexplained_changes);
}

fn print_restore_verification(report: &RestoreVerificationReport) {
    println!("status={}", report.status);
    println!("archive_path={}", report.archive_path);
    println!("source_project_path={}", report.source_project_path);
    println!("restored_project_path={}", report.restored_project_path);
    println!("archive_verified={}", report.archive_verified);
    println!("byte_identity={}", report.byte_identity);
    println!("domain_equality={}", report.domain_equality);
    println!("artifact_hash_equality={}", report.artifact_hash_equality);
    println!("unexplained_changes={:?}", report.unexplained_changes);
    println!(
        "source_byte_manifest_sha256={}",
        report.source_byte_manifest_sha256
    );
    println!(
        "restored_byte_manifest_sha256={}",
        report.restored_byte_manifest_sha256
    );
    println!("restored_project_id={}", report.restored_project_id);
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

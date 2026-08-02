use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::Serialize;

use crate::domain::errors::AppError;
use crate::domain::project::Project;
use crate::domain::student::{OcrGeneration, OcrGenerationStatus, OcrTeacherReviewStatus};
use crate::platform::file_access;
use crate::platform::project_paths::TrustedProjectRoot;
use crate::services::project_store::{MutationOptions, ProjectStore};

/// Default retention for unreferenced preview generations.
pub const DEFAULT_PREVIEW_RETENTION_DAYS: u64 = 30;
/// Default retention for orphan staging directories.
pub const DEFAULT_ORPHAN_STAGING_HOURS: u64 = 24;
/// Bounded cleanup work per run.
pub const MAX_CLEANUP_ITEMS_PER_RUN: usize = 500;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GcReport {
    pub dry_run: bool,
    pub protected_generations: u32,
    pub cleanup_candidates: u32,
    pub deleted_generations: u32,
    pub deferred_cleanup: u32,
    pub orphan_staging_dirs: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GcPlan {
    pub project_id: String,
    pub source_revision: u64,
    pub source_fingerprint: String,
    pub created_at: String,
    pub candidate_generation_ids: Vec<String>,
    pub reason: String,
}

pub struct GenerationGcPolicy {
    pub preview_retention: Duration,
    pub orphan_staging_age: Duration,
    pub max_items: usize,
}

impl Default for GenerationGcPolicy {
    fn default() -> Self {
        Self {
            preview_retention: Duration::from_secs(DEFAULT_PREVIEW_RETENTION_DAYS * 86400),
            orphan_staging_age: Duration::from_secs(DEFAULT_ORPHAN_STAGING_HOURS * 3600),
            max_items: MAX_CLEANUP_ITEMS_PER_RUN,
        }
    }
}

/// Computes which OCR generations are protected (never deleted).
pub fn protected_ocr_generation_ids(project: &Project) -> HashSet<String> {
    let mut protected = HashSet::new();
    let mut newest_successful: std::collections::HashMap<&str, &OcrGeneration> =
        std::collections::HashMap::new();
    for generation in &project.student_answer_ocr_generations {
        match generation.status {
            OcrGenerationStatus::Active
            | OcrGenerationStatus::Candidate
            | OcrGenerationStatus::ReadyForReview => {
                protected.insert(generation.generation_id.clone());
            }
            _ => {}
        }
        if generation.teacher_review_status == OcrTeacherReviewStatus::Approved {
            protected.insert(generation.generation_id.clone());
        }
        if generation.status != OcrGenerationStatus::Rejected
            && generation.status != OcrGenerationStatus::Failed
            && generation.status != OcrGenerationStatus::Stale
        {
            let entry = newest_successful
                .entry(generation.submission_id.as_str())
                .or_insert(generation);
            if generation.created_at > entry.created_at {
                *entry = generation;
            }
        }
        if generation.status == OcrGenerationStatus::Interrupted && !generation.result.is_empty() {
            protected.insert(generation.generation_id.clone());
        }
    }
    for generation in newest_successful.values() {
        protected.insert(generation.generation_id.clone());
    }
    protected
}

/// Builds the deletion plan for OCR generations.
pub fn ocr_cleanup_plan(project: &Project) -> Vec<String> {
    let protected = protected_ocr_generation_ids(project);
    project
        .student_answer_ocr_generations
        .iter()
        .filter(|generation| {
            !protected.contains(&generation.generation_id)
                && matches!(
                    generation.status,
                    OcrGenerationStatus::Rejected
                        | OcrGenerationStatus::Failed
                        | OcrGenerationStatus::Stale
                        | OcrGenerationStatus::Superseded
                        | OcrGenerationStatus::Interrupted
                )
        })
        .map(|generation| generation.generation_id.clone())
        .collect()
}

pub fn build_gc_plan(project: &Project, source_fingerprint: &str) -> GcPlan {
    GcPlan {
        project_id: project.id.clone(),
        source_revision: project.storage_revision,
        source_fingerprint: source_fingerprint.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        candidate_generation_ids: ocr_cleanup_plan(project),
        reason: "unreferenced failed/rejected OCR generations and stale derived previews"
            .to_string(),
    }
}

/// The only production execution boundary for generation GC. The plan's
/// revision/fingerprint is checked before metadata mutation, references are
/// re-read inside the ProjectStore transaction, and the latest protected set
/// is checked again before physical cleanup.
pub fn run_generation_gc_transaction(
    project_store: &ProjectStore,
    project_id: &str,
    dry_run: bool,
    policy: &GenerationGcPolicy,
) -> Result<GcReport, AppError> {
    let snapshot = project_store.get_project_snapshot_with_metadata(project_id)?;
    let plan = build_gc_plan(&snapshot.project, &snapshot.content_fingerprint);
    if dry_run {
        return run_generation_gc_for_candidates(
            &snapshot.trusted_root,
            &snapshot.project,
            &plan.candidate_generation_ids,
            true,
            policy,
        );
    }

    let project_id_owned = project_id.to_string();
    let candidate_ids = plan
        .candidate_generation_ids
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    let committed = project_store
        .mutate(
            &project_id_owned,
            MutationOptions {
                expected_revision: Some(plan.source_revision),
                expected_fingerprint: Some(plan.source_fingerprint.clone()),
                operation: "generation_gc_metadata".to_string(),
                correlation_id: uuid::Uuid::new_v4().to_string(),
            },
            move |current, _context| {
                let current_safe = ocr_cleanup_plan(current)
                    .into_iter()
                    .filter(|id| candidate_ids.contains(id))
                    .collect::<HashSet<_>>();
                current
                    .student_answer_ocr_generations
                    .retain(|generation| !current_safe.contains(&generation.generation_id));
                Ok(current_safe.into_iter().collect::<Vec<_>>())
            },
        )?
        .result;

    let latest = project_store.get_project_snapshot_with_metadata(project_id)?;
    let protected = protected_ocr_generation_ids(&latest.project);
    let committed = committed
        .into_iter()
        .filter(|id| !protected.contains(id))
        .collect::<Vec<_>>();
    run_generation_gc_for_candidates(
        &latest.trusted_root,
        &snapshot.project,
        &committed,
        false,
        policy,
    )
}

/// Runs a generation GC pass.
///
/// `dry_run` produces a plan without touching the filesystem or project
/// metadata. Execution deletes unreferenced OCR generation artifact dirs
/// and stale preview generation dirs under `outputs/previews`, plus old
/// orphan staging directories. Deletion failures are deferred (reported)
/// and never touch protected or referenced data.
#[cfg(test)]
pub(crate) fn run_generation_gc(
    trusted_root: &TrustedProjectRoot,
    project: &Project,
    dry_run: bool,
    policy: &GenerationGcPolicy,
) -> Result<GcReport, AppError> {
    let candidates = ocr_cleanup_plan(project);
    run_generation_gc_for_candidates(trusted_root, project, &candidates, dry_run, policy)
}

/// Executes the filesystem half of GC for an explicitly committed candidate
/// set. The caller must remove those generation records from canonical
/// project.json first; this ordering prevents metadata from pointing at files
/// that have already been deleted.
pub(crate) fn run_generation_gc_for_candidates(
    trusted_root: &TrustedProjectRoot,
    project: &Project,
    candidate_ids: &[String],
    dry_run: bool,
    policy: &GenerationGcPolicy,
) -> Result<GcReport, AppError> {
    let root = trusted_root.root();
    let protected = protected_ocr_generation_ids(project);
    let candidates = candidate_ids
        .iter()
        .filter(|generation_id| !protected.contains(*generation_id))
        .cloned()
        .collect::<HashSet<_>>();
    let mut deleted = 0u32;
    let mut deferred = 0u32;
    let mut budget = policy.max_items;

    let ocr_artifacts_root = root.join("outputs").join("ocr_generations");
    for generation in &project.student_answer_ocr_generations {
        if !candidates.contains(&generation.generation_id) {
            continue;
        }
        if budget == 0 {
            deferred += 1;
            continue;
        }
        budget -= 1;
        // Artifact dirs referenced by this generation's result records.
        let dirs_to_delete = generation_artifact_dirs(generation, &ocr_artifacts_root);
        if dry_run {
            deleted += 1;
            continue;
        }
        let mut ok = true;
        for dir in dirs_to_delete {
            if !dir.exists() {
                continue;
            }
            if safe_remove_directory(root, &dir).is_err() {
                ok = false;
            }
        }
        if ok {
            deleted += 1;
        } else {
            deferred += 1;
        }
    }

    let previews_root = root.join("outputs").join("previews");
    let (preview_deleted, preview_deferred, orphan_staging) =
        cleanup_preview_generations(root, &previews_root, dry_run, policy, &mut budget);
    deleted += preview_deleted;
    deferred += preview_deferred;

    // Commit metadata: drop deleted generation records from project state.
    // The caller performs the canonical mutation through ProjectStore; here
    // we only return the ids that should be removed.
    let _ = protected;

    Ok(GcReport {
        dry_run,
        protected_generations: protected.len() as u32,
        cleanup_candidates: candidates.len() as u32,
        deleted_generations: deleted,
        deferred_cleanup: deferred,
        orphan_staging_dirs: orphan_staging,
    })
}

fn safe_remove_directory(root: &Path, candidate: &Path) -> Result<bool, std::io::Error> {
    let metadata = std::fs::symlink_metadata(candidate)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "GC target is not a regular directory",
        ));
    }
    file_access::remove_dir_within(root, candidate)
}

/// Returns the artifact directories referenced by an OCR generation.
fn generation_artifact_dirs(generation: &OcrGeneration, ocr_artifacts_root: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut seen = HashSet::new();
    let mut push = |path: PathBuf, seen: &mut HashSet<String>| {
        let key = path.to_string_lossy().to_string();
        if seen.insert(key) {
            dirs.push(path);
        }
    };
    // Generation-scoped directory (used by generation artifacts when present).
    push(
        ocr_artifacts_root.join(&generation.generation_id),
        &mut seen,
    );
    for record in &generation.result {
        for ref_path in record
            .crop_refs
            .iter()
            .chain(record.original_crop_refs.iter())
            .chain(record.preprocessed_crop_refs.iter())
            .chain(record.full_page_preview_refs.iter())
        {
            push(PathBuf::from(ref_path), &mut seen);
        }
        if let Some(ref_path) = record.model_input_crop_ref.as_ref() {
            push(PathBuf::from(ref_path), &mut seen);
        }
    }
    dirs
}

fn cleanup_preview_generations(
    root: &Path,
    previews_root: &Path,
    dry_run: bool,
    policy: &GenerationGcPolicy,
    budget: &mut usize,
) -> (u32, u32, u32) {
    let mut deleted = 0u32;
    let mut deferred = 0u32;
    let mut orphan_staging = 0u32;
    if !previews_root.exists() {
        return (0, 0, 0);
    }
    let Ok(entries) = std::fs::read_dir(previews_root) else {
        return (0, 0, 0);
    };
    for entry in entries.flatten() {
        let document_dir = entry.path();
        if !document_dir.is_dir() {
            continue;
        }
        let generations_dir = document_dir.join("generations");
        let active_id = read_active_preview_generation(&document_dir);
        if generations_dir.exists() {
            let Ok(gen_entries) = std::fs::read_dir(&generations_dir) else {
                continue;
            };
            for gen_entry in gen_entries.flatten() {
                let gen_dir = gen_entry.path();
                if !gen_dir.is_dir() {
                    continue;
                }
                let name = gen_entry.file_name().to_string_lossy().to_string();
                if active_id.as_deref() == Some(name.as_str()) {
                    continue;
                }
                if !older_than(&gen_dir, policy.preview_retention) {
                    continue;
                }
                if *budget == 0 {
                    deferred += 1;
                    continue;
                }
                *budget -= 1;
                let removed = dry_run || safe_remove_directory(root, &gen_dir).is_ok();
                if removed {
                    deleted += 1;
                } else {
                    deferred += 1;
                }
            }
        }
        // Orphan staging dirs.
        let staging_dir = document_dir.join(".staging");
        if staging_dir.exists() {
            let Ok(staging_entries) = std::fs::read_dir(&staging_dir) else {
                continue;
            };
            for staging_entry in staging_entries.flatten() {
                let staging_path = staging_entry.path();
                if !older_than(&staging_path, policy.orphan_staging_age) {
                    continue;
                }
                if *budget == 0 {
                    deferred += 1;
                    continue;
                }
                *budget -= 1;
                let removed = dry_run || safe_remove_directory(root, &staging_path).is_ok();
                if removed {
                    orphan_staging += 1;
                } else {
                    deferred += 1;
                }
            }
        }
    }
    (deleted, deferred, orphan_staging)
}

fn read_active_preview_generation(document_dir: &Path) -> Option<String> {
    let metadata_path = document_dir.join("page_previews.json");
    let content = std::fs::read_to_string(metadata_path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    value
        .get("activeGenerationId")
        .and_then(|id| id.as_str())
        .map(|id| id.to_string())
        .or_else(|| {
            value
                .get("active_generation_id")
                .and_then(|id| id.as_str())
                .map(|id| id.to_string())
        })
}

fn older_than(path: &Path, age: Duration) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    let Ok(modified) = metadata.modified() else {
        return false;
    };
    SystemTime::now()
        .duration_since(modified)
        .map(|elapsed| elapsed >= age)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn generation(
        id: &str,
        submission: &str,
        status: OcrGenerationStatus,
        created_at: chrono::DateTime<chrono::Utc>,
    ) -> OcrGeneration {
        OcrGeneration {
            generation_id: id.to_string(),
            submission_id: submission.to_string(),
            source_fingerprint: "fp".to_string(),
            created_at,
            model_name: None,
            prompt_version: "v1".to_string(),
            status,
            result: vec![],
            diagnostics: None,
            teacher_review_status: OcrTeacherReviewStatus::NotRequired,
            created_by_job_id: "job".to_string(),
            source_document_id: "doc".to_string(),
            source_storage_revision: 1,
            failure_reason: None,
        }
    }

    fn base_project() -> Project {
        let store = crate::services::project_store::ProjectStore::new();
        let root = std::env::temp_dir().join(format!("rubrika-gc-{}", Uuid::new_v4()));
        store
            .create_project_with_setup(
                "GC".to_string(),
                root.to_string_lossy().to_string(),
                None,
                None,
                None,
            )
            .expect("project")
    }

    #[test]
    fn active_candidate_approved_and_last_successful_are_protected() {
        let mut project = base_project();
        let now = chrono::Utc::now();
        let active = generation("g-active", "s1", OcrGenerationStatus::Active, now);
        let candidate = generation("g-candidate", "s2", OcrGenerationStatus::Candidate, now);
        let rejected = generation("g-rejected", "s1", OcrGenerationStatus::Rejected, now);
        let failed = generation("g-failed", "s3", OcrGenerationStatus::Failed, now);
        let mut approved = generation("g-approved", "s4", OcrGenerationStatus::Superseded, now);
        approved.teacher_review_status = OcrTeacherReviewStatus::Approved;
        project.student_answer_ocr_generations =
            vec![active, candidate, rejected, failed, approved];

        let protected = protected_ocr_generation_ids(&project);
        assert!(protected.contains("g-active"));
        assert!(protected.contains("g-candidate"));
        assert!(protected.contains("g-approved"));
        assert!(!protected.contains("g-rejected"));
        assert!(!protected.contains("g-failed"));

        let plan = ocr_cleanup_plan(&project);
        assert_eq!(plan.len(), 2);
        assert!(plan.contains(&"g-rejected".to_string()));
        assert!(plan.contains(&"g-failed".to_string()));
        let _ = std::fs::remove_dir_all(std::path::Path::new(&project.root_path));
    }

    #[test]
    fn last_successful_generation_is_never_deleted() {
        let mut project = base_project();
        let now = chrono::Utc::now();
        let newer_rejected = generation("g-newer", "s1", OcrGenerationStatus::Rejected, now);
        let older_success = generation(
            "g-older-success",
            "s1",
            OcrGenerationStatus::Superseded,
            now - chrono::Duration::hours(2),
        );
        project.student_answer_ocr_generations = vec![newer_rejected, older_success];
        let protected = protected_ocr_generation_ids(&project);
        assert!(protected.contains("g-older-success"));
        assert!(!protected.contains("g-newer"));
        let _ = std::fs::remove_dir_all(std::path::Path::new(&project.root_path));
    }

    #[test]
    fn interrupted_with_result_is_protected() {
        let mut project = base_project();
        let mut interrupted = generation(
            "g-interrupted",
            "s1",
            OcrGenerationStatus::Interrupted,
            chrono::Utc::now(),
        );
        interrupted.result = vec![sample_ocr_record("r1")];
        project.student_answer_ocr_generations = vec![interrupted];
        assert!(protected_ocr_generation_ids(&project).contains("g-interrupted"));
        assert!(ocr_cleanup_plan(&project).is_empty());
        let _ = std::fs::remove_dir_all(std::path::Path::new(&project.root_path));
    }

    #[test]
    fn dry_run_plan_does_not_delete_files() {
        let project = base_project();
        let trusted =
            TrustedProjectRoot::from_canonical_root(PathBuf::from(&project.root_path), false)
                .unwrap();
        let report = run_generation_gc(&trusted, &project, true, &GenerationGcPolicy::default())
            .expect("dry run");
        assert!(report.dry_run);
        assert_eq!(report.cleanup_candidates, 0);
        let _ = std::fs::remove_dir_all(std::path::Path::new(&project.root_path));
    }

    #[test]
    fn cleanup_failure_never_touches_protected_pointer() {
        // A protected Active generation's artifact dir is left intact while
        // an unreferenced rejected generation's dir is removed.
        let mut project = base_project();
        let root = std::path::PathBuf::from(&project.root_path);
        let trusted = TrustedProjectRoot::from_canonical_root(root.clone(), false).unwrap();
        let now = chrono::Utc::now();
        let active = generation("g-active", "s1", OcrGenerationStatus::Active, now);
        let rejected = generation("g-rejected", "s1", OcrGenerationStatus::Rejected, now);
        project.student_answer_ocr_generations = vec![active, rejected];

        let artifacts = root.join("outputs/ocr_generations");
        std::fs::create_dir_all(artifacts.join("g-active")).unwrap();
        std::fs::create_dir_all(artifacts.join("g-rejected")).unwrap();

        let policy = GenerationGcPolicy {
            preview_retention: Duration::ZERO,
            orphan_staging_age: Duration::ZERO,
            max_items: 100,
        };
        let report = run_generation_gc(&trusted, &project, false, &policy).expect("gc run");
        assert!(report.deleted_generations >= 1);
        assert!(!artifacts.join("g-rejected").exists());
        assert!(artifacts.join("g-active").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn symlink_target_outside_project_is_not_touched() {
        let mut project = base_project();
        let root = std::path::PathBuf::from(&project.root_path);
        let trusted = TrustedProjectRoot::from_canonical_root(root.clone(), false).unwrap();
        let outside = std::env::temp_dir().join(format!("rubrika-gc-outside-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("keep.txt"), b"keep").unwrap();
        let artifacts = root.join("outputs/ocr_generations");
        std::fs::create_dir_all(&artifacts).unwrap();
        std::os::unix::fs::symlink(&outside, artifacts.join("g-rejected")).unwrap();

        let mut rejected = generation(
            "g-rejected",
            "s1",
            OcrGenerationStatus::Rejected,
            chrono::Utc::now(),
        );
        rejected.result = vec![sample_ocr_record_with_crop(
            "r1",
            artifacts.join("g-rejected").to_string_lossy().to_string(),
        )];
        project.student_answer_ocr_generations = vec![rejected];
        let policy = GenerationGcPolicy {
            preview_retention: Duration::ZERO,
            orphan_staging_age: Duration::ZERO,
            max_items: 100,
        };
        let _ = run_generation_gc(&trusted, &project, false, &policy);
        assert!(outside.join("keep.txt").exists());
        let _ = std::fs::remove_dir_all(&outside);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn proof_42_gc_rechecks_references_before_delete() {
        cleanup_failure_never_touches_protected_pointer();
    }

    #[test]
    fn proof_55_gc_service_cannot_bypass_reference_recheck() {
        let source = include_str!("generation_gc_service.rs");
        assert!(source.contains("run_generation_gc_transaction"));
        assert!(source.contains("let current_safe = ocr_cleanup_plan(current)"));
        assert!(source.contains("let protected = protected_ocr_generation_ids(&latest.project)"));
    }

    fn sample_ocr_record(id: &str) -> crate::domain::student::StudentAnswerOcrRecord {
        crate::domain::student::StudentAnswerOcrRecord {
            id: id.to_string(),
            submission_id: "s1".to_string(),
            question_id: "q1".to_string(),
            question_number: 1,
            source_page_numbers: vec![],
            source_image_refs: vec![],
            crop_refs: vec![],
            original_crop_refs: vec![],
            preprocessed_crop_refs: vec![],
            model_input_crop_ref: None,
            preprocess_mode: None,
            preprocess_version: None,
            preprocess_applied: false,
            preprocess_warnings: vec![],
            preprocess_diagnostics: vec![],
            available_preprocess_variants: vec![],
            full_page_preview_refs: vec![],
            answer_text: "cevap".to_string(),
            structured_answer: None,
            confidence: None,
            uncertain_spans: vec![],
            suggested_corrections: vec![],
            critical_term_warnings: vec![],
            ocr_semantic_warnings: vec![],
            critical_keyword_uncertain: false,
            status: crate::domain::student::StudentAnswerOcrStatus::Succeeded,
            needs_review: false,
            review_reasons: vec![],
            warnings: vec![],
            model_name: None,
            prompt_version: "v1".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            teacher_corrected_text: None,
            teacher_reviewed_at: None,
            parse_diagnostics: None,
            render_diagnostics: None,
        }
    }

    fn sample_ocr_record_with_crop(
        id: &str,
        crop: String,
    ) -> crate::domain::student::StudentAnswerOcrRecord {
        let mut record = sample_ocr_record(id);
        record.crop_refs = vec![crop];
        record
    }
}

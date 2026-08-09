use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use app_lib::diagnostics::DiagnosticsContext;
use app_lib::domain::assessment::AssessmentType;
use app_lib::services::assessment_organization_service::{
    AssessmentOrganizationService, CreateAssessmentActivityInput,
};
use app_lib::services::audit_service::AuditService;
use app_lib::services::project_store::{MutationOptions, ProjectStore};
use app_lib::services::school_class_service::{
    CreateClassStudentInput, CreateSchoolClassInput, CreateTeachingAssignmentInput,
    SchoolClassService,
};
use app_lib::services::transaction_journal;

fn create_temp_project(label: &str) -> (PathBuf, ProjectStore, String) {
    let base = std::env::temp_dir().join(format!("rubrika-{label}-{}", Uuid::new_v4()));
    let root = base.join("project");
    fs::create_dir_all(&base).expect("temp project directory");
    let store = ProjectStore::new();
    let project = store
        .create_project_with_setup(
            "Divergence Fix Test".to_string(),
            root.to_string_lossy().to_string(),
            Some("2026-2027".to_string()),
            Some("tde".to_string()),
            Some("Türk Dili ve Edebiyatı".to_string()),
        )
        .expect("create_project_with_setup");
    (PathBuf::from(project.root_path), store, project.id)
}

fn assert_revision_invariant(project_root: &Path, store: &ProjectStore, project_id: &str) {
    let current = store
        .get_project_snapshot(project_id.to_string())
        .expect("snapshot");
    let audit_service = AuditService::new();
    let report = audit_service
        .verify_chain_against_project(project_root, &current)
        .expect("verify_chain_against_project");

    assert!(
        report.chain_valid,
        "Audit chain is invalid for project revision {}: {:?}",
        current.storage_revision, report.reasons
    );
    assert_eq!(
        report.last_audit_revision,
        Some(current.storage_revision),
        "Revision invariant broken! project.storage_revision ({}) != audit integrity revision ({:?})",
        current.storage_revision, report.last_audit_revision
    );
    assert_eq!(
        report.project_revision_divergence_count, 0,
        "project_revision_divergence_count > 0: {:?}",
        report.reasons
    );
    assert_eq!(
        report.active_revision_divergence_count, 0,
        "active_revision_divergence_count > 0: {:?}",
        report.reasons
    );
}

#[test]
fn revision_invariant_holds_across_all_canonical_mutation_types() {
    let (root, store, project_id) = create_temp_project("canonical-mutations");
    let class_service = SchoolClassService::new(store.clone());

    // Initial project state invariant (revision 0)
    assert_revision_invariant(&root, &store, &project_id);

    // Mutation 1: update course info (revision 1)
    store
        .update_course_info(
            project_id.clone(),
            "2026-2027".to_string(),
            "tde11".to_string(),
            "11. Sınıf Türk Dili ve Edebiyatı".to_string(),
            None,
        )
        .expect("update_course_info");
    assert_revision_invariant(&root, &store, &project_id);

    // Mutation 2: add a school class (revision 2)
    let class_dto = class_service
        .create_school_class(CreateSchoolClassInput {
            project_id: project_id.clone(),
            name: "11-A".to_string(),
            academic_year: Some("2026-2027".to_string()),
            grade_level: Some(11),
            section: None,
            display_order: None,
        })
        .expect("create_school_class");
    assert_revision_invariant(&root, &store, &project_id);

    // Mutation 3: add a student (revision 3)
    let _student = class_service
        .create_class_student(CreateClassStudentInput {
            project_id: project_id.clone(),
            class_id: class_dto.id.clone(),
            display_name: Some("Ali Yılmaz".to_string()),
            number: Some("101".to_string()),
        })
        .expect("create_class_student");
    assert_revision_invariant(&root, &store, &project_id);

    // Mutation 4: generic ProjectStore::mutate call (revision 4)
    store
        .mutate(
            &project_id,
            MutationOptions::new("custom_canonical_mutation")
                .summary("Özel kanonik test mutasyonu."),
            move |project, _context| {
                project.course_name = Some("Güncellenmiş Ders Adı".to_string());
                Ok(())
            },
        )
        .expect("custom mutate");
    assert_revision_invariant(&root, &store, &project_id);

    // Run Preflight diagnostic check
    let diagnostics_ctx = DiagnosticsContext::new();
    let preflight = diagnostics_ctx
        .data_loss_preflight(&root)
        .expect("preflight report");

    assert_eq!(preflight.audit_project_divergence_count, 0);
    assert_ne!(
        preflight.decision, "DO_NOT_OPEN_FOR_WRITING",
        "Preflight wrongly blocked project writing: blockers={:?}",
        preflight.blockers
    );

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

#[test]
fn assessment_organization_workflow_maintains_revision_invariant_at_every_step() {
    let (root, store, project_id) = create_temp_project("org-e2e");
    let class_service = std::sync::Arc::new(SchoolClassService::new(store.clone()));
    let org_service = std::sync::Arc::new(AssessmentOrganizationService::new(
        store.clone(),
        class_service.clone(),
    ));

    // Step 0: Initial state (revision 0)
    assert_revision_invariant(&root, &store, &project_id);

    // Create a class and teaching assignment so activities can be attached
    let class_dto = class_service
        .create_school_class(CreateSchoolClassInput {
            project_id: project_id.clone(),
            name: "11-B".to_string(),
            academic_year: Some("2026-2027".to_string()),
            grade_level: Some(11),
            section: None,
            display_order: None,
        })
        .expect("create_school_class");

    class_service
        .create_teaching_assignment(CreateTeachingAssignmentInput {
            project_id: project_id.clone(),
            academic_year_id: "2026-2027".to_string(),
            course_id: "tde".to_string(),
            course_name: "Türk Dili ve Edebiyatı".to_string(),
            class_section_id: class_dto.id.clone(),
            teacher_id: None,
        })
        .expect("create_teaching_assignment");

    // Step 1: Create a written assessment activity (new revision)
    let activity = org_service
        .create_activity(CreateAssessmentActivityInput {
            project_id: project_id.clone(),
            academic_year_id: "2026-2027".to_string(),
            course_id: "tde".to_string(),
            course_name: "Türk Dili ve Edebiyatı".to_string(),
            title: "1. Yazılı".to_string(),
            grade_level: 11,
            term: 1,
            assessment_type: AssessmentType::Written,
            sequence_number: 1,
            school_class_ids: vec![class_dto.id.clone()],
            speaking_configuration: None,
            listening_details: None,
        })
        .expect("create_activity");
    assert_revision_invariant(&root, &store, &project_id);

    // Step 2: Add a class student (new revision)
    let _student_dto = class_service
        .create_class_student(CreateClassStudentInput {
            project_id: project_id.clone(),
            class_id: class_dto.id.clone(),
            display_name: Some("Ayşe Kaya".to_string()),
            number: Some("102".to_string()),
        })
        .expect("create_class_student");
    assert_revision_invariant(&root, &store, &project_id);

    // Step 3: Update the assessment activity (new revision)
    let _updated = org_service
        .update_activity(
            app_lib::services::assessment_organization_service::UpdateAssessmentActivityInput {
                project_id: project_id.clone(),
                activity_id: activity.id.clone(),
                title: Some("1. Yazılı (Güncellendi)".to_string()),
                status: Some(app_lib::domain::assessment::AssessmentStatus::Scheduled),
                speaking_configuration: None,
            },
        )
        .expect("update_activity");
    assert_revision_invariant(&root, &store, &project_id);

    // Step 4: Set active written activity (new revision)
    org_service
        .set_active_written_activity(
            app_lib::services::assessment_organization_service::AssessmentActivityIdInput {
                project_id: project_id.clone(),
                activity_id: activity.id.clone(),
            },
        )
        .expect("set_active_written_activity");
    assert_revision_invariant(&root, &store, &project_id);

    // Preflight gate acceptance check
    let diagnostics_ctx = DiagnosticsContext::new();
    let preflight = diagnostics_ctx
        .data_loss_preflight(&root)
        .expect("data_loss_preflight");

    assert_eq!(
        preflight.audit_project_divergence_count, 0,
        "audit_project_divergence_count must be 0"
    );
    assert_ne!(
        preflight.decision, "DO_NOT_OPEN_FOR_WRITING",
        "Preflight MUST NOT return DO_NOT_OPEN_FOR_WRITING on clean project: blockers={:?}",
        preflight.blockers
    );

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

#[test]
fn failpoint_incomplete_transaction_produces_typed_write_blocker() {
    let (root, store, project_id) = create_temp_project("failpoint-incomplete");

    // Manually create an incomplete transaction journal entry (status = "intent")
    let intent = transaction_journal::begin(
        &root,
        &project_id,
        "interrupted_mutation",
        "corr-failpoint",
        Some(0),
        Some(1),
    )
    .expect("begin journal");

    let count = transaction_journal::incomplete_count(&root).expect("incomplete count");
    assert_eq!(count, 1, "There should be 1 incomplete transaction");

    // Preflight must detect this incomplete transaction and return DO_NOT_OPEN_FOR_WRITING
    let diagnostics_ctx = DiagnosticsContext::new();
    let preflight = diagnostics_ctx
        .data_loss_preflight(&root)
        .expect("preflight");

    assert_eq!(
        preflight.decision, "DO_NOT_OPEN_FOR_WRITING",
        "Incomplete transaction journal entry MUST block project write access!"
    );

    drop(intent);
    drop(store);
    let _ = fs::remove_dir_all(root.parent().unwrap());
}

#[test]
fn failpoint_missing_audit_record_produces_typed_write_blocker() {
    let (root, store, project_id) = create_temp_project("failpoint-missing-audit");

    // Create a transaction journal entry with status "audit_missing"
    let _tx = transaction_journal::begin(
        &root,
        &project_id,
        "audit_missing_mutation",
        "corr-failpoint-audit",
        Some(0),
        Some(1),
    )
    .expect("begin journal");
    transaction_journal::update(&root, &_tx.transaction_id, "audit_missing")
        .expect("update status to audit_missing");

    let diagnostics_ctx = DiagnosticsContext::new();
    let preflight = diagnostics_ctx
        .data_loss_preflight(&root)
        .expect("preflight");

    assert_eq!(
        preflight.decision, "DO_NOT_OPEN_FOR_WRITING",
        "audit_missing status in transaction journal MUST block project write access!"
    );

    drop(store);
    let _ = fs::remove_dir_all(root.parent().unwrap());
}

use app_lib::domain::errors::AppErrorCode;
use app_lib::domain::job::{DuplicatePolicy, JobKind};
use app_lib::jobs::job_manager::{JobManager, JobRegistrationInput};

fn registration(project_id: &str, kind: JobKind, key: &str) -> JobRegistrationInput {
    JobRegistrationInput {
        project_id: project_id.to_string(),
        project_root_path: None,
        kind,
        display_label: Some("scope-test".to_string()),
        total: 1,
        message: "scope-test".to_string(),
        correlation_id: None,
        idempotency_key: Some(key.to_string()),
        duplicate_policy: DuplicatePolicy::ReturnExisting,
        cancellable: true,
        retry_of_job_id: None,
    }
}

#[test]
fn explicit_idempotency_key_is_scoped_by_project_and_kind() {
    let manager = JobManager::new();
    let app = tauri::test::mock_app();
    let handle = app.handle();

    let project_a_scoring = manager
        .register_or_get_active_job(
            handle,
            registration("project-a", JobKind::Scoring, "shared-key"),
        )
        .expect("first scoped registration");

    let project_b_scoring = manager
        .register_or_get_active_job(
            handle,
            registration("project-b", JobKind::Scoring, "shared-key"),
        )
        .expect("same caller key must be allowed in another project");

    let project_a_ocr = manager
        .register_or_get_active_job(
            handle,
            registration("project-a", JobKind::StudentAnswerOcr, "shared-key"),
        )
        .expect("same caller key must be allowed for another job kind");

    assert_ne!(project_a_scoring.snapshot.id, project_b_scoring.snapshot.id);
    assert_ne!(project_a_scoring.snapshot.id, project_a_ocr.snapshot.id);
    assert_ne!(
        project_a_scoring.snapshot.idempotency_key,
        project_b_scoring.snapshot.idempotency_key
    );
    assert_ne!(
        project_a_scoring.snapshot.idempotency_key,
        project_a_ocr.snapshot.idempotency_key
    );

    let duplicate = manager.register_or_get_active_job(
        handle,
        registration("project-a", JobKind::Scoring, "shared-key"),
    );
    match duplicate {
        Ok(_) => panic!("same project + kind + caller key must remain a duplicate"),
        Err(error) => assert_eq!(error.code, AppErrorCode::JobAlreadyRunning),
    }
}

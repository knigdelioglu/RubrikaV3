use std::sync::Arc;

use app_lib::domain::assessment::{
    AssessmentActivity, AssessmentClassApplication, AssessmentStatus, AssessmentType,
    ClassApplicationStatus, SpeakingConfigurationSnapshot,
};
use app_lib::domain::school_class::{SchoolClass, SchoolClassStatus};
use app_lib::jobs::job_manager::JobManager;
use app_lib::services::llama_server_gateway::LlamaServerGateway;
use app_lib::services::model_config_service::ModelConfigService;
use app_lib::services::model_process_manager::ModelProcessManager;
use app_lib::services::model_runtime_service::ModelRuntimeService;
use app_lib::services::project_store::{MutationOptions, ProjectStore};
use app_lib::services::speaking_exam_service::SpeakingExamService;
use speakoflow_engine::SpeakoflowEngine;
use uuid::Uuid;

/// proof_23: speaking session start is backend-persisted.
///
/// UI start command → ProjectStore revision increases → speaking exam is
/// InProgress/persisted → reload still present → a second start does not
/// create a duplicate session.
#[test]
fn speaking_start_is_backend_persisted_and_duplicate_safe() {
    let base = std::env::temp_dir().join(format!("rubrika-speaking-persist-{}", Uuid::new_v4()));
    let root = base.join("project");
    std::fs::create_dir_all(&base).unwrap();
    std::env::set_var(
        "RUBRIKA_V3_MODEL_CONFIG_PATH",
        base.join("model_profiles.json"),
    );

    let store = ProjectStore::new();
    let project = store
        .create_project_with_setup(
            "Konuşma".to_string(),
            root.to_string_lossy().to_string(),
            None,
            None,
            None,
        )
        .expect("project");
    let project_id = project.id.clone();

    let school_class = SchoolClass {
        id: "class-1".to_string(),
        name: "9-A".to_string(),
        display_name: "9-A".to_string(),
        normalized_name: "9-a".to_string(),
        academic_year: None,
        academic_year_id: None,
        grade_level: None,
        section: None,
        display_order: 1,
        status: SchoolClassStatus::Active,
        created_at: String::new(),
        updated_at: String::new(),
    };
    let activity = AssessmentActivity {
        id: "activity-1".to_string(),
        academic_year_id: "2025".to_string(),
        course_id: "tur".to_string(),
        course_name: "Türkçe".to_string(),
        title: "Konuşma Sınavı 1".to_string(),
        grade_level: 9,
        term: 1,
        assessment_type: AssessmentType::Speaking,
        workflow_family: app_lib::domain::assessment::WorkflowFamily::Speaking,
        sequence_number: 1,
        status: AssessmentStatus::Active,
        common_document_ids: vec![],
        listening_details: None,
        speaking_configuration: Some(SpeakingConfigurationSnapshot {
            speaking_type: "prepared".to_string(),
            task_text: "Bir gününüzü anlatın.".to_string(),
            target_duration_seconds: 180,
            min_duration_seconds: 120,
            max_duration_seconds: 240,
            rubric_version: "v1".to_string(),
            scoring_policy_version: "v1".to_string(),
            cleanup_prompt_version: "v1".to_string(),
            evaluation_prompt_version: "v1".to_string(),
            frozen_model_file_hash: None,
            rubric_snapshot: serde_json::json!({}),
        }),
        performance_details: None,
        class_applications: vec![AssessmentClassApplication {
            id: "app-1".to_string(),
            activity_id: "activity-1".to_string(),
            school_class_id: "class-1".to_string(),
            scheduled_at: None,
            application_date: None,
            status: ClassApplicationStatus::Active,
            notes: None,
            document_ids: vec![],
            student_scope_ids: vec![],
            speaking_attempts: vec![],
            performance_assessments: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        }],
        created_at: String::new(),
        updated_at: String::new(),
    };

    store
        .mutate(
            &project_id,
            MutationOptions::new("fixture_activity"),
            move |current, _| {
                current.school_classes.push(school_class);
                current.assessment_activities.push(activity);
                Ok(())
            },
        )
        .expect("fixture mutation");

    let config = ModelConfigService::new();
    let gateway = Arc::new(LlamaServerGateway::default());
    let process_manager = ModelProcessManager::new_with_state_path(
        config.clone(),
        gateway.clone(),
        base.join("model_state.json"),
    );
    let runtime = ModelRuntimeService::new(config, process_manager);
    let job_manager = Arc::new(JobManager::new());
    let engine = Arc::new(SpeakoflowEngine::new());
    let service = SpeakingExamService::new(store.clone(), gateway, runtime, job_manager, engine);

    let before = store.get_project_snapshot(project_id.clone()).unwrap();
    let revision_before = before.storage_revision;

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let output = runtime
        .block_on(service.start(
            &project_id,
            "Konuşma Sınavı 1",
            vec!["app-1".to_string()],
            Some("activity-1".to_string()),
            "prepared",
            "Bir gününüzü anlatın.",
            180,
            120,
            240,
            None,
            None,
            None,
        ))
        .expect("backend start");
    assert!(output.started);
    let exam_id = output.exam_id.clone();

    let after = store.get_project_snapshot(project_id.clone()).unwrap();
    assert!(
        after.storage_revision > revision_before,
        "start must persist through ProjectStore"
    );
    let exams = after.speaking_exams.clone();
    assert_eq!(exams.len(), 1, "exactly one speaking exam");
    assert_eq!(exams[0].id, exam_id);
    assert_eq!(
        exams[0].assessment_activity_id.as_deref(),
        Some("activity-1")
    );

    // Reload from disk: session survives refresh/restart. The project is
    // still locked by the first store, so persistence is verified from the
    // canonical project.json bytes (the second writer is blocked on purpose).
    let project_json: app_lib::domain::project::Project =
        serde_json::from_str(&std::fs::read_to_string(root.join("project.json")).unwrap()).unwrap();
    assert_eq!(project_json.speaking_exams.len(), 1);
    assert_eq!(project_json.speaking_exams[0].id, exam_id);

    // Second start must not create a duplicate active session.
    let output_again = runtime
        .block_on(service.start(
            &project_id,
            "Konuşma Sınavı 1",
            vec!["app-1".to_string()],
            Some("activity-1".to_string()),
            "prepared",
            "Bir gününüzü anlatın.",
            180,
            120,
            240,
            Some(exam_id.clone()),
            None,
            None,
        ))
        .expect("second start");
    assert_eq!(output_again.exam_id, exam_id);
    let final_state = store.get_project_snapshot(project_id).unwrap();
    assert_eq!(final_state.speaking_exams.len(), 1);

    // The OS lease releases when the store drops; a read-only reload can
    // inspect the canonical bytes without silently running migration.
    drop(service);
    drop(store);
    let reopened = ProjectStore::open_project_at_path(&root).expect("read-only reload");
    assert_eq!(reopened.speaking_exams.len(), 1);

    let _ = std::fs::remove_dir_all(&base);
}

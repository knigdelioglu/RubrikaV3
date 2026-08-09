use std::fs;
use std::path::PathBuf;

use app_lib::services::audit_service::AuditService;
use app_lib::services::project_store::ProjectStore;
use uuid::Uuid;

#[test]
fn project_creation_can_complete_its_transactional_audit() {
    let root = std::env::temp_dir().join(format!("rubrika-create-regression-{}", Uuid::new_v4()));
    let store = ProjectStore::new();
    let project = store
        .create_project_with_setup(
            "Oluşturma regresyonu".to_string(),
            root.to_string_lossy().to_string(),
            Some("2026-2027".to_string()),
            Some("tde".to_string()),
            Some("Türk Dili ve Edebiyatı".to_string()),
        )
        .expect("project creation should succeed");

    let project_root = PathBuf::from(&project.root_path);
    let audit = AuditService::new();
    let report = audit
        .verify_chain_against_project(&project_root, &project)
        .expect("audit chain should be readable");
    assert!(
        report.chain_valid,
        "audit chain is invalid: {:?}",
        report.reasons
    );
    assert!(project_root.join("logs/transactions").is_dir());

    fs::remove_dir_all(&project_root).expect("test project cleanup");
}

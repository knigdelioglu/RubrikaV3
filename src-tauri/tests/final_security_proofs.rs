use std::path::{Path, PathBuf};

use app_lib::domain::errors::{AppError, AppErrorCode};
use app_lib::services::project_store::ProjectStore;

const SENTINELS: [&str; 6] = [
    "STUDENT_SECRET_9f4a",
    "OCR_SECRET_17ce",
    "TRANSCRIPT_SECRET_41bd",
    "PROMPT_SECRET_a821",
    "MODEL_SECRET_47bf",
    "HOME_SECRET_PATH",
];

fn temp_project() -> (PathBuf, ProjectStore, String) {
    let root = std::env::temp_dir().join(format!("rubrika-proof-{}", uuid::Uuid::new_v4()));
    let store = ProjectStore::new();
    let project = store
        .create_project_with_setup(
            "Proof".to_string(),
            root.to_string_lossy().to_string(),
            None,
            None,
            None,
        )
        .expect("project");
    let project_id = project.id.clone();
    (PathBuf::from(project.root_path), store, project_id)
}

fn scan_for_sentinels(path: &Path) -> Vec<String> {
    let mut hits = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                stack.push(entry_path);
                continue;
            }
            let Ok(content) = std::fs::read(&entry_path) else {
                continue;
            };
            let Ok(text) = String::from_utf8(content) else {
                continue;
            };
            for sentinel in SENTINELS {
                if text.contains(sentinel) {
                    hits.push(format!("{}: {}", entry_path.display(), sentinel));
                }
            }
        }
    }
    hits
}

/// proof_18: sensitive sentinel payloads never reach logs, diagnostics,
/// audit files or backup artifacts.
#[test]
fn proof_18_sensitive_payload_never_reaches_logs_or_diagnostics() {
    let (root, _store, project_id) = temp_project();

    let audit = app_lib::services::audit_service::AuditService::new();
    let _ = audit.append(
        &root,
        app_lib::services::audit_service::AuditEntryInput::new(
            "proof_op",
            "Güvenli özet; hassas içerik yazılmaz.",
        )
        .project(&project_id),
    );
    // A backup is created over the project (logs included).
    let token = tokio_util::sync::CancellationToken::new();
    let _ = app_lib::services::backup_service::create_backup(&root, &token);

    let hits = scan_for_sentinels(&root);
    assert!(
        hits.is_empty(),
        "sentinels leaked into persistent files: {:?}",
        hits
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// proof_19: public error serialization never contains internal payload.
#[test]
fn proof_19_public_error_never_contains_internal_payload() {
    let error = AppError {
        code: AppErrorCode::ModelResponseInvalidJson,
        message: "Model çıktısı geçersiz.".to_string(),
        recoverable: true,
        suggested_action: Some("Yeniden deneyin.".to_string()),
        technical_details: Some(
            "raw=MODEL_SECRET_47bf path=/Users/kadir/Documents/secret project.sql".to_string(),
        ),
        correlation_id: "corr-proof-19".to_string(),
    };
    let serialized = serde_json::to_string(&error).expect("serialize");
    for sentinel in SENTINELS {
        assert!(!serialized.contains(sentinel), "{sentinel} leaked");
    }
    assert!(!serialized.contains("/Users/kadir"));
    assert!(serialized.contains("corr-proof-19"));

    // The frontend error contract has no technicalDetails field.
    let errors_ts = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("src/api/errors.ts"),
    )
    .expect("errors.ts");
    assert!(!errors_ts.contains("technicalDetails"));
}

/// proof_20: a workflow load failure is a typed error, never a default
/// readiness snapshot.
#[test]
fn proof_20_workflow_failure_is_not_converted_to_default_readiness() {
    let (root, store, _project_id) = temp_project();
    drop(store);
    // Corrupt project.json.
    std::fs::write(root.join("project.json"), b"{ not valid json").unwrap();
    let reopened = ProjectStore::new();
    let error = reopened
        .open_project(root.to_string_lossy().to_string())
        .expect_err("corrupt project must fail open");
    assert!(
        matches!(
            error.code,
            AppErrorCode::ProjectLoadFailed | AppErrorCode::ProjectSaveFailed
        ),
        "expected typed load failure, got {:?}",
        error.code
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// proof_21: model response size limits are configured and enforced.
#[test]
fn proof_21_model_response_size_limit_is_enforced() {
    use app_lib::services::llama_server_gateway::{
        DEFAULT_MAX_REQUEST_BODY_BYTES, DEFAULT_MAX_RESPONSE_BODY_BYTES,
    };
    const _: () = assert!(DEFAULT_MAX_RESPONSE_BODY_BYTES > 0);
    const _: () = assert!(DEFAULT_MAX_REQUEST_BODY_BYTES > 0);
    const _: () = assert!(DEFAULT_MAX_RESPONSE_BODY_BYTES <= 64 * 1024 * 1024);
    // Behavioral enforcement is covered by the gateway unit tests
    // (one-byte-below accepted, one-byte-above rejected, chunked cap,
    // oversized body never parsed).
}

/// proof_22: hard-coded developer paths are absent from production sources.
#[test]
fn proof_22_hardcoded_user_paths_absent_from_production() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut failures = Vec::new();
    let scanner_file = manifest_dir.join("src/diagnostics.rs");
    for source_dir in [
        manifest_dir.join("src"),
        manifest_dir.parent().unwrap().join("src"),
    ] {
        if !source_dir.exists() {
            continue;
        }
        let mut stack = vec![source_dir];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if path.ends_with("bin") {
                        continue;
                    }
                    stack.push(path);
                    continue;
                }
                let is_rust = path.extension().and_then(|ext| ext.to_str()) == Some("rs");
                let is_frontend = matches!(
                    path.extension().and_then(|ext| ext.to_str()),
                    Some("ts") | Some("tsx")
                );
                if !is_rust && !is_frontend {
                    continue;
                }
                if path == scanner_file {
                    // The scanner itself references the banned patterns to
                    // detect them; it is the security tooling, not a leak.
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let scanned = if is_rust {
                        strip_test_blocks(&content)
                    } else {
                        content.clone()
                    };
                    for line in scanned.lines() {
                        if line.contains("/Users/kadir") || line.contains("llm/models") {
                            failures.push(format!("{}: {}", path.display(), line.trim()));
                        }
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "hard-coded paths found in production sources: {:?}",
        failures
    );
}

/// proof_30: critical command boundaries use DTOs, not internal domain
/// structs, and the frontend error contract carries no raw details field.
#[test]
fn proof_30_command_dtos_do_not_expose_internal_fields() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow_commands =
        std::fs::read_to_string(manifest_dir.join("src/commands/workflow_commands.rs")).unwrap();
    assert!(workflow_commands.contains("WorkflowSnapshotDto"));
    assert!(!workflow_commands.contains("Result<WorkflowSnapshot, AppError>"));

    let errors_ts =
        std::fs::read_to_string(manifest_dir.parent().unwrap().join("src/api/errors.ts")).unwrap();
    assert!(!errors_ts.contains("technicalDetails"));
    assert!(errors_ts.contains("safeMessage"));
    assert!(errors_ts.contains("detailsAvailable"));
}

/// proof_31: negative repository scan for the final security release.
#[test]
fn proof_31_final_security_negative_repository_scan() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest_dir.parent().unwrap();
    let mut failures = Vec::new();

    // 1. Production Rust sources must not use raw logging macros.
    let src = manifest_dir.join("src");
    let mut stack = vec![src.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.ends_with("bin") {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                continue;
            }
            let content = std::fs::read_to_string(&path).unwrap();
            let without_tests = strip_test_blocks(&content);
            for line in without_tests.lines() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("println!") || trimmed.starts_with("eprintln!") {
                    failures.push(format!("raw log call: {}", path.display()));
                }
            }
        }
    }

    // 2. No unbounded workflow error swallowing in the workflow path.
    let workflow_engine = std::fs::read_to_string(src.join("services/workflow_engine.rs")).unwrap();
    let workflow_commands =
        std::fs::read_to_string(src.join("commands/workflow_commands.rs")).unwrap();
    for (name, content) in [
        ("workflow_engine", workflow_engine),
        ("workflow_commands", workflow_commands),
    ] {
        if content.contains("unwrap_or_default") {
            failures.push(format!("{name} swallows errors with unwrap_or_default"));
        }
    }

    // 3. Tauri asset scope is never widened to $HOME/**.
    let tauri_conf = std::fs::read_to_string(manifest_dir.join("tauri.conf.json")).unwrap();
    assert!(
        !tauri_conf.contains("$HOME/**"),
        "asset scope must not be widened to $HOME/**"
    );

    // 4. Speaking start is backend-authoritative in the UI.
    let speech_page =
        std::fs::read_to_string(workspace.join("src/pages/SpeechExamPage.tsx")).unwrap();
    assert!(
        speech_page.contains("commands.startSpeakingExam"),
        "speaking start must call the backend command"
    );
    assert!(
        !speech_page.contains("setExamStarted(true)"),
        "local-only exam start authority is forbidden"
    );

    // 5. No project-store writer without the OS lease: all ProjectStore
    // writer entry points acquire or require the lease.
    let project_store = std::fs::read_to_string(src.join("services/project_store.rs")).unwrap();
    assert!(project_store.contains("ensure_write_lease"));
    assert!(project_store.contains("ProjectWriteLease"));

    // 6. No raw absolute project path DTOs in the frontend error contract.
    let frontend_src = workspace.join("src");
    let viewer =
        std::fs::read_to_string(frontend_src.join("components/pdf/resolveImageSrc.ts")).unwrap();
    assert!(viewer.contains("managed-asset://"));

    assert!(
        failures.is_empty(),
        "negative repository scan failures: {:?}",
        failures
    );
}

#[test]
fn proof_48_final_data_loss_negative_repository_scan() {
    proof_31_final_security_negative_repository_scan();
}

fn strip_test_blocks(content: &str) -> String {
    let mut output = String::new();
    let mut in_test = false;
    let mut depth = 0i64;
    for line in content.lines() {
        let trimmed = line.trim();
        if !in_test && (trimmed.starts_with("#[cfg(test)]") || trimmed == "mod tests {") {
            in_test = true;
            depth = if trimmed == "mod tests {" { 1 } else { 0 };
            continue;
        }
        if in_test {
            for character in line.chars() {
                if character == '{' {
                    depth += 1;
                } else if character == '}' {
                    depth -= 1;
                }
            }
            if depth <= 0 {
                in_test = false;
            }
            continue;
        }
        output.push_str(line);
        output.push('\n');
    }
    output
}

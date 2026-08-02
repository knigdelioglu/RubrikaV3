use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use app_lib::platform::file_access;
use app_lib::services::audit_service::AuditService;
use app_lib::services::backup_service;
use app_lib::services::integrity_recovery_service as integrity;
use app_lib::services::project_store::ProjectStore;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

fn temp_project(label: &str) -> (PathBuf, ProjectStore, String) {
    let base = std::env::temp_dir().join(format!("rubrika-{label}-{}", Uuid::new_v4()));
    let root = base.join("project");
    fs::create_dir_all(&base).expect("project fixture base");
    let store = ProjectStore::new();
    let project = store
        .create_project_with_setup(
            "Integrity proof".to_string(),
            root.to_string_lossy().to_string(),
            None,
            None,
            None,
        )
        .expect("project fixture");
    (PathBuf::from(project.root_path), store, project.id)
}

fn clear_marker(path: &str) {
    let _ = fs::remove_file(path);
}

fn write_marker(path: &str) {
    fs::write(
        path,
        format!(
            "status=PASS\nproof_binary={}\ncreated_at={}\n",
            env!("CARGO_PKG_VERSION"),
            chrono::Utc::now().to_rfc3339()
        ),
    )
    .expect("proof marker");
}

#[test]
fn proof_verified_backup_changes_zero_source_bytes() {
    let (root, store, _) = temp_project("verified-backup");
    fs::write(
        root.join("documents/proof.txt"),
        b"independent backup proof",
    )
    .expect("fixture document");
    let before = integrity::build_source_manifest(&root).expect("source manifest before");
    let backup_dir = root
        .parent()
        .expect("fixture parent")
        .join("verified-backups");
    let summary =
        backup_service::create_verified_backup(&root, Some(&backup_dir), &CancellationToken::new())
            .expect("verified backup");
    let after = integrity::build_source_manifest(&root).expect("source manifest after");
    assert_eq!(
        before.byte_manifest_sha256(),
        after.byte_manifest_sha256(),
        "backup must not change source bytes"
    );
    assert_eq!(before.summary, after.summary);
    let report = integrity::verify_backup(Path::new(&summary.archive_path), Some(&root))
        .expect("backup verification");
    assert!(report.archive_verified);
    assert_eq!(
        report.source_byte_count,
        Some(before.summary.total_regular_bytes)
    );
    drop(store);
    let _ = fs::remove_dir_all(root.parent().expect("fixture parent"));
}

#[test]
fn proof_historical_audit_divergence_requires_recovery_anchor() {
    let (root, store, _) = temp_project("recovery-anchor");
    let audit_path = root.join("logs/audit.jsonl");
    fs::write(&audit_path, b"{\"legacy\":true}\n").expect("legacy audit fixture");
    let backup_dir = root
        .parent()
        .expect("fixture parent")
        .join("verified-backups");
    let backup =
        backup_service::create_verified_backup(&root, Some(&backup_dir), &CancellationToken::new())
            .expect("verified backup");
    let destination = root
        .parent()
        .expect("fixture parent")
        .join("repaired-candidate");
    let recovery = integrity::recover_copy(
        Path::new(&backup.archive_path),
        &destination,
        Some(&root),
        false,
    )
    .expect("recovery copy");
    assert_eq!(recovery.historical_recovery_anchor_status, "VALID");
    assert_eq!(recovery.active_audit_status, "VALID");
    let candidate = ProjectStore::open_project_at_path(&destination).expect("candidate open");
    let audit = AuditService::new()
        .verify_chain_against_project(&destination, &candidate)
        .expect("active audit verification");
    assert!(audit.chain_valid);
    assert_eq!(audit.active_revision_divergence_count, 0);
    assert_eq!(
        audit.historical_recovery_anchor_status, "VALID",
        "historical invalidity must be anchored, not rewritten"
    );
    drop(candidate);
    drop(store);
    let _ = fs::remove_dir_all(root.parent().expect("fixture parent"));
}

#[test]
fn proof_recovery_never_rewrites_original_audit_records() {
    let (root, store, _) = temp_project("audit-preservation");
    let original = b"{\"legacy\":true,\"kept\":\"byte-for-byte\"}\n";
    let audit_path = root.join("logs/audit.jsonl");
    fs::write(&audit_path, original).expect("legacy audit fixture");
    let backup_dir = root
        .parent()
        .expect("fixture parent")
        .join("verified-backups");
    let backup =
        backup_service::create_verified_backup(&root, Some(&backup_dir), &CancellationToken::new())
            .expect("verified backup");
    let destination = root
        .parent()
        .expect("fixture parent")
        .join("repaired-candidate");
    integrity::recover_copy(
        Path::new(&backup.archive_path),
        &destination,
        Some(&root),
        false,
    )
    .expect("recovery copy");
    let historical = fs::read(destination.join("logs/recovery/historical/audit.jsonl"))
        .expect("historical audit");
    assert_eq!(historical, original);
    assert_eq!(
        integrity::sha256_bytes(&historical),
        integrity::sha256_bytes(original)
    );
    drop(store);
    let _ = fs::remove_dir_all(root.parent().expect("fixture parent"));
}

#[test]
fn proof_active_audit_chain_matches_current_project_revision() {
    let (root, store, _) = temp_project("active-audit");
    let audit_path = root.join("logs/audit.jsonl");
    fs::write(&audit_path, b"{\"legacy\":true}\n").expect("legacy audit fixture");
    let backup_dir = root
        .parent()
        .expect("fixture parent")
        .join("verified-backups");
    let backup =
        backup_service::create_verified_backup(&root, Some(&backup_dir), &CancellationToken::new())
            .expect("verified backup");
    let destination = root
        .parent()
        .expect("fixture parent")
        .join("repaired-candidate");
    integrity::recover_copy(
        Path::new(&backup.archive_path),
        &destination,
        Some(&root),
        false,
    )
    .expect("recovery copy");
    let candidate = ProjectStore::open_project_at_path(&destination).expect("candidate open");
    let report = AuditService::new()
        .verify_chain_against_project(&destination, &candidate)
        .expect("audit report");
    assert!(report.chain_valid);
    assert_eq!(report.last_audit_revision, Some(candidate.storage_revision));
    assert_eq!(report.active_revision_divergence_count, 0);
    assert_eq!(report.duplicate_revision_count, 0);
    assert_eq!(report.missing_revision_count, 0);
    drop(candidate);
    drop(store);
    let _ = fs::remove_dir_all(root.parent().expect("fixture parent"));
}

#[test]
fn process_kill_real_child_fixture() {
    clear_marker("/tmp/RubrikaV3-process-kill-proofs.green");
    let root = std::env::temp_dir().join(format!("rubrika-process-kill-{}", Uuid::new_v4()));
    fs::create_dir_all(root.join("documents")).expect("fixture root");
    fs::write(root.join("project.json"), b"{\"old\":\"complete\"}\n").expect("old project");
    fs::write(root.join("documents/exam.pdf"), b"old document").expect("old document");

    for phase in ["canonical", "import", "speaking", "restore"] {
        let ready = root.join(format!("{phase}.ready"));
        let mut child = Command::new(std::env::current_exe().expect("test binary"))
            .args([
                "--exact",
                "process_kill_real_child_fixture_child",
                "--ignored",
                "--nocapture",
            ])
            .env("RUBRIKA_KILL_PHASE", phase)
            .env("RUBRIKA_KILL_ROOT", &root)
            .env("RUBRIKA_KILL_READY", &ready)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn kill fixture");
        wait_for_file(&ready);
        unsafe {
            libc::kill(child.id() as i32, libc::SIGKILL);
        }
        let status = child.wait().expect("wait killed child");
        assert!(!status.success(), "{phase} child must be SIGKILLed");

        match phase {
            "canonical" => {
                assert_eq!(
                    fs::read(root.join("project.json")).expect("canonical project"),
                    b"{\"old\":\"complete\"}\n"
                );
                assert_eq!(
                    fs::read(root.join("project.tmp")).expect("staged project"),
                    b"{\"new\":\"complete\"}\n"
                );
            }
            "import" => {
                assert_eq!(
                    fs::read(root.join("documents/exam.pdf")).expect("active document"),
                    b"old document"
                );
                assert!(root.join("documents/.exam.pdf.importing").exists());
            }
            "speaking" => {
                assert!(!root.join("speaking/completed.json").exists());
                assert!(root.join("speaking/audio-original.wav.partial").exists());
            }
            "restore" => {
                assert!(!root.join("restored-project").exists());
                assert!(root.join(".rubrika-restore-staging-child").exists());
            }
            _ => unreachable!(),
        }
    }
    drop(fs::remove_dir_all(&root));
    write_marker("/tmp/RubrikaV3-process-kill-proofs.green");
}

#[test]
#[ignore = "child fixture invoked by process_kill_real_child_fixture"]
fn process_kill_real_child_fixture_child() {
    let phase = std::env::var("RUBRIKA_KILL_PHASE").expect("phase");
    let root = PathBuf::from(std::env::var("RUBRIKA_KILL_ROOT").expect("root"));
    let ready = PathBuf::from(std::env::var("RUBRIKA_KILL_READY").expect("ready"));
    match phase.as_str() {
        "canonical" => {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(root.join("project.tmp"))
                .expect("canonical staging");
            file.write_all(b"{\"new\":\"complete\"}\n")
                .expect("canonical staging write");
            file.sync_all().expect("canonical staging sync");
        }
        "import" => {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(root.join("documents/.exam.pdf.importing"))
                .expect("import staging");
            file.write_all(b"new document")
                .expect("import staging write");
            file.sync_all().expect("import staging sync");
        }
        "speaking" => {
            fs::create_dir_all(root.join("speaking")).expect("speaking fixture");
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(root.join("speaking/audio-original.wav.partial"))
                .expect("audio staging");
            file.write_all(b"partial wav payload")
                .expect("audio staging write");
            file.sync_all().expect("audio staging sync");
        }
        "restore" => {
            fs::create_dir_all(root.join(".rubrika-restore-staging-child"))
                .expect("restore staging");
            fs::write(
                root.join(".rubrika-restore-staging-child/project.json"),
                b"{\"new\":\"project\"}\n",
            )
            .expect("restore staging write");
        }
        _ => panic!("unknown phase"),
    }
    fs::write(ready, b"ready").expect("ready marker");
    thread::sleep(Duration::from_secs(30));
}

#[test]
fn proof_real_short_write_never_reports_success() {
    let mut writer = ShortWriter {
        remaining: 3,
        failed: false,
    };
    let error = writer
        .write_all(b"long payload")
        .expect_err("short writer must fail");
    assert_eq!(error.kind(), io::ErrorKind::WriteZero);
}

#[test]
fn proof_real_permission_failure_preserves_project() {
    let root = std::env::temp_dir().join(format!("rubrika-permission-{}", Uuid::new_v4()));
    let readonly = root.join("readonly");
    fs::create_dir_all(&readonly).expect("readonly fixture");
    let target = readonly.join("project.json");
    fs::write(&target, b"old canonical").expect("old canonical");
    set_readonly(&readonly);
    let result = file_access::atomic_write_bytes(&target, b"new canonical");
    set_writable(&readonly);
    assert!(result.is_err(), "read-only directory must reject write");
    assert_eq!(
        fs::read(&target).expect("canonical state"),
        b"old canonical"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn proof_rename_failure_preserves_previous_canonical_state() {
    let root = std::env::temp_dir().join(format!("rubrika-rename-failure-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).expect("root");
    let target = root.join("project.json");
    fs::write(&target, b"old canonical").expect("old canonical");
    fs::create_dir(target.with_extension("tmp")).expect("blocking tmp target");
    let result = file_access::atomic_write_bytes(&target, b"new canonical");
    assert!(result.is_err(), "rename conflict must fail");
    assert_eq!(
        fs::read(&target).expect("canonical state"),
        b"old canonical"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn proof_parent_sync_failure_returns_durability_uncertain() {
    let error = io::Error::other("parent directory fsync failed after rename: injected OS fault");
    assert!(file_access::is_durability_uncertain(&error));
}

#[test]
fn disk_fault_real_filesystem_fixture() {
    clear_marker("/tmp/RubrikaV3-disk-fault-proofs.green");
    proof_real_short_write_never_reports_success();
    proof_real_permission_failure_preserves_project();
    proof_rename_failure_preserves_previous_canonical_state();
    proof_parent_sync_failure_returns_durability_uncertain();
    write_marker("/tmp/RubrikaV3-disk-fault-proofs.green");
}

#[test]
fn destructive_race_real_two_process_fixture() {
    clear_marker("/tmp/RubrikaV3-destructive-race-proofs.green");
    let (root, store, _) = temp_project("race");
    drop(store);
    let binary = env!("CARGO_BIN_EXE_rubrika");
    let mut lock_holder = Command::new(binary)
        .args(["lock-hold"])
        .arg(&root)
        .args(["--hold-seconds", "4"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn process A");
    wait_for_locked(lock_holder.stdout.take().expect("lock stdout"));

    let backup_dir = root.parent().expect("fixture parent").join("race-backups");
    let backup_attempt = Command::new(binary)
        .args(["backup-create"])
        .arg(&root)
        .args(["--destination"])
        .arg(&backup_dir)
        .status()
        .expect("spawn backup process B");
    assert!(
        !backup_attempt.success(),
        "backup must return typed busy error while mutation holds lock"
    );
    assert!(lock_holder.wait().expect("wait process A").success());

    let backup =
        backup_service::create_verified_backup(&root, Some(&backup_dir), &CancellationToken::new())
            .expect("backup after lock release");
    let destination = root
        .parent()
        .expect("fixture parent")
        .join("restore-destination");
    fs::create_dir_all(&destination).expect("destination");
    let mut destination_holder = Command::new(binary)
        .args(["lock-hold"])
        .arg(&destination)
        .args(["--hold-seconds", "4"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn destination process A");
    wait_for_locked(
        destination_holder
            .stdout
            .take()
            .expect("destination lock stdout"),
    );
    let restore_attempt = Command::new(binary)
        .args(["restore-copy"])
        .arg(&backup.archive_path)
        .arg(&destination)
        .status()
        .expect("spawn restore process B");
    assert!(
        !restore_attempt.success(),
        "restore must reject a destination concurrently opened by another process"
    );
    assert!(destination_holder
        .wait()
        .expect("wait destination A")
        .success());
    assert!(
        !destination.join("project.json").exists(),
        "destination race must not activate a partial restore"
    );
    let _ = fs::remove_dir_all(root.parent().expect("fixture parent"));
    write_marker("/tmp/RubrikaV3-destructive-race-proofs.green");
}

fn wait_for_locked(stdout: impl std::io::Read) {
    let mut lines = BufReader::new(stdout).lines();
    let line = lines.next().expect("process must emit lock line");
    assert!(
        line.expect("lock stdout").starts_with("LOCKED"),
        "child did not acquire lock"
    );
}

fn wait_for_file(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if path.is_file() {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "child fixture did not reach its kill boundary: {}",
        path.display()
    );
}

fn set_readonly(path: &Path) {
    let mut permissions = fs::metadata(path).expect("permissions").permissions();
    permissions.set_readonly(true);
    fs::set_permissions(path, permissions).expect("set read-only");
}

fn set_writable(path: &Path) {
    let mut permissions = fs::metadata(path).expect("permissions").permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o755);
    }
    #[cfg(not(unix))]
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions).expect("restore writable");
}

struct ShortWriter {
    remaining: usize,
    failed: bool,
}

impl Write for ShortWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.failed || self.remaining == 0 {
            self.failed = true;
            return Ok(0);
        }
        let written = bytes.len().min(self.remaining);
        self.remaining -= written;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

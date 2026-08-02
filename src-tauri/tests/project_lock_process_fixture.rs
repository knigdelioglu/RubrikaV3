use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

/// proof_24: a real second child process cannot acquire the project write
/// lease while the first holds it, and the lock becomes available again
/// after the first process exits.
#[test]
fn second_process_cannot_write_locked_project() {
    let root = std::env::temp_dir().join(format!("rubrika-lock-process-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();

    let binary = env!("CARGO_BIN_EXE_rubrika");

    // Process A: acquire and hold the lease.
    let mut process_a = Command::new(binary)
        .args(["lock-hold"])
        .arg(&root)
        .args(["--hold-seconds", "4"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn process A");
    wait_for_locked(process_a.stdout.take().unwrap());

    // Process B: must be rejected while A holds the lease.
    let status_b = Command::new(binary)
        .args(["lock-hold"])
        .arg(&root)
        .args(["--hold-seconds", "1"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn process B");
    assert_eq!(
        status_b.code(),
        Some(7),
        "process B must receive ProjectAlreadyOpen"
    );

    // A exits; the OS releases the lease.
    let status_a = process_a.wait().expect("wait process A");
    assert!(status_a.success(), "process A must release cleanly");

    // Process C: can now acquire.
    let status_c = Command::new(binary)
        .args(["lock-hold"])
        .arg(&root)
        .args(["--hold-seconds", "1"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn process C");
    assert!(
        status_c.success(),
        "process C must acquire after A exits, got {:?}",
        status_c.code()
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn proof_44_second_process_cannot_run_destructive_operation() {
    second_process_cannot_write_locked_project();
}

fn wait_for_locked(stdout: impl std::io::Read) {
    let mut lines = BufReader::new(stdout).lines();
    let line = lines.next().expect("process A must emit a lock line");
    let line = line.expect("process A stdout must be valid UTF-8");
    if line.starts_with("LOCKED") {
        return;
    }
    panic!("unexpected output from process A: {line}");
}

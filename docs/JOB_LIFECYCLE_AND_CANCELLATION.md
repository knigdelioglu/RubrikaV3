# Job Lifecycle and Cancellation Architecture (Phase 5 & Phase 5C)

This document defines the canonical job state machine, persistence rules, startup rehydration, atomic duplicate prevention, cancellation token mechanics, correlation ID propagation, document import job handling, service-specific cancellation contracts, production proof test inventory, and UI event contracts for RubrikaV3.

---

## 1. Canonical Job State Machine

The job state machine manages all asynchronous long-running tasks in RubrikaV3.

### States
- **Queued**: Job registered, waiting to start task.
- **Running**: Task running asynchronously.
- **Partial**: Terminal state. Completed partially (e.g. 61 of 63 students processed, 2 require review). Emits `job_partial`. NOT Succeeded.
- **Succeeded**: Terminal state. All acceptance criteria fully met. Emits `job_succeeded`.
- **Failed**: Terminal state. Task failed with structured `AppError`. Emits `job_failed`.
- **Cancelled**: Terminal state. Observed `cancellation_token` cancel signal, cleaned up staging/lease, stopped safely. Emits `job_cancelled`.
- **Interrupted**: Terminal state. Task lost due to app restart or process crash. Emits `job_interrupted`.

### State Transition Rules
1. **Terminal Lock**: Once a job enters a terminal state (`Partial`, `Succeeded`, `Failed`, `Cancelled`, `Interrupted`), no further status transitions or progress updates are allowed.
2. **Cancellation Flag**: `cancellation_requested: bool` records user request. The job status remains `Running` until the task reaches a safe checkpoint and completes cleanup, at which point it transitions to `Cancelled`.
3. **Single Terminal Event**: Exactly one terminal event is emitted per job (`job_succeeded`, `job_partial`, `job_failed`, `job_cancelled`, or `job_interrupted`).

---

## 2. Production Job Inventory & Service Cancellation Contracts

| Job Kind | Scope | Duplicate Policy | Safe Cancellation Checkpoints | Commit & Data Preservation Contract |
|---|---|---|---|---|
| `DocumentImport` | Project + Doc Role | `ReturnExisting` | Before copy, chunk boundaries, before validation | Partial file unlinked from disk; project `documents` list untouched |
| `PdfPreviewRender` | Project + Doc ID | `ReturnExisting` | Before render, page loop, before manifest update | Staging directory unlinked; existing active preview generation preserved |
| `QuestionTextExtraction` | Project + Doc ID | `ReturnExisting` | After `set_running`, vision fallback loop, before commit | Teacher-edited/confirmed question text preserved byte-for-byte; suggested text discarded |
| `RubricPdfImport` | Project + Doc ID | `ReturnExisting` | After `set_running`, before runtime acquire, before commit | Teacher-edited/confirmed rubric state remains `Confirmed`; draft rubric discarded |
| `ExamPackageBuild` | Project | `ReturnExisting` | Between package stages, before freeze assertion | `exam_package_freeze` snapshot is NOT created or set to `Frozen` |
| `StudentAnswerOcr` | Project + Scope | `ReturnExisting` | Before lease acquire, item loop, before commit | Active OCR records preserved; candidate record discarded without approval |
| `StudentIdentityOcr` | Project + Doc ID | `ReturnExisting` | Page loop, before model calls, before commit | Unconfirmed student identity mappings uncommitted; existing roster untouched |
| `Scoring` | Project + Question | `ReturnExisting` | Before lease acquire, item loop, before commit | Existing valid score records preserved; invalid/cancelled score ignored |
| `SpeakingEvaluation` | Project + Exam ID | `ReturnExisting` | Post `set_running`, post transcript cleanup, before commit | Teacher notes, star ratings, and readable transcript preserved byte-for-byte |
| `AssessmentAnalysis` | Project | `ReturnExisting` | Before model call, after model call, before report save | Final report file uncommitted; `AnalysisStatus` remains untouched (not set to `Ready` or `Partial`) |

---

## 3. Production Proof Test Inventory (Phase 5C Verification)

The backend test suite enforces 16 production proof tests (`proof_1` through `proof_16`) covering concurrency, cancellation, fault isolation, and retention:

| Proof ID | Test Name | Location | Verified Behavior |
|---|---|---|---|
| `proof_1` | `proof_1_50_concurrent_duplicate_requests_single_job` | `job_manager.rs` | 50 concurrent requests produce exactly 1 job registration |
| `proof_2` | `proof_2_real_cancellation` | `job_manager.rs` | Cancellation token signals task to cancel cleanly |
| `proof_3` | `proof_3_partial_is_not_succeeded` | `job_manager.rs` | Partial state is terminal and emits `job_partial`, not `job_succeeded` |
| `proof_4` | `proof_4_restart_recovery` | `job_manager.rs` | Rehydrated active jobs transition to `Interrupted` on restart |
| `proof_5` | `proof_5_preview_cancel_preserves_active_generation` | `pdf_preview_service.rs` | Cancelled preview generation leaves active preview generation intact |
| `proof_6` | `proof_6_rubric_cancel_preserves_existing_state` | `rubric_extraction_service.rs` | Cancelled rubric import leaves existing teacher confirmed rubric untouched |
| `proof_7` | `proof_7_question_cancel_preserves_teacher_text` | `question_text_service.rs` | Cancelled question extraction leaves teacher-edited question text intact |
| `proof_8` | `proof_8_speaking_cancel_preserves_teacher_data` | `speaking_exam_service.rs` | Cancelled speaking evaluation preserves teacher ratings and notes |
| `proof_9` | `proof_9_analysis_cancel_does_not_finalize_report` | `analysis_service.rs` | Cancelled analysis does not write `Ready` report or set `Partial` status |
| `proof_10` | `proof_10_correlation_id_is_end_to_end` | `job_manager.rs` | `correlation_id` matches across input, snapshot, error, and events |
| `proof_11` | `proof_11_retry_creates_new_job` | `job_manager.rs` | Retrying a job creates a new `job_id` referencing `retry_of_job_id` |
| `proof_12` | `proof_12_task_panic_cannot_leave_running_job` | `job_manager.rs` | Panicked tasks are caught by guard and moved to `Failed` status |
| `proof_13` | `proof_13_controlled_shutdown_leaves_no_running_jobs` | `job_manager.rs` | App exit converts running jobs to `Interrupted` and blocks new jobs |
| `proof_14` | `proof_14_retention_preserves_active_and_referenced_jobs` | `job_manager.rs` | History retention cleans old terminal jobs while preserving active/referenced jobs |
| `proof_15` | `proof_15_document_import_cancel_never_activates_partial_file` | `document_service.rs` | Cancelled document import unlinks partial target file without adding to project |
| `proof_16` | `proof_16_exam_package_build_cancel_preserves_unfrozen_state` | `exam_package_build_service.rs` | Cancelled exam package build does not create or freeze package snapshot |
| `proof_17` | `proof_17_real_tauri_shutdown_rehydrates_running_jobs_as_interrupted` | `job_manager.rs` | Real Tauri shutdown persists active jobs as `Interrupted` and rehydrates 0 active jobs on relaunch |

---

## 4. End-to-End Correlation ID Equality Chain

Every long-running job enforces identical correlation ID propagation across all architectural layers:

$$\text{Command Correlation ID} = \text{JobSnapshot Correlation ID} = \text{Model Lease Correlation ID} = \text{ProjectStore Commit Event ID} = \text{Tauri Event ID}$$

Verification in Rust unit tests (`proof_10`):
```rust
assert_eq!(reg.snapshot.correlation_id, expected_corr_id);
assert_eq!(snap.error.unwrap().correlation_id, expected_corr_id);
```

---

## 5. Persistence Authority & Rehydration

- `JobManager` is the single authority for job persistence.
- Snapshots are written atomically (`temp -> flush -> sync -> rename`) under `<project_root>/logs/jobs/<job_id>.json` using `TrustedProjectRoot`.
- On application startup or project opening, `load_persisted_jobs` loads existing JSON snapshots into `JobManager`.
- Any non-terminal jobs (`Queued` or `Running`) loaded during rehydration without an active task handle are automatically transitioned to `Interrupted`.

---

## 6. Atomic Registration & Duplicate Prevention

- `register_or_get_active_job` executes atomic duplicate check and insertion under a single `Mutex` lock.
- Backend computes a canonical `idempotency_key` derived from `project_id + operation_kind + scope + input_fingerprint`.
- Policy:
  - `ReturnExisting`: Returns snapshot of already running job.
  - `RejectAlreadyRunning`: Returns `AppErrorCode::JobAlreadyRunning`.
  - `AllowParallel`: Generates unique job.

---

## 7. Task Panic Protection & Execution Guard (`JobTaskGuard`)

- If a Tokio background task panics or is dropped unexpectedly while running a job, `JobTaskGuard` automatically detects task termination upon drop.
- If the job is still marked as `Running` when the guard drops:
  - If `cancellation_requested` is true, the job transitions to `Cancelled`.
  - Otherwise, the job transitions to `Failed` with `AppErrorCode::JobExecutionOwnerLost` ("İşlemi yürüten görev beklenmeyen bir şekilde sonlandı.").
- This guarantees zero orphan `Running` or `Queued` states.

---

## 8. Job History Retention & Retries

- **Retry Semantics**: Calling `retry_job(old_job_id)` validates that `old_job_id` is terminal (`Failed`, `Cancelled`, `Interrupted`). It generates a fresh `job_id` and new `correlation_id`, preserves `retry_of_job_id = Some(old_job_id)`, and registers a new job.
- **Retention Cleanup**: `cleanup_job_history(root_path, max_terminal_jobs)`:
  - Protects active jobs (`Queued`, `Running`).
  - Protects terminal jobs that are referenced by active or retained `retry_of_job_id` chains.
  - Sorts remaining terminal jobs by `updated_at` descending.
  - Retains top `max_terminal_jobs` (default 100) and safely removes older terminal job JSON files from disk.

---

## 9. Controlled Application Shutdown

- Wired into Tauri application lifecycle in `lib.rs`:
```rust
.run(|app_handle, event| {
    if let tauri::RunEvent::ExitRequested { .. } = event {
        if let Some(state) = app_handle.try_state::<AppState>() {
            state.job_manager.shutdown_all_jobs(app_handle);
        }
    }
});
```
- During application shutdown (`shutdown_all_jobs`):
  - Sets `accepting_new_jobs = false` to reject any late job registrations.
  - Signals `cancel()` on all active job `CancellationToken`s.
  - Transitions all non-stopped active jobs to `Interrupted` and persists their updated snapshots.

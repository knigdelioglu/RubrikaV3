# Job System

Long-running operations (like PDF import, exam package build, OCR, scoring) must never block a Tauri command directly. They must be executed as background jobs that emit events.

## Job Lifecycle

1. **Queued**: Job is submitted and receives a `job_id`.
2. **Running**: Job is actively being processed.
3. **Completed**: Job can end in one of three states:
   - `succeeded`: The job finished successfully.
   - `failed`: The job encountered an error.
   - `cancelled`: The job was terminated before completion.

## Job Event Schema

All jobs emit structured events back to the UI.

```typescript
type JobEvent =
  | { type: "job_started"; jobId: string; kind: JobKind }
  | { type: "job_progress"; jobId: string; current: number; total: number; message: string }
  | { type: "job_succeeded"; jobId: string; result: unknown }
  | { type: "job_failed"; jobId: string; error: AppErrorDto }
  | { type: "job_cancelled"; jobId: string };
```

## Progress Event Schema

Progress updates use the `job_progress` type:
- `current`: The current item being processed (e.g., student index, page number).
- `total`: Total number of items (e.g., total students, total pages).
- `message`: Human-readable progress description (e.g., "Öğrenci 2 / Sayfa 1 işleniyor").

## Failure Event Schema

When a job fails, it emits a `job_failed` event containing an `AppErrorDto` (mapped from `AppError`). This error must appear in the UI error banner, job history panel, project log, and diagnostic export.

## Cancellation Rule

Jobs must periodically check a cancellation token or atomic flag. If a job receives a cancellation signal, it must cleanly abort its current operation, avoid writing partial/corrupt state, and emit a `job_cancelled` event.

## Diagnostic Logging

All job events, state transitions, and failures must be logged with their associated `correlation_id` and `job_id`. This history is crucial for the diagnostic export feature and debugging.
For PDF-backed extraction jobs, the job log should also point to the shared document content artifact directory so question text and rubric runs can be traced back to the same extracted source material.

## Persistent Job Snapshots
- Job snapshots are persisted under `logs/jobs/<job_id>.json`.
- The diagnostic CLI reads persisted snapshots to inspect stale or running jobs after restart.
- A stale candidate is a queued or running job that has not advanced for a meaningful diagnostic window.

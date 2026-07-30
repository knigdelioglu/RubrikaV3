# API Contracts

This document defines the Tauri commands that bridge the React frontend with the Rust backend in Rubrika v3.

## Core Principles
- All commands are typed (input, success output, error output).
- Long-running operations must be implemented as jobs, not synchronous commands.
- Errors must use the `AppError` structure defined in `ERROR_CODES.md`.

## Commands

### Speaking evaluation contract

Speaking attempts persist `rawTranscript`, segment-preserving cleanup candidate/status, `transcriptForScoring`, model provenance, frozen prompt/policy versions and `evaluationInputHash`. Cleanup failures return a reviewable attempt and do not create a scoring transcript. Speaking model output contains subindicator level, positive/counter evidence IDs, missing requirements and rationale; backend validates evidence and computes whole-point criterion and total scores.

`SpeakingSubindicatorScore` exposes `selectedLevelId`, `appliedLevelId`, `points`, positive/counter evidence IDs, missing requirements and optional ceiling reason/explanation. `selectedLevelId` is immutable model provenance; UI uses `appliedLevelId` for the effective contribution.

`SpeakingMetrics` exposes recording/active speech durations, words per minute, silence/long pause/filler/repetition counts, duration tier, expected minimum duration, `sampleDurationSufficient` and `measurementConfidence`. A low-confidence short sample requires teacher review and is not automatic zero.

### `get_app_status`
- **Input**: None
- **Output**: `AppStatus` (includes version, platform, config paths)
- **Errors**: None expected
- **Job-based**: No

### `create_project`
- **Input**: `CreateProjectInput { name: String, root_path: Option<String> }`
- **Output**: `ProjectSnapshot`
- **Errors**: `PERMISSION_DENIED`, `PROJECT_SAVE_FAILED`
- **Job-based**: No

### `open_project`
- **Input**: `OpenProjectInput { path: String }`
- **Output**: `ProjectSnapshot`
- **Errors**: `PROJECT_NOT_FOUND`, `PROJECT_LOAD_FAILED`
- **Job-based**: No

### `get_project_snapshot`
- **Input**: `GetProjectInput { project_id: String }`
- **Output**: `ProjectSnapshot`
- **Errors**: `PROJECT_NOT_FOUND`
- **Job-based**: No

### `get_workflow_snapshot`
- **Input**: `GetWorkflowInput { project_id: String }`
- **Output**: `WorkflowSnapshot`
- **Errors**: `PROJECT_NOT_FOUND`
- **Job-based**: No

### `import_exam_source_pdf`
- **Input**: `ImportPdfInput { project_id: String, file_path: String }`
- **Output**: `DocumentSnapshot`
- **Errors**: `DOCUMENT_IMPORT_FAILED`, `PDF_RENDER_FAILED`
- **Job-based**: Yes (triggers PDF rendering job)

### `get_pdf_page_count`
- **Input**: `PdfDocumentInput { project_id: String, document_id: String }`
- **Output**: `u32`
- **Errors**: `PDF_DOCUMENT_NOT_FOUND`, `PDF_PAGE_COUNT_FAILED`
- **Job-based**: No

### `start_pdf_preview_render`
- **Input**: `PdfDocumentInput { project_id: String, document_id: String }`
- **Output**: `StartPdfPreviewRenderOutput { job_id: String, status: "queued" | "running" }`
- **Errors**: `PDF_DOCUMENT_NOT_FOUND`, `PDF_RENDER_FAILED`, `FILE_WRITE_FAILED`
- **Job-based**: Yes

### `get_pdf_preview_status`
- **Input**: `PdfDocumentInput { project_id: String, document_id: String }`
- **Output**: `PdfPreviewStatusSnapshot`
- **Errors**: `PDF_DOCUMENT_NOT_FOUND`, `PDF_PREVIEW_NOT_FOUND`
- **Job-based**: No

### `get_pdf_renderer_status`
- **Input**: None
- **Output**: `PdfRendererStatus`
- **Errors**: None expected
- **Job-based**: No

### `get_pdf_page_preview`
- **Input**: `PdfPagePreviewInput { project_id: String, document_id: String, page_number: u32 }`
- **Output**: `PdfPagePreview`
- **Errors**: `PDF_DOCUMENT_NOT_FOUND`, `PDF_PREVIEW_NOT_FOUND`
- **Job-based**: No

### `list_pdf_page_previews`
- **Input**: `PdfDocumentInput { project_id: String, document_id: String }`
- **Output**: `PdfPagePreview[]`
- **Errors**: `PDF_DOCUMENT_NOT_FOUND`, `PDF_PREVIEW_NOT_FOUND`
- **Job-based**: No

### `import_answer_key_pdf`
- **Input**: `ImportPdfInput { project_id: String, file_path: String }`
- **Output**: `DocumentSnapshot`
- **Errors**: `DOCUMENT_IMPORT_FAILED`, `PDF_RENDER_FAILED`
- **Job-based**: Yes (triggers PDF rendering job)

### `import_student_scan_pdf`
- **Input**: `ImportPdfInput { project_id: String, file_path: String }`
- **Output**: `DocumentSnapshot`
- **Errors**: `DOCUMENT_IMPORT_FAILED`, `PDF_RENDER_FAILED`
- **Job-based**: Yes (triggers PDF rendering job)

### `list_student_scan_documents`
- **Input**: `ProjectIdInput { project_id: String }`
- **Output**: `Document[]`
- **Errors**: `PROJECT_NOT_FOUND`
- **Job-based**: No

### `start_student_scan_preview_render`
- **Input**: `PdfDocumentInput { project_id: String, document_id: String }`
- **Output**: `StartPdfPreviewRenderOutput { job_id: String, status: "queued" | "running" }`
- **Errors**: `PDF_DOCUMENT_NOT_FOUND`, `PDF_RENDER_FAILED`, `FILE_WRITE_FAILED`
- **Job-based**: Yes

### `get_student_scan_preview_status`
- **Input**: `PdfDocumentInput { project_id: String, document_id: String }`
- **Output**: `PdfPreviewStatusSnapshot`
- **Errors**: `PDF_DOCUMENT_NOT_FOUND`, `PDF_PREVIEW_NOT_FOUND`
- **Job-based**: No

### `create_student_page_groups`
- **Input**: `CreateStudentPageGroupsInput { project_id: String, document_id: String, mode: "one_pdf_one_student" | "fixed_pages_per_student" | "manual", pages_per_student?: number }`
- **Output**: `CreateStudentPageGroupsOutput`
- **Errors**: `STUDENT_SCAN_NOT_FOUND`, `STUDENT_SCAN_PREVIEW_NOT_READY`, `STUDENT_GROUPING_INVALID`
- **Job-based**: No

### `list_student_submissions`
- **Input**: `ProjectIdInput { project_id: String }`
- **Output**: `StudentSubmission[]`
- **Errors**: `PROJECT_NOT_FOUND`
- **Job-based**: No

### `update_student_identity`
- **Input**: `UpdateStudentIdentityInput { project_id: String, submission_id: String, display_name?: String, number?: String, class_name?: String }`
- **Output**: `StudentSubmission`
- **Errors**: `STUDENT_SUBMISSION_NOT_FOUND`, `STUDENT_IDENTITY_INVALID`
- **Job-based**: No

### `update_submission_pages`
- **Input**: `UpdateSubmissionPagesInput { project_id: String, submission_id: String, page_numbers: number[] }`
- **Output**: `StudentSubmission`
- **Errors**: `STUDENT_SUBMISSION_NOT_FOUND`, `STUDENT_GROUPING_INVALID`
- **Job-based**: No

### `delete_student_submission`
- **Input**: `DeleteStudentSubmissionInput { project_id: String, submission_id: String }`
- **Output**: `void`
- **Errors**: `STUDENT_SUBMISSION_NOT_FOUND`
- **Job-based**: No

### `mark_student_grouping_complete`
- **Input**: `MarkStudentGroupingCompleteInput { project_id: String }`
- **Output**: `Project`
- **Errors**: `STUDENT_SCAN_NOT_FOUND`, `STUDENT_SCAN_PREVIEW_NOT_READY`, `STUDENT_IDENTITY_INVALID`
- **Job-based**: No

### `get_ocr_readiness`
- **Input**: `ProjectIdInput { project_id: String }`
- **Output**: `StudentScanReadinessSnapshot`
- **Errors**: `PROJECT_NOT_FOUND`
- **Job-based**: No

### `start_student_answer_ocr`
- **Input**: `ProjectIdInput { project_id: String }`
- **Output**: `StartStudentAnswerOcrOutput { job_id: String, status: "queued" }`
- **Errors**: `WORKFLOW_BLOCKED`, `STUDENT_SCAN_NOT_FOUND`, `STUDENT_SCAN_PREVIEW_NOT_READY`, `STUDENT_GROUPING_NOT_READY`, `QUESTION_TEXT_MISSING`, `MODEL_SERVER_START_FAILED`, `MODEL_SERVER_READY_TIMEOUT`
- **Job-based**: Yes
- **Notes**: If the model server is not healthy yet, the backend starts the managed model process, waits for `/health`, then queues OCR. Teacher-facing errors stay structured; technical details remain in diagnostics.

### `start_question_text_extraction`
- **Input**: `StartQuestionTextExtractionInput { project_id: String, document_id?: String, source: "exam_pdf" }`
- **Output**: `JobSnapshot`
- **Errors**: `DOCUMENT_NOT_FOUND`, `PDF_PREVIEW_NOT_READY`, `MODEL_SERVER_NOT_RUNNING`, `QUESTION_TEXT_EXTRACTION_FAILED`
- **Job-based**: Yes

### `start_exam_package_build`
- **Input**: `StartExamPackageBuildInput { project_id: String, expected_question_count: number }`
- **Output**: `StartExamPackageBuildOutput { job_id: String, status: "queued" | "running" }`
- **Errors**: `EXAM_SOURCE_PDF_MISSING`, `RUBRIC_DOCUMENT_MISSING`, `QUESTION_COUNT_MISSING`, `EXAM_PACKAGE_BUILD_PRECHECK_FAILED`, `MODEL_SERVER_NOT_RUNNING`, `MODEL_SERVER_START_FAILED`
- **Job-based**: Yes

### `get_question_text_extraction_status`
- **Input**: `QuestionTextProjectInput { project_id: String }`
- **Output**: `QuestionTextExtractionStatus`
- **Errors**: `PROJECT_NOT_FOUND`
- **Job-based**: No

### `list_question_text_suggestions`
- **Input**: `QuestionTextProjectInput { project_id: String }`
- **Output**: `QuestionTextSuggestion[]`
- **Errors**: `PROJECT_NOT_FOUND`, `DOCUMENT_NOT_FOUND`
- **Job-based**: No

### `confirm_question_text`
- **Input**: `ConfirmQuestionTextInput { project_id: String, question_id: String }`
- **Output**: `Question`
- **Errors**: `QUESTION_TEXT_CONFIRM_FAILED`, `QUESTION_TEXT_SUGGESTION_NOT_FOUND`
- **Job-based**: No

### `edit_question_text`
- **Input**: `EditQuestionTextInput { project_id: String, question_id: String, text: String }`
- **Output**: `Question`
- **Errors**: `QUESTION_TEXT_CONFIRM_FAILED`, `QUESTION_TEXT_SUGGESTION_NOT_FOUND`
- **Job-based**: No

### `import_rubric_json`
- **Input**: `ImportRubricJsonInput { project_id: String, document_id?: String, file_path?: String }`
- **Output**: `ImportRubricJsonOutput`
- **Errors**: `RUBRIC_JSON_INVALID`, `RUBRIC_JSON_SCHEMA_UNSUPPORTED`, `RUBRIC_NOT_READY`, `RUBRIC_QUESTION_NOT_FOUND`, `FILE_READ_FAILED`
- **Job-based**: No

### `get_rubric_state`
- **Input**: `GetRubricStateInput { project_id: String }`
- **Output**: `RubricStateSnapshot`
- **Errors**: `PROJECT_NOT_FOUND`
- **Job-based**: No

### `list_rubric_items`
- **Input**: `ListRubricItemsInput { project_id: String }`
- **Output**: `RubricQuestionSnapshot[]`
- **Errors**: `PROJECT_NOT_FOUND`
- **Job-based**: No

### `update_question_rubric`
- **Input**: `UpdateQuestionRubricInput { project_id: String, question_id: String, answer_type?: AnswerType, max_score?: number, expected_answer?: string, criteria: RubricCriterion[], partial_credit_hints: string[], zero_score_conditions: string[], common_mistakes: string[] }`
- **Output**: `Question`
- **Errors**: `RUBRIC_NOT_READY`, `RUBRIC_QUESTION_NOT_FOUND`, `RUBRIC_MAX_SCORE_MISSING`, `RUBRIC_CRITERIA_SCORE_MISMATCH`, `RUBRIC_PLACEHOLDER_DETECTED`
- **Job-based**: No
- **Notes**: `answer_type` is persisted in the canonical question model and invalidates a previously frozen package, so OCR cannot depend on screen-local question-type state.

### `confirm_question_rubric`
- **Input**: `ConfirmQuestionRubricInput { project_id: String, question_id: String }`
- **Output**: `Question`
- **Errors**: `RUBRIC_NOT_READY`, `RUBRIC_QUESTION_NOT_FOUND`, `RUBRIC_CONFIRM_FAILED`
- **Job-based**: No

### `confirm_all_rubrics`
- **Input**: `ConfirmAllRubricsInput { project_id: String }`
- **Output**: `Project`
- **Errors**: `RUBRIC_NOT_READY`, `RUBRIC_CONFIRM_FAILED`
- **Job-based**: No

### `validate_rubrics`
- **Input**: `ValidateRubricsInput { project_id: String }`
- **Output**: `RubricValidationReport`
- **Errors**: `PROJECT_NOT_FOUND`
- **Job-based**: No

### `start_answer_key_extraction`
- **Input**: `StartExtractionInput { project_id: String, document_id: String }`
- **Output**: `JobSnapshot`
- **Errors**: `WORKFLOW_BLOCKED`, `DOCUMENT_NOT_FOUND`
- **Job-based**: Yes

### `confirm_question_rubric`
- **Input**: `ConfirmRubricInput { project_id: String, question_id: String, rubric: RubricState }`
- **Output**: `QuestionSnapshot`
- **Errors**: `WORKFLOW_BLOCKED`
- **Job-based**: No

### `start_ocr_job`
- **Input**: `StartOcrInput { project_id: String }`
- **Output**: `JobSnapshot`
- **Errors**: `WORKFLOW_BLOCKED`, `MODEL_SERVER_NOT_RUNNING`
- **Job-based**: Yes

### `get_graded_exam_review`
- **Input**: `GetGradedExamReviewInput { project_id: String, submission_id: String }`
- **Output**: `GradedExamReview` (student page previews, normalized model-score annotations, criterion score parts, teacher-facing review guidance, placement warnings, and model total)
- **Errors**: `PROJECT_NOT_FOUND`, `STUDENT_SUBMISSION_NOT_FOUND`, `SCORING_NOT_READY`, `PDF_PREVIEW_NOT_READY`
- **Job-based**: No. This command builds a read-only annotation manifest from existing preview, crop-template, and active scoring data; it does not render or mutate the source PDF.
- **Invariant**: A missing or unapplied model score is returned as `needs_review` / `Kontrol`, never as zero. Missing crop geometry is reported in `unplaced_scores` and is never guessed.
- **Teacher review detail**: `scoreParts` preserves criterion-level awards such as `4 + 4 + 3 + 4`; `reviewGuidance` contains teacher-facing instructions rather than raw scoring/review codes.

### `build_qep_packages`
- **Input**: `BuildQepInput { project_id: String }`
- **Output**: `JobSnapshot`
- **Errors**: `WORKFLOW_BLOCKED`, `QUESTION_TEXT_MISSING`, `RUBRIC_MISSING`
- **Job-based**: Yes

### `freeze_qep_package`
- **Input**: `FreezeQepInput { project_id: String, package_id: String }`
- **Output**: `QepSnapshot`
- **Errors**: `WORKFLOW_BLOCKED`
- **Job-based**: No

### `start_scoring_job`
- **Input**: `StartScoringInput { project_id: String }`
- **Output**: `JobSnapshot`
- **Errors**: `WORKFLOW_BLOCKED`, `QEP_NOT_FROZEN`, `MODEL_SERVER_NOT_RUNNING`
- **Job-based**: Yes
- **Scoring result invariant**: `ScoringRecord.awardedScore` is nullable and `scoringApplied=false` when no trustworthy model score was produced. A model/parse failure must never be persisted as a normal zero.
- **Deterministic validation**: Criterion ids, titles, and maximum scores come from the frozen rubric. Missing criteria prevent model scoring from being applied; low confidence, short rationales, OCR uncertainty, and criterion contract mismatches require teacher review.

### `update_scoring_record`
- **Input**: `UpdateScoringRecordInput { project_id: String, record_id: String, teacher_manual_score?: f32, teacher_notes?: String, teacher_approved: bool }`
- **Output**: `ScoringRecord`
- **Errors**: `SCORING_NOT_READY`
- **Job-based**: No
- **Notes**: A record with `scoringApplied=false` cannot be approved until the teacher supplies a manual score.

### `finish_assessment`
- **Input**: `FinishAssessmentInput { project_id: String, kind: "written" | "speaking", source_id?: String }`
- **Output**: `FinishAssessmentOutput { analysis_id: String, job_id: String, status: "queued" }`
- **Errors**: `ANALYSIS_NOT_READY`, `PROJECT_NOT_FOUND`
- **Job-based**: Yes (`assessment_analysis`)
- **Notes**: Deterministic charts are saved before the Gemma report starts. Report failure produces a partial analysis and never removes chart data.

### `get_assessment_analysis`
- **Input**: `GetAssessmentAnalysisInput { project_id: String, analysis_id: String }`
- **Output**: `AssessmentAnalysis`
- **Errors**: `ANALYSIS_FAILED`, `PROJECT_NOT_FOUND`
- **Job-based**: No

### `list_assessment_analyses`
- **Input**: `ListAssessmentAnalysesInput { project_id: String }`
- **Output**: `AssessmentAnalysis[]`
- **Errors**: `ANALYSIS_FAILED`, `PROJECT_NOT_FOUND`
- **Job-based**: No

### Speaking roster and review commands
- `list_class_students` reads the canonical common class roster used by written and speaking exams.
- `select_speaking_exam_class` and `select_speaking_exam_student` persist the resumable session location.
- `update_speaking_criterion_level` accepts `very_good | good | moderate | developing` for teacher-only criteria and maps the level to a bounded score in Rust.
- `update_speaking_attempt_note` persists the teacher note without waiting for final approval.
- `approve_speaking_attempt` atomically saves the score, then permanently deletes the temporary WAV.

### `get_model_status`
- **Input**: None
- **Output**: `ModelStatus`
- **Errors**: None
- **Job-based**: No

### `get_model_runtime_status`
- **Input**: None
- **Output**: `ModelRuntimeStatus`
- **Errors**: None
- **Job-based**: No
- **Notes**: Central runtime status surface for startup, health, and capability checks.

### `start_model_server`
- **Input**: `ProfileSelectionInput { profileId?: string }`
- **Output**: `StartModelServerOutput`
- **Errors**: `MODEL_PROFILE_NOT_MANAGED`, `MODEL_SERVER_PATH_MISSING`, `MODEL_MODEL_PATH_MISSING`, `MODEL_MMPROJ_PATH_MISSING`, `MODEL_PORT_ALREADY_IN_USE`, `MODEL_SERVER_START_FAILED`, `MODEL_SERVER_READY_TIMEOUT`, `MODEL_SERVER_UNSUPPORTED_FLAGS`
- **Job-based**: No

### `stop_model_server`
- **Input**: `ProfileSelectionInput { profileId?: string }`
- **Output**: `StopModelServerOutput`
- **Errors**: `MODEL_SERVER_STOP_FAILED`, `MODEL_SERVER_NOT_STARTED_BY_APP`
- **Job-based**: No

### `set_model_mode`
- **Input**: `SetModelModeInput { profileId?: string, mode: "external" | "managed" }`
- **Output**: `ModelStatus`
- **Errors**: `MODEL_PROFILE_NOT_FOUND`
- **Job-based**: No

### `reset_model_profile`
- **Input**: None
- **Output**: `ModelStatus`
- **Errors**: `MODEL_SERVER_START_FAILED`
- **Job-based**: No

### `preview_model_server_args`
- **Input**: `ProfileSelectionInput { profileId?: string }`
- **Output**: `ModelServerArgsPreview`
- **Errors**: `MODEL_SERVER_START_FAILED`, `MODEL_SERVER_UNSUPPORTED_FLAGS`
- **Job-based**: No

## Diagnostic CLI
- `rubrika doctor <project_path>`
- `rubrika doctor <project_path> --json`
- `rubrika inspect project <project_path>`
- `rubrika inspect jobs <project_path>`
- `rubrika inspect model`
- `rubrika inspect documents <project_path>`
- `rubrika inspect document-content <project_path>`
- `rubrika inspect question-text <project_path>`
- `rubrika inspect rubric <project_path>`
- `rubrika inspect model-inputs <project_path>`
- `rubrika replay rubric-import <project_path> --dry-run`
- `rubrika replay question-text <project_path> --dry-run`

## Model Input Pipeline
- PDF preview cache remains under `cache/page_previews`
- Shared document content artifacts live under `cache/document_content/<document_id>`
- Model input cache is written under `cache/model_inputs`
- Question text extraction and rubric vision fallback use optimized JPEG model inputs
- `inspect document-content` should report `method`, `raw_text_length`, `normalized_text_length`, `enough_text`, `vision_fallback_needed`, question coverage markers, and artifact paths
- Request diagnostics should include prompt length, image count, image bytes, and base64 size estimates

*Note: Some commands listed above are explicitly planned and may not be fully implemented yet.*

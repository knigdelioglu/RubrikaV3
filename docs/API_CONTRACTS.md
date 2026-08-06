# API Contracts

This document defines the Tauri commands that bridge the React frontend with the Rust backend in Rubrika v3.

## Core Principles
- All commands are typed (input, success output, error output).
- Long-running operations must be implemented as jobs, not synchronous commands.
- Errors must use the `AppError` structure defined in `ERROR_CODES.md`.

## Project path security contract

`create_project` ve `open_project` backend tarafından `TrustedProjectRoot` ile canonicalize edilir. `project.json.root_path` yalnız metadata'dır; runtime save root'u değildir. Açılan root ile stored metadata uyuşmazlığı load failure değildir, ancak warning olarak raporlanır.

Managed document/artifact inputs frontend'den absolute path kabul etmez; command yalnız `project_id` + domain ID veya managed relative path kullanır. Backend trusted root üzerinden containment, regular-file ve symlink kontrollerini tekrarlar. `file_path` alanı yalnız kullanıcının açıkça seçtiği external import source veya export destination sözleşmelerinde bulunabilir.

## Commands

### Assessment organization contract

`list_assessment_classes` is the shared class-selection read for written, listening and speaking activities. It returns only active classes matching the academic year, course, grade and active teaching assignments. `create_assessment_activity` creates one `AssessmentActivity` and one `ClassApplication` per selected class; duplicate main keys, duplicate activity/class links, ineligible classes, and mixed-grade selections are rejected. Speaking setup writes the activity directly; it does not create a new independent speaking class list. `attach_assessment_document` uses no application ID for common documents and an application ID for class-specific documents.

The canonical UI ownership is `/project/:projectId/classes` for `create_teaching_assignment` and `/project/:projectId/activities` for assessment activity commands. Assessment create mode has no free-form course, academic-year, or grade inputs.

Commands: `list_assessment_activities`, `get_assessment_sequence_options`, `get_assessment_activity`, `set_active_written_activity`, `list_assessment_classes`, `create_assessment_activity`, `update_assessment_activity`, `get_assessment_class_applications`, `get_class_application_students`, `add_assessment_class_application`, `archive_assessment_class_application`, `remove_assessment_class_application`, `attach_assessment_document`, `list_teaching_assignments`, `create_teaching_assignment`, `archive_teaching_assignment`.

`assessment_type` values are `written`, `listening`, `speaking`, `performance`; workflow families are derived as `written`, `written`, `speaking`, `performance` respectively. All organization errors are structured `AppError` values.

`set_active_written_activity` (`{ projectId, activityId }` → `AssessmentActivity`) is the TD-01 backend-authoritative written-scope selector. It persists `Project.activeWrittenAssessmentActivityId` so the project-level written collections (`questions`, `studentSubmissions`, OCR records, `scoringRecords`, freeze) are read and written against the selected written/listening activity. Non-written activities return `ASSESSMENT_INVALID_INPUT`; an unknown activity returns `ASSESSMENT_ACTIVITY_NOT_FOUND`. Creating a written/listening activity auto-selects it as active. Entering a written/listening workspace (`/project/:projectId/activities/:assessmentActivityId/:step`) calls this command so data never leaks across written exams in the same project.

Canonical Assessment Workspace routes are `/project/:projectId/activities/:assessmentActivityId/:step`. Canonical steps for `written` (`prep`, `students`, `ocr`, `scoring`, `results`), `listening` (`listening_content`, `questions`, `students`, `ocr_scoring`, `results`), `speaking` (`settings`, `students`, `transcript`, `evaluation`, `results`), and `performance` (`task`, `assessment`, `results`) present type-specific steps in the main workspace content shell without changing the global navigation. Step readiness is calculated exclusively from backend `WorkflowSnapshot` and attempt states.

Canonical speaking execution commands are `start_speaking_exam_attempt`, `toggle_speaking_capture`, `select_speaking_exam_class`, `select_speaking_exam_student` and `get_speaking_exam`. Canonical input carries `assessmentActivityId` and `classApplicationId`; `classId` alone is only a legacy adapter input and is not used by the new UI. `start_speaking_exam_attempt` rejects missing activity/application, unrelated applications, archived applications and students outside the selected SchoolClass roster.

`SpeakingAttempt` is persisted under `AssessmentActivity.classApplications[*].speakingAttempts` with activity, application and school-class references plus the speaking configuration snapshot. `SpeakingExam` remains an in-memory/runtime compatibility projection for the existing audio service.

### Speaking evaluation contract

Speaking attempts persist `rawTranscript`, segment-preserving cleanup candidate/status, `transcriptForScoring`, model provenance, frozen prompt/policy versions and `evaluationInputHash`. Cleanup failures return a reviewable attempt and do not create a scoring transcript. Speaking model output contains subindicator level, positive/counter evidence IDs, missing requirements and rationale; backend validates evidence and computes whole-point criterion and total scores.

`SpeakingSubindicatorScore` exposes `selectedLevelId`, `appliedLevelId`, `points`, positive/counter evidence IDs, missing requirements and optional ceiling reason/explanation. `selectedLevelId` is immutable model provenance; UI uses `appliedLevelId` for the effective contribution.

`SpeakingMetrics` exposes recording/active speech durations, words per minute, silence/long pause/filler/repetition counts, duration tier, expected minimum duration, `sampleDurationSufficient` and `measurementConfidence`. A low-confidence short sample requires teacher review and is not automatic zero.

### Performance değerlendirme sözleşmesi

Performans görevi, yazılı sınav akışından (PDF/OCR/QEP) bağımsız ayrı bir akıştır. `ScoringRecord` ve yazılı sınav puanlama/rapor kayıtları kullanılmaz. Organizasyon (`academicYearId + courseId + gradeLevel + term + assessmentType + sequenceNumber`), sınıf uygulaması ve öğrenci doğrulaması ortak `AssessmentOrganizationService`/`SchoolClassService` üzerinden yapılır.

Görev ve rubrik:

- `create_performance_task`: input `CreatePerformanceTaskInput { projectId, academicYearId, courseId, courseName, gradeLevel, term, sequenceNumber, schoolClassIds, title?, performanceDetails, initialRubric? }`. `initialRubric` sürüm 0 taslak olarak kaydedilir; yoksa boş taslak oluşturulur. Output `AssessmentActivity`. Errors: `ASSESSMENT_ACTIVITY_ALREADY_EXISTS`, `ASSESSMENT_CLASS_NOT_ELIGIBLE`, `ASSESSMENT_CLASS_LEVEL_MISMATCH`, `PROJECT_NOT_FOUND`.
- `update_performance_task`: input `UpdatePerformanceTaskInput { projectId, activityId, title?, performanceDetails? }`. Yalnız görev bilgilerini günceller; rubrik sürümlerine dokunmaz.
- `list_performance_tasks`: input `{ projectId, courseId?, term?, schoolClassId? }`. Output `AssessmentActivity[]` (yalnız `performance` türü).
- `get_performance_task`: input `{ projectId, activityId }`. Output `AssessmentActivity`.

Rubrik sürümleme (K8):

- `publish_performance_rubric`: input `PublishPerformanceRubricInput { projectId, activityId, rubric }`. Doğrulama: 3-6 ölçüt, 3 veya 5 düzey, her ölçüt adı/açıklaması, her düzeyde gözlenebilir tanım, düzey puanları azalan ve benzersiz. Yayın = yeni sürüm (>= 1); onaylı değerlendirmesi olan rubrik kilitlidir (yeni sürüm bile yayınlanamaz). Output yayınlanan `PerformanceRubric`.
- `get_performance_rubric_history`: input `{ projectId, activityId }`. Output `PerformanceRubric[]` (sürüm 0 dahil tüm sürümler).

Değerlendirme (K3/K9):

- `save_performance_assessment`: input `SavePerformanceAssessmentInput { projectId, activityId, applicationId, studentId, assessmentId?, ratings?, feedback? }`. Kayıt `ClassApplication.performanceAssessments` altında canonical saklanır. Geçici toplam (`provisionalTotal`) yalnız servis tarafından hesaplanır; istemci girdisine güvenilmez. Yayınlanmış rubrik yoksa reddedilir. Onaylanmış kayıt düzenlenemez.
- `approve_performance_assessment`: input `{ projectId, activityId, applicationId, assessmentId }`. Tüm ölçütler değerlendirilmeden onay reddedilir; onay tarihi ve rubrik sürümü kayda sabitlenir. Onay sonrası düzenleme reddedilir.
- `set_performance_assessment_status`: input `{ projectId, activityId, applicationId, studentId, assessmentId?, status }`; `status` yalnız `missing` veya `not_performed` olabilir. Bu kayıtlara sıfır puan yazılmaz, toplam hesabına girmez; raporda ayrı gösterilir. Onaylanmış kaydın durumu değiştirilemez.
- `list_performance_assessments`: input `{ projectId, activityId, applicationId? }`. Output `PerformanceAssessment[]`.

Sonuç raporu (Faz C):

- `get_performance_report`: input `{ projectId, activityId, applicationId }`. Output `PerformanceReportDto`: görev metadata'sı (başlık, ders, sınıf adı, dönem/sıra, tema, beceri alanı, çalışma biçimi, öğretmen kimliği, rubrik adı/sürümü), `criteria`/`levels` (görüntü rubriği = yayınlanmış en yeni sürüm), `maxPoints`, `generatedAt`, `summary` (öğrenci/değerlendirilen/onaylı/eksik/gösterilmedi/değerlendirilmeyen sayaçları) ve `rows`. Her satır öğrencinin kendi sabitlediği rubrik sürümüyle çözülür; `Missing`/`NotPerformed`/değerlendirilmeyen öğrencilerde ölçüt puanları ve toplam `null` kalır — sıfırla karıştırılmaz. Errors: `ASSESSMENT_ACTIVITY_NOT_FOUND`, `ASSESSMENT_CLASS_APPLICATION_NOT_FOUND`, `RUBRIC_MISSING`, `STUDENT_NOT_FOUND`. Job-based: Hayır (salt-okunur).

PDF/Excel çıktıları bu DTO üzerinden `PerformanceResultsView`'da üretilir: PDF için yazdırma görünümü (`window.print()`), Excel için noktalı virgül ayraçlı UTF-8 CSV (BOM ile). AI hiçbir noktada puan üretmez; rapordaki veriler öğretmen kararlarından gelir.

### Job commands contract
- `list_jobs`: Inputs `{ projectId: String, projectRootPath: Option<String> }`. Rehydrates persisted jobs and returns list of jobs.
- `get_job_snapshot`: Inputs `{ jobId: String }`. Returns `JobSnapshot`.
- `cancel_job`: Inputs `{ jobId: String }`. Sets `cancellation_requested = true`, signals Tokio cancellation token, cleans up active lease/staging state, and returns updated `JobSnapshot`.
- `retry_job`: Inputs `{ jobId: String }`. Validates target terminal job (`failed`, `cancelled`, `interrupted`), registers new job with fresh `job_id`/`correlation_id`, preserves `retry_of_job_id`, and returns new `JobSnapshot`.
- `cleanup_job_history`: Inputs `{ projectRootPath: String, maxTerminalJobs: Option<usize> }`. Cleans up terminal jobs exceeding `max_terminal_jobs` while protecting active jobs and retry-chain referenced jobs; returns `RetentionStats`.

### Job cancellation & correlation ID contract (Phase 5C)
- **Cancellation Checkpoints**: Every long-running job service periodically queries `cancellation_token.is_cancelled()`. On cancellation, active staging files and uncommitted draft models are discarded without modifying teacher-confirmed or active production data.
- **Correlation ID Chain**: `correlation_id` passed at command invocation propagates identically across `JobRegistrationInput` $\rightarrow$ `JobSnapshot` $\rightarrow$ `ModelRuntimeLease` $\rightarrow$ `ModelGateway Request` $\rightarrow$ `ProjectStore Commit Event` $\rightarrow$ `Tauri Job Event`.

### `get_app_status`
- **Input**: None
- **Output**: `AppStatus` (includes version, platform, config paths)
- **Errors**: None expected
- **Job-based**: No

### `create_project`
- **Input**: `CreateProjectInput { name: String, root_path: String, academic_year_id: Option<String>, course_id: Option<String>, course_name: Option<String> }`
- **Output**: `CreateProjectOutput { project: ProjectSnapshot, project_path: String, warnings: String[] }`
- **Errors**: `PERMISSION_DENIED`, `PROJECT_SAVE_FAILED`, `PROJECT_ALREADY_EXISTS`, `PROJECT_DIRECTORY_NOT_EMPTY`
- **Job-based**: No
- **Path rule**: Hedef klasör yoksa oluşturulur; mevcut klasör boş değilse veya `project.json` içeriyorsa backend reddeder ve sessiz overwrite yapmaz.

### `get_default_project_path`
- **Input**: `{ project_name: String, academic_year_id: Option<String> }`
- **Output**: `{ path: String }`
- **Errors**: `PROJECT_SAVE_FAILED` when the platform path cannot be resolved
- **Job-based**: No
- **Path rule**: Eğitim yılı varsa güvenli varsayılan klasör adı `<project_name>_<academic_year_id>` olur. Aynı yıl ve ad zaten kullanılıyorsa `_2`, `_3` gibi çakışma eki eklenir. Yıl bilgisi olmayan eski çağrılar mevcut adlandırma kuralını korur.

### `open_project`
- **Input**: `OpenProjectInput { path: String }`
- **Output**: `ProjectSnapshot`
- **Errors**: `PROJECT_NOT_FOUND`, `PROJECT_LOAD_FAILED`, `MANAGED_PATH_SYMLINK_ESCAPE`
- **Job-based**: No
- **Path rule**: `path` kullanıcı tarafından seçilen proje klasörü veya o klasördeki `project.json` olabilir. Stored `root_path` runtime root'u değiştiremez; taşınmış projeler yeni canonical konumundan açılır.

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
- **Errors**: `DOCUMENT_IMPORT_FAILED`, `PDF_RENDER_FAILED`, `UNSAFE_MANAGED_PATH`, `MANAGED_PATH_SYMLINK_ESCAPE`
- **Job-based**: Yes (triggers PDF rendering job)
- **Path rule**: `file_path` açıkça seçilmiş external source'tur. Import sonrası `Document.storedPath` yalnız `documents/...` relative managed kopyadır.

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
- **Errors**: `DOCUMENT_IMPORT_FAILED`, `PDF_RENDER_FAILED`, `UNSAFE_MANAGED_PATH`, `MANAGED_PATH_SYMLINK_ESCAPE`
- **Job-based**: Yes (triggers PDF rendering job)

### `import_student_scan_pdf`
- **Input**: `ImportPdfInput { project_id: String, file_path: String }`
- **Output**: `DocumentSnapshot`
- **Errors**: `DOCUMENT_IMPORT_FAILED`, `PDF_RENDER_FAILED`, `UNSAFE_MANAGED_PATH`, `MANAGED_PATH_SYMLINK_ESCAPE`
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
- **Errors**: `STUDENT_SUBMISSION_NOT_FOUND`, `STUDENT_SUBMISSION_IN_USE`, `SUBMISSION_DELETE_CONFLICT`
- **Job-based**: No
- **Safety**: OCR generation/history, OCR review, scoring, artifact veya running job referansı varsa metadata ve dosya silinmez.

### `mark_student_grouping_complete`
- **Input**: `MarkStudentGroupingCompleteInput { project_id: String }`
- **Output**: `Project`
- **Errors**: `STUDENT_SCAN_NOT_FOUND`, `STUDENT_SCAN_PREVIEW_NOT_READY`, `STUDENT_IDENTITY_INVALID`
- **Job-based**: No

### `save_student_answer_crop_template`
- **Input**: `SaveStudentAnswerCropTemplateInput { project_id: String, templates: QuestionAnswerTemplate[] }`
- **Output**: `Project`
- **QuestionAnswerTemplate**: `{ questionId, regions[] }`; each region carries `regionId`, `pageOffset`, deterministic `order`, `normalizedBBox`, `regionRole`, and `continuationPolicy`.
- **Persistence rule**: UI may keep an unsaved rectangle locally; only this backend command changes the canonical template and recalculates workflow/crop coverage.
- **Migration**: legacy `studentAnswerCropTemplate.items[]` entries are migrated losslessly into one `regions[0]` entry per item. Region order is normalized before persistence and is preserved in OCR input metadata.
- **Errors**: `PROJECT_NOT_FOUND`, `CROP_REGION_MISSING`, `PROJECT_MUTATION_CONFLICT`
- **Job-based**: No

### `get_ocr_readiness`
- **Input**: `ProjectIdInput { project_id: String }`
- **Output**: `StudentScanReadinessSnapshot`
- **OCR policy**: `ocrReviewPolicy { version, fingerprint, lowConfidenceThreshold, criticalConfidenceThreshold, reasonLabels }` is backend-authoritative and must be rendered by the UI; the UI does not recalculate review readiness.
- **Errors**: `PROJECT_NOT_FOUND`
- **Job-based**: No

### `start_student_answer_ocr`
- **Input**: `ProjectIdInput { project_id: String, force_rerun?: boolean, mode?: "production" | "experimental_full_page_review_only" }`
- **Output**: `StartStudentAnswerOcrOutput { job_id: String, status: "queued", rerun: boolean, mode }`
- **Production gate**: `mode=production` requires at least one saved answer region for every question. Missing coverage returns structured `CROP_REGION_MISSING`/`OCR_NOT_READY`; there is no production full-page fallback.
- **Experimental mode**: `experimental_full_page_review_only` is explicitly typed and review-only. Its records carry `needsReview=true`, `ocrProvenance.approvableForScoring=false`, and cannot be teacher-approved, accepted as scoring-ready OCR, or pass the scoring gate. It may be used only as a teacher text-correction reference.
- **OCR provenance**: successful attempts record source checksum/pages, region IDs/order/page offsets, renderer/DPI (or explicit unknown), preprocess policy/variant/version, final prepared model image dimensions/cache keys, invocation contract, input budget, and response diagnostics. Missing legacy metadata remains unknown.
- **Structured answer**: OCR uses a tagged `StructuredAnswer` union (`multiple_choice`, `matching`, `ordered_slots`, `numeric`, `table`, `correction_table`, `sentence_annotation`, `grammar_analysis`, `open_text`). The backend maps each `AnswerType` to its allowed variant. Mismatch, invalid schema, or placeholder values remain reviewable data and cannot produce an applied score; legacy arbitrary JSON is retained as `legacy_unparsed` salvage.
- **Errors**: `WORKFLOW_BLOCKED`, `CROP_REGION_MISSING`, `OCR_NOT_READY`, `STUDENT_SCAN_NOT_FOUND`, `STUDENT_SCAN_PREVIEW_NOT_READY`, `STUDENT_GROUPING_NOT_READY`, `QUESTION_TEXT_MISSING`, `MODEL_SERVER_START_FAILED`, `MODEL_SERVER_READY_TIMEOUT`
- **Job-based**: Yes
- **Notes**: If the model server is not healthy yet, the backend starts the managed model process, waits for `/health`, then queues OCR. Teacher-facing errors stay structured; technical details remain in diagnostics.

### `accept_student_answer_ocr_generation`
- **Input**: `OcrGenerationInput { project_id: String, generation_id: String }`
- **Output**: `OcrGeneration`
- **Errors**: `OCR_GENERATION_CONFLICT`, `PROJECT_ENTITY_NOT_FOUND`, `PROJECT_MUTATION_CONFLICT`
- **Job-based**: No
- **Safety**: Active pointer transaction içinde değiştirilir; eski generation history'de kalır ve bağlı scoring invalidated olur.

### `reject_student_answer_ocr_generation`
- **Input**: `OcrGenerationInput { project_id: String, generation_id: String }`
- **Output**: `OcrGeneration`
- **Errors**: `OCR_GENERATION_CONFLICT`, `PROJECT_ENTITY_NOT_FOUND`
- **Job-based**: No
- **Safety**: Candidate rejected olur; mevcut active OCR projection'ı korunur.

### Preview generation safety

`start_pdf_preview_render` staging generation üretir. `PdfPreviewState.activeGenerationId` yalnız tüm sayfalar regular file, page count, manifest, source fingerprint ve trusted-root kontrollerinden sonra commit edilir. `PREVIEW_GENERATION_FAILED`, `PREVIEW_GENERATION_STALE` veya `PREVIEW_ACTIVE_GENERATION_MISSING` teacher-facing güvenli preview mesajlarına map edilir; UUID/path UI'a sızdırılmaz.

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
- **Input**: `UpdateQuestionRubricInput { project_id: String, question_id: String, answer_type?: AnswerType, max_score?: number, expected_answer?: string, key_concepts: string[], criteria: RubricCriterion[], partial_credit_hints: string[], zero_score_conditions: string[], common_mistakes: string[] }`
- **Output**: `Question`
- **Errors**: `RUBRIC_NOT_READY`, `RUBRIC_QUESTION_NOT_FOUND`, `RUBRIC_MAX_SCORE_MISSING`, `RUBRIC_CRITERIA_SCORE_MISMATCH`, `RUBRIC_PLACEHOLDER_DETECTED`
- **Job-based**: No
- **Notes**: `answer_type` is persisted in the canonical question model and invalidates a previously frozen package, so OCR cannot depend on screen-local question-type state.

### Rubric extraction contract
- **Version**: `rubric_extraction_contract_v2`.
- **Schema source**: JSON Schema and prompt field list are built from canonical `RubricState` DTO data fields: `expectedAnswer`, `keyConcepts`, `criteria`, `partialCreditHints`, `zeroScoreConditions`, `commonMistakes`, `warnings`, and score data. `source`, `status`, and `updatedAt` are backend-owned boundaries and are not model-authored.
- **Authority**: model output is persisted as `source=gemma_draft`, `status=suggested`; teacher confirmation is required before it can become authoritative. Placeholder detection remains active.

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

### `migrate_rubric_levels`
- **Input**: `MigrateRubricLevelsInput { project_id: String, question_id?: String }`
- **Output**: `MigrateRubricLevelsOutput { migratedCount, teacherConfirmationRequired, qepInvalidated, warnings[] }`
- **Errors**: `PROJECT_NOT_FOUND`, `PROJECT_SAVE_FAILED`
- **Job-based**: No
- **Invariant**: Eski numeric/max-only kriter verisi silinmez; oluşturulan seviyeler `suggested` durumunda kalır ve öğretmen onayı olmadan authoritative scoring policy sayılmaz. Frozen QEP değişiklikte invalidated olur.

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
- **Lifecycle**: `decisionState` separates `modelCandidate`/`provisional`, `deterministicAccepted`, `autoAccepted`, `teacherApproved`, `rejected`, and `failed`. `finalScore` consumes only teacher-approved final records.
- **Semantic scoring**: Model output carries criterion/level/evidence proposals; Rust maps frozen rubric levels to scores. Direct model score fields are ignored and recorded as review diagnostics.
- **Reproducibility**: `scoringFingerprint` is an exact QEP/answer/OCR/prompt/schema/policy/model/runtime/sampling/calibration/anchor identity. Candidate cache hits and exact duplicate reuse expose provenance and do not turn a reviewable proposal into a final decision.

### `update_scoring_record`
- **Input**: `UpdateScoringRecordInput { project_id: String, record_id: String, teacher_manual_score?: f32, teacher_notes?: String, teacher_approved: bool }`
- **Output**: `ScoringRecord`
- **Errors**: `SCORING_NOT_READY`
- **Job-based**: No
- **Notes**: A record with `scoringApplied=false` cannot be approved until the teacher supplies a manual score.

### `get_scoring_summary`
- **Input**: `GetScoringSummaryInput { project_id: String }`
- **Output**: `ScoringSummaryDto { provisionalScore, acceptedScore, finalScore, maxScore, isComplete, submissions[] }`
- **Errors**: `PROJECT_NOT_FOUND`
- **Job-based**: No
- **Notes**: `finalScore` excludes provisional, review-required, rejected and failed records. The frontend displays this backend summary and never calculates a student or project total.

### `list_scoring_anchors`
- **Input**: `ListScoringAnchorsInput { project_id: String }`
- **Output**: `ScoringAnchorDto[]`
- **Errors**: `PROJECT_NOT_FOUND`
- **Job-based**: No
- **Notes**: Each DTO includes the immutable anchor version/source record, QEP/question/rubric/policy fingerprints, teacher action history, evidence hashes and a backend-derived `eligibility` (`eligible`, `stale`, `ineligible`, `revoked`) with teacher-facing reasons.

### `create_scoring_anchor`
- **Input**: `CreateScoringAnchorInput { project_id: String, source_record_id: String }`
- **Output**: `ScoringAnchorDto`
- **Errors**: `QEP_NOT_FROZEN`, `SCORING_ANCHOR_NOT_ELIGIBLE`, `SCORING_ANCHOR_ALREADY_EXISTS`, `SCORING_ANCHOR_NOT_FOUND`
- **Job-based**: No
- **Invariant**: Only a teacher-approved, final, placeholder-free scoring decision with an approved OCR source and current QEP/policy can be anchored. Model proposals, reviewable records and zero-score fallbacks are rejected. The ProjectStore mutation is atomic and the action is written to the audit log.

### `revoke_scoring_anchor`
- **Input**: `RevokeScoringAnchorInput { project_id: String, anchor_id: String, reason?: String }`
- **Output**: `ScoringAnchorDto`
- **Errors**: `SCORING_ANCHOR_NOT_FOUND`, `SCORING_ANCHOR_ALREADY_REVOKED`
- **Job-based**: No
- **Invariant**: Revocation preserves the immutable anchor version/evidence and appends a teacher action; it does not delete historical anchor data. Rubric/QEP/policy changes similarly leave the record present but make it stale/ineligible through the backend read model.

### `finish_assessment`
- **Input**: `FinishAssessmentInput { project_id: String, kind: "written" | "speaking", source_id?: String }`
- **Output**: `FinishAssessmentOutput { analysis_id: String, job_id: String, status: "queued" }`
- **Errors**: `ANALYSIS_NOT_READY`, `PROJECT_NOT_FOUND`
- **Job-based**: Yes (`assessment_analysis`)
- **Notes**: Deterministic charts and a canonical aggregate `metrics[]` registry are saved before the Gemma job starts. The model receives only anonymous aggregate metrics, never raw student answers or student read-model records. Structured claims are resolved against that registry; missing or contradictory references become teacher-review/unsupported claims. Report failure produces a partial analysis and never removes chart data.

### `get_assessment_analysis`
- **Input**: `GetAssessmentAnalysisInput { project_id: String, analysis_id: String }`
- **Output**: `AssessmentAnalysis { metrics[], claims[] }` where each claim has `claim`, `metricRefs`, `recommendation`, `evidenceStatus` and `teacherVisibleExplanation`
- **Errors**: `ANALYSIS_FAILED`, `PROJECT_NOT_FOUND`
- **Job-based**: No
- **Compatibility**: Legacy analysis files without `metrics`/`claims` are opened without data loss; the backend derives the aggregate metric registry and does not treat the legacy free-text report as verified evidence.

### `list_assessment_analyses`
- **Input**: `ListAssessmentAnalysesInput { project_id: String }`
- **Output**: `AssessmentAnalysis[]`
- **Errors**: `ANALYSIS_FAILED`, `PROJECT_NOT_FOUND`
- **Job-based**: No

### Speaking roster and review commands
- `list_class_students` reads the canonical common class roster used by written, listening and speaking exams.
- `create_class_student` registers a student before any exam; active class applications for that class receive the new student in their persisted scope.
- `update_class_student` edits the name or school number without creating a second student record.
- The class roster UI is `/project/:projectId/students?tab=roster`; PDF grouping and identity OCR remain separate operational tabs.
- `select_speaking_exam_class` and `select_speaking_exam_student` persist the resumable session location.
- `update_speaking_criterion_level` accepts `very_good | good | moderate | developing` for teacher-only criteria and maps the level to a bounded score in Rust.
- `update_speaking_attempt_note` persists the teacher note without waiting for final approval.
- `approve_speaking_attempt` atomically saves the score, then permanently deletes the temporary WAV.

### `get_model_status`
- **Input**: None
- **Output**: `ModelStatus` including optional `privacyMode`, `privacyBlocked`, `privacyBlockReason`, and `modelFingerprint` fields.
- **Errors**: None for a blocked legacy profile; the blocked state carries a friendly suggested action.
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
- **Output**: `StopModelServerOutput { stopped: boolean, draining: boolean, activeLeaseCount: number, message: string }`
- **Errors**: `MODEL_SERVER_STOP_FAILED`, `MODEL_PROCESS_UNVERIFIED`, `MODEL_PROCESS_IDENTITY_MISMATCH`, `MODEL_RUNTIME_IN_USE`
- **Job-based**: No

`stop_model_server` aktif lease varken process'i doğrudan sonlandırmaz. Coordinator
`Draining` durumuna geçer ve mevcut lease'ler release edilene kadar bekler.

### `acquire_ready_runtime_lease` / `release_runtime` (backend service contract)
- **Owner**: `ModelProcessManager` üzerinden `ModelRuntimeService`
- **Acquire**: `profile`, `consumer`, `operation`, `correlationId`
- **Grant**: lease ID, verified runtime instance ID, profile fingerprint, optional model fingerprint, correlation ID ve verified base URL
- **Readiness**: bounded `/health` + process identity; completion probe yalnız explicit manual probe/doctor/benchmark akışındadır.
- **Release**: yalnız aynı lease ID + runtime instance ID; idempotent public guard
- **Errors**: `MODEL_RUNTIME_DRAINING`, `MODEL_RUNTIME_PROFILE_BUSY`, `MODEL_RUNTIME_START_FAILED`, `MODEL_RUNTIME_READINESS_TIMEOUT`, `MODEL_RUNTIME_LEASE_INVALID`, `MODEL_RUNTIME_LEASE_ALREADY_RELEASED`, `MODEL_PRIVACY_BLOCKED`
- **Invariant**: lease release'i başka job'ın runtime'ını durdurmaz; son lease sonrası idle shutdown uygulanır.

### `set_model_mode`
- **Input**: `SetModelModeInput { profileId?: string, mode: "external" | "managed" }`
- **Output**: `ModelStatus`
- **Errors**: `MODEL_PROFILE_NOT_FOUND`, `MODEL_EXTERNAL_CONSENT_REQUIRED` when switching to external without explicit consent.
- **Job-based**: No

### `enable_external_model`
- **Input**: `EnableExternalModelInput { profileId?: string, projectRootPath?: string, confirmExternalDataTransfer: boolean }`
- **Output**: `ModelStatus`
- **Errors**: `MODEL_EXTERNAL_CONSENT_REQUIRED`, `MODEL_PROFILE_NOT_FOUND`, `AUDIT_WRITE_FAILED`
- **Job-based**: No
- **Invariant**: This is the only external-privacy opt-in path. The command persists `ExplicitExternal`, requires the explicit confirmation field, and appends a project/application audit event containing no student content.

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
- `rubrika preflight <project_path>`
- `rubrika --json preflight <project_path>`

`preflight` returns `DataLossPreflightReport` and is strictly read-only. It does
not run migration, recovery, repair, audit append, import, delete, GC or
restore. `decision` is one of `SAFE_TO_OPEN`, `SAFE_TO_OPEN_WITH_BACKUP`, or
`DO_NOT_OPEN_FOR_WRITING`; the last value is the only valid result when
project parsing, symlink, active-pointer, audit-chain, or backup verification
fails.

## Model Input Pipeline
- Every model request carries a versioned `PromptContract` at the Rust gateway boundary: immutable system policy, typed serialized user data, and `ModelInvocationContract` provenance (`useCase`, `promptVersion`, `schemaVersion`, `policyVersion`, `modelFingerprint`, `runtimeFingerprint`, `samplingParameters`, and optional `responseFormat`).
- OCR user data contains question structure and observed student content only; rubric, expected answer, key concepts, partial-credit rules and zero-score conditions are excluded. OCR issue correction is limited to observed text, crop/location and image-quality data; any contextual suggestion remains review-only.
- Backend schema/domain validation remains mandatory. Raw model response text is diagnostic artifact data and is not part of the teacher-facing result contract.
- PDF preview cache remains under `cache/page_previews`
- Shared document content artifacts live under `cache/document_content/<document_id>`
- Model input cache is written under `cache/model_inputs`
- Question text extraction and rubric vision fallback use optimized JPEG model inputs
- Model input JPEG cache keys include source hash, ordered crop regions, alignment/preprocess/resize policy, JPEG quality, and encoder version; cache publication and manifest publication are atomic and transaction-tagged.
- `inspect document-content` should report `method`, `raw_text_length`, `normalized_text_length`, `enough_text`, `vision_fallback_needed`, question coverage markers, and artifact paths
- Request diagnostics should include prompt length, image count, image bytes, and base64 size estimates

*Note: Some commands listed above are explicitly planned and may not be fully implemented yet.*

## Faz 6 — security & maintenance commands

```text
start_backup_job({ projectId }) -> { jobId, status }            # JobKind: project_backup
start_restore_job({ archivePath, destinationPath }) -> { jobId, status } # JobKind: project_restore
run_generation_gc({ projectId, dryRun? }) -> GcReport           # protected/candidates/deleted/deferred/orphanStaging
get_workflow_snapshot({ projectId }) -> WorkflowSnapshotDto     # explicit DTO (F1)
```

Error contract: bütün komut hataları `PublicErrorDto` olarak serializelenir
(`code`, `safeMessage`, `recoveryAction`, `correlationId`, `retryable`,
`detailsAvailable`). `AppError.technical_details` Tauri sınırını geçmez.
Yeni kodlar: `MODEL_RESPONSE_TOO_LARGE`, `MODEL_RESPONSE_TRUNCATED`,
`MODEL_RESPONSE_INVALID_CONTENT_TYPE`, `MODEL_REQUEST_TOO_LARGE`,
`PROJECT_ALREADY_OPEN`, `PROJECT_WRITE_LEASE_MISSING`, `AUDIT_WRITE_FAILED`,
`AUDIT_CHAIN_INVALID`, `BACKUP_FAILED`, `BACKUP_ARCHIVE_INVALID`,
`BACKUP_CANCELLED`, `RESTORE_FAILED`, `RESTORE_DESTINATION_CONFLICT`,
`RESTORE_CANCELLED`, `GENERATION_GC_FAILED`, `APP_ALREADY_RUNNING`.

## Final pre-use data-loss contract

`DataLossPreflightReport` includes `readOnly`, `projectParseOk`, recursive
inventory SHA-256, symlink paths, missing active-pointer count, orphan/staging
counts, audit-chain report, verified/failed backups, warnings/errors and the
write decision. The report is diagnostic evidence, not a write authorization
override. Full proof and release status are maintained in
[`FINAL_PRE_USE_DATA_LOSS_AUDIT.md`](FINAL_PRE_USE_DATA_LOSS_AUDIT.md).
## Faz 2 — ProjectStore concurrency contract

## Integrity recovery commands

rubrika backup-create project_path [--destination external_dir]
rubrika backup-verify archive_path [--source-project project_path]
rubrika restore-copy archive_path new_destination
rubrika verify-restore archive source_project restored_path [--proof-path path]
rubrika audit-forensics project_path
rubrika classify-orphans project_path
rubrika recover-copy archive new_destination [--source-project path] [--dry-run]
rubrika recovery-diff source_project candidate_path
rubrika preflight project_path

recover-copy rejects the source path, existing destinations, traversal,
symlinks and unverified archives. dry-run performs no recovery writes. The
Tauri start_recovery_copy_job command carries the same sourceProjectPath,
backupPath, destinationPath and dryRun contract and uses the project_recovery
job kind.

DataLossPreflightReport includes source byte changes, original/active audit
status, recovery-anchor status, active revision divergence, ambiguous
transactions, verified backup path/hash/restore status, and the three real
destructive-proof statuses. The frontend displays these as teacher-facing
labels and never turns a failed preflight into general write authorization.

`initializationWriteAllowed` is a narrow exception for a pristine project at
storage revision zero with no documents, questions, classes, students,
activities, submissions, OCR/scoring data, orphan/staging artifacts, or audit
problems. It permits only the first setup writes; the normal `DO_NOT_OPEN_FOR_WRITING`
decision and full backup/validation gates remain in force once real project
data exists.

For the 11_46 closure, `verifiedBackupRestoreStatus=PASS` while
`fullTestSuiteGreen=false`; the backend decision remains
`DO_NOT_OPEN_FOR_WRITING` and the frontend must keep project writes disabled.

`Project` JSON'unda backend-authoritative `storageRevision` alanı bulunur. Alanı olmayan legacy projeler `0` revision ile yüklenir; salt açılış rewrite yapmaz. Başarılı canonical mutation revision'ı tam bir kez artırır.

`UpdateCourseInfoInput` gibi kısa mutation DTO'ları isteğe bağlı `expectedRevision` taşıyabilir. Frontend revision üretemez veya authoritative olarak overwrite edemez; backend güncel entity'yi ID ile bulur.

Backend içi persistence sonuçları:

- `ProjectSnapshot { project, revision, contentFingerprint, trustedRoot }`
- `MutationOutput<T> { result, snapshot }`
- `JobCommitResult<T>`: `Applied`, `Stale`, `Conflict`, `EntityMissing`, `Rejected`

Typed conflict kodları `PROJECT_REVISION_CONFLICT`, `PROJECT_EXTERNALLY_MODIFIED`, `PROJECT_MUTATION_CONFLICT`, `PROJECT_ENTITY_STALE`, `PROJECT_ENTITY_NOT_FOUND` ve `PROJECT_MUTATION_REJECTED` olarak API error union'ına eklenmiştir. Teacher-facing mesajlar teknik revision/hash/path göstermez; conflict UI'sı yerel formu koruyarak “Son durumu yenile” eylemi sunar.

Servisler uzun iş sonunda eski full snapshot'ı körlemesine yazamaz. Snapshot + source hash/generation alınır, dış iş lock'suz yapılır, sonuç `commit_job` ile güncel dosyaya narrow merge edilir. Ayrıntılı sözleşme ve production writer matrisi [`docs/PROJECTSTORE_CONCURRENCY.md`](PROJECTSTORE_CONCURRENCY.md) içindedir.

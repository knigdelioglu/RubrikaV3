Rubrika v3 Engineering Standards
Target stack: React + TypeScript + Tauri/Rust + llama.cpp server Purpose: This document defines how Rubrika v3 must be designed and coded so that we do not repeat the control, state, UI, worker, and model-lifecycle mistakes from Rubrika v2.
Rubrika v3 is not a UI rewrite. It is a controlled re-architecture.
The goal is not merely to make the app look modern. The goal is to make every user action, workflow state, long-running job, model call, storage mutation, and failure reason observable, testable, and explainable.

1. Core principle
Rubrika v3 must always answer these questions:
Which user action happened?
Which command did it call?
What did the backend return?
Did a job start?
What stage is the job in?
Was the model called?
What did the model return?
Did project state change?
Why is the workflow blocked?
What is the next valid user action?
If any of these questions cannot be answered from logs, command responses, job events, and project state, the design is not acceptable.

2. Non-negotiable architectural rules
2.1 UI does not own domain logic
React components must not decide core workflow readiness.
The frontend may display:
	•	current workflow stage
	•	blocking reasons
	•	next valid actions
	•	job progress
	•	validation errors
	•	teacher-facing explanations
But the frontend must not calculate:
	•	whether QEP is ready
	•	whether scoring may start
	•	whether question text is complete
	•	whether rubric is authoritative
	•	whether model output is valid
	•	whether OCR/scoring failure should become review
	•	whether a project should advance to the next stage
Those decisions belong to Rust domain services.
Bad:
const canCreateGuide =
  question.questionText && question.rubric.criteria.length > 0 && !qepFrozen;
Good:
const snapshot = await getWorkflowSnapshot(projectId);
return <NextActions actions={snapshot.nextActions} />;

2.2 Every state-changing user action is a backend command
A button must call a named command. It must not mutate local domain state directly.
Bad:
question.questionText.status = "confirmed";
setQuestions([...questions]);
Good:
await commands.confirmQuestionText({ projectId, questionId });
queryClient.invalidateQueries(["project-snapshot", projectId]);

2.3 Every long operation is a job
These operations must never block a Tauri command directly:
	•	PDF import and render cache
	•	question text extraction
	•	answer key/rubric extraction
	•	OCR
	•	QEP generation
	•	scoring
	•	export
	•	model server startup/probe if slow
The command starts a job and returns immediately.
const result = await commands.startOcrJob({ projectId });
jobStore.track(result.jobId);
The job emits events:
{
  "type": "job_progress",
  "job_id": "job_001",
  "stage": "ocr_page",
  "current": 4,
  "total": 18,
  "message": "Öğrenci 2 / Sayfa 1 işleniyor"
}

2.4 No raw exceptions in UI
The user must never see raw Rust panics, JavaScript stack traces, NameError, ValueError, NoneType, unwrap failed, or raw model JSON parsing errors.
All failures must become structured application errors.
type AppErrorDto = {
  code: AppErrorCode;
  message: string;
  recoverable: boolean;
  suggestedAction?: string;
  technicalDetails?: string;
  correlationId: string;
};
Teacher-facing UI shows message and suggestedAction.
Developer/debug panels may show technicalDetails.

2.5 Model failure is a domain result, not a crash
Local LLM output is unreliable by default. The model may return:
	•	empty content
	•	reasoning-only content
	•	invalid JSON
	•	partial JSON
	•	timeout
	•	transport error
	•	backend unavailable
	•	context-length failure
	•	hallucinated schema
	•	unexpected language
	•	wrong answer type structure
None of these may crash OCR/scoring.
For scoring, failure becomes:
type ScoreRecord = {
  scoringApplied: false;
  needsReview: true;
  reviewReasons: string[];
  teacherVisibleExplanation: string;
  rawDiagnostics?: ModelDiagnostics;
};
A failed model call is not the same as a student receiving zero points.

2.6 One source of truth
No permanent dual model such as:
legacy flat questions
section questions
screen-local question cache
QEP-local question copy
Rubrika v3 must have one canonical project model. Derived forms may exist only as read models, caches, or DTOs.
Rules:
	•	ProjectStore is the only writer of project files.
	•	Domain services receive and return canonical model types.
	•	UI receives snapshots/DTOs, not mutable model internals.
	•	Migration code runs once at import/open time, not continuously through bidirectional sync.
	•	Compatibility fields must be explicitly marked as derived or deprecated.

2.7 Placeholder is not data
Placeholder strings must never be persisted or treated as real rubric/answer-key content.
These are invalid real data:
kelime1, kelime2, kelime3
Kısmi puan kriterleri veya ek değerlendirme notları...
Anahtar kavramları girin...
Beklenen cevabı yazın...
Use placeholder UI props, not saved values.
Bad:
{
  "keyConcepts": ["kelime1", "kelime2", "kelime3"]
}
Good:
{
  "keyConcepts": [],
  "rubricStatus": "missing"
}

2.8 Teacher UI must not show technical codes
Teacher-facing UI must not display raw codes such as:
missing_question_text
unknown_answer_type
structured_parse_missing
frozen_for_scoring
general_text
MODEL_RESPONSE_INVALID_JSON
Use label mappers:
general_text -> Genel metin
sentence_annotation -> Cümle üzerinde işaretleme
missing_question_text -> Soru metni eksik
Technical codes may appear only in Developer / Diagnostics / Raw JSON panels.

2.9 QEP frozen gate must never be weakened
Scoring may run only when QEP is frozen for scoring.
QEP status must be frozen_for_scoring.
Do not add UI shortcuts, test bypasses, hidden fallbacks, or “temporary” logic that allows scoring from:
	•	missing QEP
	•	draft QEP
	•	suggested rubric
	•	unconfirmed Gemma draft
	•	invalidated package
	•	failed package
	•	teacher-review-required package

2.10 No production mock behavior
Mocks are allowed in tests. Mock data must never be used as production fallback.
Bad:
if model_failed {
    return fake_score_record();
}
Good:
if model_failed {
    return ScoreRecord::failure_review(...);
}

3. Required top-level architecture
React / TypeScript frontend
  - screens
  - components
  - API client
  - UI state only

Tauri command layer
  - typed commands
  - error mapping
  - event emission

Rust domain/backend
  - workflow engine
  - project store
  - PDF service
  - model gateway
  - OCR service
  - rubric service
  - QEP service
  - scoring service
  - analysis/export service

llama.cpp server
  - external or sidecar
  - OpenAI-compatible HTTP API

4. Suggested repository layout
rubrika-v3/
  docs/
    ENGINEERING_STANDARDS.md
    API_CONTRACTS.md
    WORKFLOW_STATES.md
    ERROR_CODES.md
    MODEL_GATEWAY.md

  src/
    app/
      App.tsx
      router.tsx
      queryClient.ts
    pages/
      ProjectCreatePage.tsx
      DocumentsPage.tsx
      QuestionTextPage.tsx
      RubricPrepPage.tsx
      CropTemplatePage.tsx
      OcrPage.tsx
      ReviewPage.tsx
      QepPage.tsx
      ScoringPage.tsx
      AnalysisPage.tsx
    components/
      pdf/
      workflow/
      review/
      qep/
      rubric/
      common/
    api/
      commands.ts
      tauriClient.ts
      types.ts
      errors.ts
    state/
      uiStore.ts
    utils/
      labels.ts
      formatting.ts

  src-tauri/
    Cargo.toml
    tauri.conf.json
    src/
      main.rs
      commands/
        project_commands.rs
        document_commands.rs
        workflow_commands.rs
        pdf_commands.rs
        question_text_commands.rs
        rubric_commands.rs
        ocr_commands.rs
        qep_commands.rs
        scoring_commands.rs
        model_commands.rs
      domain/
        project.rs
        document.rs
        question.rs
        rubric.rs
        ocr.rs
        qep.rs
        scoring.rs
        workflow.rs
        errors.rs
      services/
        project_store.rs
        document_service.rs
        pdf_service.rs
        document_content_extraction_service.rs
        model_gateway.rs
        llama_server_gateway.rs
        question_text_service.rs
        rubric_service.rs
        ocr_service.rs
        qep_service.rs
        scoring_service.rs
        analysis_service.rs
      jobs/
        job_manager.rs
        job_events.rs
      platform/
        paths.rs
        sidecar.rs
        file_access.rs
      tests/

5. Domain model standards
5.1 Project
type Project = {
  id: string;
  name: string;
  createdAt: string;
  updatedAt: string;
  rootPath: string;
  sections: Section[];
  documents: Document[];
  questions: Question[];
  workflow: WorkflowSnapshot;
};
sections are the canonical class/group structure. Do not maintain a separate permanent flat representation unless it is a derived read model.

5.2 Document
type DocumentRole =
  | "student_scan"
  | "exam_source"
  | "answer_key"
  | "rubric"
  | "export";

type Document = {
  id: string;
  role: DocumentRole;
  fileName: string;
  storedPath: string;
  pageCount: number;
  addedAt: string;
  checksum?: string;
};
Rules:
	•	exam_source is never counted as a student scan.
	•	answer_key is never used for student OCR.
	•	rubric is never used as direct student OCR truth.
	•	Only safe format hints may be used in OCR prompts.

5.3 Question
type AnswerType =
  | "general_text"
  | "short_text"
  | "essay"
  | "table"
  | "correction_table"
  | "sentence_annotation"
  | "grammar_analysis";

type Question = {
  id: string;
  number: number;
  maxScore: number;
  answerType: AnswerType;
  questionText: TextFieldState;
  rubric: RubricState;
  cropTemplate?: CropTemplate;
};

5.4 TextFieldState
type TextFieldState = {
  value: string;
  source: "manual" | "exam_pdf" | "student_pdf" | "imported_template" | "unknown";
  status: "missing" | "suggested" | "confirmed" | "edited" | "failed";
  confidence?: number;
  warnings: string[];
  updatedAt?: string;
};
Rules:
	•	Model-generated text is suggested.
	•	Teacher-confirmed text is confirmed.
	•	Teacher-edited text is edited.
	•	Suggested text cannot silently become authoritative.

5.5 RubricState
type RubricState = {
  expectedAnswer?: string | object;
  keyConcepts: string[];
  criteria: RubricCriterion[];
  partialCreditNotes: string[];
  zeroScoreConditions: string[];
  source: "manual" | "answer_key_pdf" | "rubric_pdf" | "gemma_draft" | "imported_template" | "unknown";
  status: "missing" | "suggested" | "confirmed" | "edited" | "failed";
  warnings: string[];
  updatedAt?: string;
};
Rules:
	•	Gemma draft rubric is never authoritative until teacher approval.
	•	Answer key PDF extraction is suggested until teacher confirmation.
	•	Manual teacher rubric may be edited or confirmed.
	•	Placeholder content is missing.

6. Workflow engine standards
Every project must expose a WorkflowSnapshot.
type WorkflowSnapshot = {
  currentStage: WorkflowStage;
  blockingReasons: BlockingReason[];
  nextActions: WorkflowAction[];
  summary: WorkflowSummary;
};
Allowed stages:
documents_missing
question_text_missing
question_text_suggested
rubric_missing
rubric_suggested
crop_missing
ocr_ready
ocr_running
review_required
qep_missing
qep_ready
qep_frozen
scoring_ready
scoring_running
scoring_done
analysis_ready
The workflow engine must be deterministic.
Same project input must always produce the same workflow snapshot.
Screens must not invent readiness states.

7. Command contract standards
7.1 All commands are typed
Every command must have:
	•	input type
	•	success output type
	•	error output type
	•	tests
	•	documentation entry
Example:
#[tauri::command]
async fn start_question_text_extraction(
    state: State<'_, AppState>,
    input: StartQuestionTextExtractionInput,
) -> Result<StartJobOutput, AppError> {
    state
        .services
        .question_text
        .start_extraction(input)
        .await
}

7.2 Command naming
Use verb-noun names:
create_project
open_project
import_exam_source_pdf
start_question_text_extraction
confirm_question_text
start_ocr_job
update_teacher_correction
build_qep_packages
freeze_qep_package
start_scoring_job
get_workflow_snapshot
Avoid vague names:
handle_next
do_action
process
run
submit

7.3 Commands must not perform hidden unrelated work
Bad:
import_exam_source_pdf also starts OCR, creates QEP, and changes current screen
Good:
import_exam_source_pdf imports the file and updates workflow snapshot.
The UI then shows the next valid action.

8. Error handling standards
8.1 AppError structure
Rust must use one central error type.
pub struct AppError {
    pub code: AppErrorCode,
    pub message: String,
    pub recoverable: bool,
    pub suggested_action: Option<String>,
    pub technical_details: Option<String>,
    pub correlation_id: String,
}
8.2 Error code categories
PROJECT_NOT_FOUND
PROJECT_LOAD_FAILED
PROJECT_SAVE_FAILED
DOCUMENT_IMPORT_FAILED
PDF_RENDER_FAILED
CROP_REGION_MISSING
WORKFLOW_BLOCKED
MODEL_SERVER_NOT_RUNNING
MODEL_HEALTH_FAILED
MODEL_TIMEOUT
MODEL_RESPONSE_EMPTY
MODEL_RESPONSE_INVALID_JSON
MODEL_RESPONSE_REASONING_ONLY
OCR_FAILED
SCORING_FAILED
QEP_NOT_FROZEN
RUBRIC_MISSING
QUESTION_TEXT_MISSING
PERMISSION_DENIED
UNKNOWN_ERROR
8.3 No unwrap in production paths
Disallowed outside tests and startup assertions:
unwrap()
expect()
panic!()
todo!()
unimplemented!()
If a failure can happen in real use, return AppError.

9. Job system standards
9.1 Job output
type JobSnapshot = {
  id: string;
  kind: JobKind;
  status: "queued" | "running" | "succeeded" | "failed" | "cancelled";
  progress: {
    current: number;
    total: number;
    message: string;
  };
  result?: unknown;
  error?: AppErrorDto;
  createdAt: string;
  updatedAt: string;
};
9.2 Job events
All long-running jobs emit events.
type JobEvent =
  | { type: "job_started"; jobId: string; kind: JobKind }
  | { type: "job_progress"; jobId: string; current: number; total: number; message: string }
  | { type: "job_succeeded"; jobId: string; result: unknown }
  | { type: "job_failed"; jobId: string; error: AppErrorDto };
9.3 Job failure must be visible
A failed job must appear in:
	•	UI error banner
	•	job history panel
	•	project log
	•	diagnostic export

10. Model gateway standards
10.1 UI never calls llama-server directly
Only Rust ModelGateway talks to llama.cpp server.
Bad:
fetch("http://127.0.0.1:8080/v1/chat/completions")
Good:
await commands.startOcrJob({ projectId });

10.2 ModelGateway trait
#[async_trait]
pub trait ModelGateway {
    async fn health(&self) -> Result<ModelHealth, AppError>;
    async fn extract_question_text(&self, input: QuestionTextInput) -> Result<QuestionTextOutput, AppError>;
    async fn extract_ocr_answer(&self, input: OcrInput) -> Result<OcrOutput, AppError>;
    async fn draft_rubric(&self, input: RubricDraftInput) -> Result<RubricDraftOutput, AppError>;
    async fn score_answer(&self, input: ScoringInput) -> Result<ScoringOutput, AppError>;
}
10.3 Model diagnostics
Every model call must capture diagnostics:
type ModelDiagnostics = {
  endpoint: string;
  requestKind: "question_text" | "ocr" | "rubric_draft" | "scoring";
  httpStatus?: number;
  durationMs: number;
  finishReason?: string;
  contentLength?: number;
  reasoningContentLength?: number;
  rawTextStoredPath?: string;
  errorCode?: string;
};
10.4 Structured output parsing
Never trust model JSON blindly.
Parsing pipeline:
raw response
→ extract content
→ detect reasoning-only / empty
→ JSON parse
→ schema validate
→ normalize tolerant variants
→ domain validation
→ record warnings

11. Prompt standards
11.1 Prompt builders are backend-only
Prompts must be built in Rust service modules, not frontend components.
11.2 Prompt names
Each prompt must have:
	•	stable name
	•	version
	•	input schema
	•	output schema
	•	tests with sample responses
Example:
prompt: ocr_sentence_annotation_v1
output schema: StructuredAnswer::SentenceAnnotation
11.3 No answer-key leakage into student OCR
Student OCR may use:
	•	question text
	•	answer type
	•	structural hints
	•	table columns if needed
	•	response format requirements
Student OCR must not receive:
	•	expected answer
	•	scoring rubric
	•	correct answer
	•	key concepts
	•	partial credit rules
	•	zero score conditions
Scoring may receive rubric and expected answer. OCR must not.

12. OCR standards
12.1 OCR output
type OcrRecord = {
  id: string;
  studentId: string;
  questionId: string;
  answerType: AnswerType;
  ocrText: string;
  structuredAnswer?: StructuredAnswer;
  teacherCorrection?: string;
  needsReview: boolean;
  reviewReasons: string[];
  warnings: string[];
  modelDiagnostics?: ModelDiagnostics;
};
12.2 Effective answer priority
teacherCorrection
→ structuredAnswer
→ ocrText
12.3 OCR review rules
OCR must set needsReview=true when:
	•	answer is empty but visual content exists
	•	structured parse failed
	•	answer type mismatch
	•	table structure incomplete
	•	annotation parse incomplete
	•	model response invalid
	•	confidence below threshold
	•	warnings contain critical items

13. QEP standards
13.1 QEP lifecycle
missing
draft_created
teacher_review_required
teacher_approved
frozen_for_scoring
invalidated
failed
13.2 QEP readiness
QEP may be built only when:
	•	question text is confirmed/edited/imported
	•	answer type is known
	•	max score is valid
	•	rubric or expected answer is confirmed/edited/imported
	•	placeholders are absent
	•	question-type-specific rubric requirements are satisfied
13.3 Frozen scoring gate
Scoring service must reject anything other than:
frozen_for_scoring
This rule is enforced in backend service, not only in UI.

14. Scoring standards
14.1 Scoring failure is not zero score
Invalid model output must never become normal zero points.
Bad:
if parse_failed {
    score = 0.0;
}
Good:
ScoreRecord {
    score: None,
    scoring_applied: false,
    needs_review: true,
    review_reasons: vec!["invalid_scoring_json"],
    teacher_visible_explanation: "...",
}
14.2 ScoreRecord
type ScoreRecord = {
  id: string;
  studentId: string;
  questionId: string;
  score?: number;
  maxScore: number;
  scoringApplied: boolean;
  needsReview: boolean;
  reviewReasons: string[];
  feedback?: string;
  criterionScores?: CriterionScore[];
  teacherVisibleExplanation?: string;
  modelDiagnostics?: ModelDiagnostics;
};

15. Frontend coding standards
15.1 TypeScript strict mode
Required:
{
  "compilerOptions": {
    "strict": true,
    "noImplicitAny": true,
    "noUncheckedIndexedAccess": true
  }
}
No any unless justified with a comment and isolated at external boundaries.
15.2 Component responsibilities
Page components:
	•	fetch snapshots
	•	render layout
	•	connect buttons to commands
	•	show loading/error states
Page components must not:
	•	implement domain validation
	•	mutate project state locally
	•	parse model output
	•	decide workflow readiness
15.3 Server state
Use TanStack Query for backend state:
	•	project snapshot
	•	workflow snapshot
	•	OCR records
	•	QEP packages
	•	score records
	•	job snapshots
Use local UI store only for:
	•	selected row
	•	open/closed panels
	•	zoom level
	•	currently selected crop rectangle before save
	•	unsaved form draft
15.4 No silent button failure
Every action button must have:
	•	disabled reason if disabled
	•	loading state if running
	•	success feedback if completed
	•	error banner if failed

16. Rust coding standards
16.1 Service boundaries
Each domain service owns one responsibility.
ProjectStore        load/save only
WorkflowEngine     readiness and next actions
DocumentService    import/list/remove documents
PdfService         PDF page rendering/crops
ModelGateway       llama-server communication
OcrService         OCR domain logic
RubricService      rubric extraction/prep
QepService         package lifecycle
ScoringService     scoring lifecycle
AnalysisService    summaries/export
16.2 No god service
If a service imports everything, it is becoming a god object.
Warning signs:
	•	service has more than one unrelated responsibility
	•	service starts jobs and mutates project and calls model and updates UI state
	•	service owns screen-specific behavior
	•	service has many optional dependencies
Split it.
16.3 Tests for every service
Each service must have unit tests for:
	•	success path
	•	blocked workflow path
	•	recoverable failure
	•	invalid input
	•	persistence effect if any

17. Storage standards
17.1 Project folder
RubrikaProjects/
  project_id/
    project.json
    documents/
    cache/
      page_previews/
      model_raw/
    crops/
    outputs/
    logs/
17.2 Atomic save
Project save must be atomic:
write project.json.tmp
fsync if practical
rename to project.json
17.3 Project logs
Every important domain action writes an event:
{
  "timestamp": "...",
  "event": "question_text_extraction_started",
  "project_id": "...",
  "correlation_id": "..."
}
This is different from frontend console logs.

18. Observability standards
18.1 Correlation IDs
Every user action gets a correlation ID.
The same ID appears in:
	•	frontend command call
	•	backend command log
	•	job events
	•	model diagnostics
	•	project log
	•	error message
18.2 Diagnostic export
Diagnostic report must include:
	•	app version
	•	platform
	•	project id/path
	•	workflow snapshot
	•	documents
	•	model status
	•	latest jobs
	•	latest errors
	•	warnings
	•	selected raw model diagnostics paths
No secrets or full student answers unless user explicitly exports project data.

18.3 Diagnostic CLI
The `rubrika` CLI must be able to inspect project health, jobs, model status, documents, question text, rubric state, and model input caches without opening the GUI.

18.4 PDF preview vs model input
PDF preview images are for humans.
Document content extraction is the shared text-first layer that writes raw text, normalized text, and quality metadata.
Model input images are for llama-server requests and are only prepared when text coverage is insufficient.
Those caches must be separated and recorded independently.

19. Testing standards
19.1 Required tests per feature
For every feature, add:
	•	Rust unit tests
	•	command contract test
	•	frontend component test if UI changes
	•	integration test for project state mutation
	•	failure-path test
	•	regression test for previous known bug if applicable
19.2 Known bug regression examples
Add tests for these classes of bugs:
button click does nothing
missing import / missing dependency
PDF exists but workflow still says missing without explaining why
placeholder treated as real rubric
model invalid JSON crashes scoring
QEP not frozen but scoring starts
teacher UI shows raw technical code
state changes but workflow snapshot remains stale
19.3 Required commands before task completion
npm run typecheck
npm run lint
npm test
cargo fmt --check
cargo clippy -- -D warnings
cargo test
npm run tauri dev -- --smoke
If a command is not yet configured, the task must either configure it or explicitly document why it is not available.

20. UI/UX standards
20.1 Workflow-first screens
Every main screen shows:
	•	current stage
	•	blocking reasons
	•	next valid actions
	•	current job status if any
The user must never wonder:
What should I do next?
Why is this button disabled?
Why did nothing happen?
20.2 Preparation stages
Rubrika must clearly separate:
Soru Metni Hazırlığı
Cevap Anahtarı / Rubrik Hazırlığı
OCR
Öğretmen Kontrolü
QEP
Scoring
Analiz
Do not show QEP technical panels while question text or rubric is missing.
20.3 Progressive disclosure
Teacher UI first. Technical details second.
Main UI:
Cevap anahtarı eksik
Soru metni onay bekliyor
Model çalışmıyor
Developer panel:
RUBRIC_MISSING
question.rubric.status=missing
MODEL_SERVER_NOT_RUNNING

21. Migration from Rubrika v2
21.1 v2 code is reference, not template
Port concepts, not architecture.
Bring:
	•	successful prompt ideas
	•	structured OCR schema concepts
	•	scoring failure taxonomy
	•	QEP frozen gate
	•	known test cases
	•	export requirements
Do not bring:
	•	MainWindow orchestration
	•	UI-owned workflow decisions
	•	bidirectional legacy sync
	•	raw exception failure paths
	•	placeholder-as-data patterns
	•	hidden model fallback behavior
21.2 Migration tools are explicit
If v2 projects are imported, write a dedicated importer:
import_v2_project
→ validate
→ migrate
→ produce migration report
Do not keep permanent v2/v3 dual models.

22. Codex task protocol
Every Codex task must follow this format.
22.1 Before coding
Codex must state:
Files I will inspect:
Files I expect to modify:
Domain assumptions:
Command/API contract affected:
Tests I will add/update:
Risks:
22.2 During coding
Codex must not do broad rewrites without stating scope.
If it discovers a larger issue, it must stop and report instead of silently changing unrelated modules.
22.3 After coding
Codex must report:
What changed
Why it changed
Files modified
New/updated commands
New/updated types
New/updated tests
Commands run
Test results
Known limitations
Manual verification steps
22.4 Completion is not valid without tests
A task is not complete because code was edited. A task is complete only when:
	•	relevant tests exist
	•	tests pass
	•	smoke check passes if UI/command changed
	•	manual verification steps are provided

23. Pull request checklist
Before merging any change:
[ ] UI does not own new domain logic.
[ ] New command has typed input/output.
[ ] Long operation is a job.
[ ] Errors use AppError codes.
[ ] No raw exception reaches UI.
[ ] WorkflowSnapshot reflects new state.
[ ] ProjectStore is the only persistence writer.
[ ] Model calls go only through ModelGateway.
[ ] Teacher UI has friendly labels.
[ ] Technical details are behind diagnostics/dev panel.
[ ] Placeholder data is not treated as real data.
[ ] QEP frozen gate is preserved.
[ ] Scoring failure is not saved as normal zero.
[ ] Tests cover success and failure paths.
[ ] Regression tests cover previous bug class.
[ ] Logs/diagnostics can explain failure.

24. Anti-patterns that must be rejected
24.1 God object
Reject code where a single screen/controller:
	•	starts model server
	•	mutates project
	•	launches jobs
	•	parses model output
	•	calculates workflow readiness
	•	updates storage
	•	controls navigation
24.2 Hidden side effect
Reject commands that do multiple unrelated things.
24.3 UI-only fix
Reject fixes that only hide the symptom in UI while backend state remains wrong.
24.4 Test-only bypass
Reject production behavior that changes under test flags.
24.5 Silent fallback
Reject fallback that changes behavior without visible diagnostics.
Example:
8081 failed, silently use 8080
Allowed only if diagnostics explicitly say fallback was used.
24.6 Raw model trust
Reject direct use of model output without schema validation and normalization.

25. First implementation milestone
The first milestone must not include OCR/scoring.
It must include only:
create_project
open_project
import_exam_source_pdf
get_workflow_snapshot
get_model_status
frontend workflow panel
logs/diagnostics
Acceptance:
A user can create a project, import an exam source PDF, and see exactly why the workflow is blocked and what the next valid action is.
Only after this works should question text extraction be implemented.

26. Final rule
Rubrika v3 exists because Rubrika v2 became difficult to reason about.
Therefore every new line of code must improve at least one of:
observability
testability
state clarity
domain separation
error traceability
workflow correctness
If a change only adds UI surface while making state harder to reason about, reject it.

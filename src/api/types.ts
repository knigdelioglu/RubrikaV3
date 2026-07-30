import type { AppError } from './errors';

export type AppStatus = {
  app_version: string;
  platform: string;
  tauri_ready: boolean;
  rust_backend_ready: boolean;
};

export type JobKind =
  | 'question_text_extraction'
  | 'pdf_preview_render'
  | 'rubric_pdf_import'
  | 'exam_package_build'
  | 'student_answer_ocr'
  | 'student_identity_ocr'
  | 'scoring'
  | 'speaking_evaluation'
  | 'assessment_analysis';

export type JobStatus = 'queued' | 'running' | 'succeeded' | 'partial' | 'failed' | 'cancelled';

export type JobProgress = {
  current: number;
  total: number;
  message: string;
};

export type JobSnapshot = {
  id: string;
  projectId: string;
  kind: JobKind;
  status: JobStatus;
  progress: JobProgress;
  result?: unknown;
  error?: AppError;
  createdAt: string;
  updatedAt: string;
};

export type WorkflowStage =
  | 'documents_missing'
  | 'pdf_preview_missing'
  | 'pdf_preview_ready'
  | 'pdf_preview_ready_question_text_missing'
  | 'exam_package_build_ready'
  | 'exam_package_build_running'
  | 'exam_package_review_needed'
  | 'exam_package_incomplete'
  | 'exam_package_ready_for_qep'
  | 'question_text_missing'
  | 'question_text_extraction_running'
  | 'question_text_suggested'
  | 'question_text_confirmed'
  | 'rubric_missing'
  | 'rubric_suggested'
  | 'rubric_imported_needs_review'
  | 'rubric_invalid'
  | 'rubric_confirmed'
  | 'student_scans_missing'
  | 'student_scan_preview_missing'
  | 'student_grouping_missing'
  | 'student_grouping_ready'
  | 'crop_missing'
  | 'ocr_ready'
  | 'ocr_running'
  | 'review_required'
  | 'student_answer_ocr_running'
  | 'student_answer_ocr_review_needed'
  | 'student_answer_ocr_ready_for_scoring'
  | 'qep_missing'
  | 'qep_ready'
  | 'qep_frozen'
  | 'scoring_ready'
  | 'scoring_running'
  | 'scoring_done'
  | 'analysis_ready';

export type BlockingReason =
  | 'EXAM_SOURCE_MISSING'
  | 'EXAM_SOURCE_PDF_MISSING'
  | 'RUBRIC_DOCUMENT_MISSING'
  | 'QUESTION_COUNT_MISSING'
  | 'EXAM_PACKAGE_BUILD_PRECHECK_FAILED'
  | 'PDF_PREVIEW_MISSING'
  | 'QUESTION_TEXT_MISSING'
  | 'RUBRIC_MISSING'
  | 'RUBRIC_INVALID'
  | 'CROP_MISSING'
  | 'PLACEHOLDER_DATA_DETECTED'
  | 'REVIEW_REQUIRED'
  | 'QEP_NOT_FROZEN'
  | 'RUBRIC_NOT_READY'
  | 'RUBRIC_JSON_INVALID'
  | 'RUBRIC_JSON_PARSE_FAILED'
  | 'RUBRIC_JSON_SCHEMA_UNSUPPORTED'
  | 'RUBRIC_SCHEMA_VALIDATION_FAILED'
  | 'RUBRIC_QUESTION_NOT_FOUND'
  | 'RUBRIC_PLACEHOLDER_DETECTED'
  | 'RUBRIC_MAX_SCORE_MISSING'
  | 'RUBRIC_EXPECTED_ANSWER_MISSING'
  | 'RUBRIC_CRITERIA_MISSING'
  | 'RUBRIC_POINTS_TOTAL_MISMATCH'
  | 'RUBRIC_CONFIRM_FAILED'
  | 'QUESTION_COVERAGE_INCOMPLETE'
  | 'QUESTION_LAST_ITEM_MISSING'
  | 'STUDENT_SCAN_NOT_FOUND'
  | 'STUDENT_SCAN_PREVIEW_NOT_READY'
  | 'STUDENT_GROUPING_NOT_READY'
  | 'STUDENT_GROUPING_INVALID'
  | 'STUDENT_SUBMISSION_NOT_FOUND'
  | 'STUDENT_IDENTITY_INVALID'
  | 'OCR_NOT_READY'
  | 'STUDENT_ANSWER_OCR_NOT_READY'
  | string; // fallback

export type WorkflowAction = {
  code: string;
  label: string;
  enabled: boolean;
  disabledReason?: string;
  command?: string;
  requires?: string[];
};

export type WorkflowStep = {
  code: string;
  label: string;
  status: 'pending' | 'running' | 'succeeded' | 'failed' | 'partial';
  message: string;
  current?: number;
  total?: number;
};

export type WorkflowReadiness = {
  examPackageFreeze: boolean;
  studentIntake: boolean;
  scoring: boolean;
};

export type WorkflowSummary = {
  text?: string | null;
  steps: WorkflowStep[];
  readiness: WorkflowReadiness;
};

export type WorkflowSnapshot = {
  currentStage: WorkflowStage;
  currentStageLabel: string;
  blockingReasons: BlockingReason[];
  nextActions: WorkflowAction[];
  summary: WorkflowSummary;
};

export type StartSpeakingExamInput = {
  projectId: string;
  examName: string;
  examType: 'prepared' | 'impromptu';
  taskText: string;
  targetMinutes?: number;
  minimumMinutes?: number;
  maximumMinutes?: number;
  targetSeconds?: number;
  minimumSeconds?: number;
  maximumSeconds?: number;
  classId?: string;
  assignedClassIds?: string[];
  examId?: string;
  teacherNote?: string;
  examDate?: string;
};

export type StartSpeakingExamOutput = {
  started: boolean;
  engine: 'Speakoflow Embedded';
  examId: string;
  message: string;
};

export type SpeakingEngineRuntimeStatus = {
  state: string;
  whisperReady: boolean;
  whisperLoaded: boolean;
  whisperModelPath?: string | null;
  activeSession: boolean;
  elapsedMs: number;
  audioPeak: number;
  audioRms: number;
};

export type MicrophoneDevice = {
  id: string;
  name: string;
  isDefault: boolean;
};

export type ToggleSpeakingCaptureInput = {
  projectId: string;
  examId: string;
  studentId: string;
  action: 'start' | 'pause' | 'resume' | 'stop' | 'cancel';
};

export type ToggleSpeakingCaptureOutput = {
  action: 'start' | 'pause' | 'resume' | 'stop' | 'cancel';
  accepted: boolean;
  attemptId?: string | null;
  message: string;
};

export type SpeakingExamType = 'prepared' | 'impromptu';
export type SpeakingAttemptState = 'draft' | 'recording' | 'paused' | 'finalizing' | 'cleaning_transcript' | 'evaluating' | 'teacher_review' | 'approved' | 'cancelled' | 'failed';
export type SpeakingCriterionRole = 'automatic' | 'ai_suggested' | 'teacher_only';
export type SpeakingConfidence = 'high' | 'medium' | 'low' | 'not_evaluated';
export type SpeakingTranscriptCleanupStatus = 'not_started' | 'running' | 'accepted' | 'needs_review' | 'failed';
export type SpeakingPerformanceLevel =
  | 'very_good'
  | 'good'
  | 'moderate'
  | 'developing'
  | 'not_observed'
  | 'star_3'
  | 'star_2'
  | 'star_1'
  | 'not_evaluated'
  | 'performance_not_shown';

export type SpeakingTranscriptCleanup = {
  status: SpeakingTranscriptCleanupStatus;
  transcriptForScoring?: string | null;
  modelId: string;
  promptVersion: string;
  failureReason?: string | null;
  candidate?: string | null;
  needsReview?: boolean;
};

export type SpeakingCriterion = {
  id: string;
  label: string;
  description: string;
  maxScore: number;
  role: SpeakingCriterionRole;
  performanceLevels: SpeakingPerformanceDescriptor[];
};

export type SpeakingPerformanceDescriptor = {
  level: SpeakingPerformanceLevel;
  label: string;
  description: string;
  scoreRatio: number;
};

export type SpeakingEvidence = {
  startMs: number;
  endMs: number;
  quote: string;
  reason: string;
};

export type SpeakingCriterionScore = {
  criterionId: string;
  criterionLabel: string;
  maxScore: number;
  automaticScore?: number | null;
  aiSuggestedScore?: number | null;
  aiConfidence: SpeakingConfidence;
  aiSummary: string;
  subindicatorScores: Array<{
    subindicatorId: string;
    selectedLevelId: string;
    appliedLevelId: string;
    points: number;
    evidenceSegmentIds: string[];
    counterEvidenceSegmentIds: string[];
    missingRequirements: string[];
    ceilingReasonCode?: string | null;
    ceilingExplanation?: string | null;
    rationale: string;
  }>;
  evidence: SpeakingEvidence[];
  teacherScore?: number | null;
  teacherLevel?: SpeakingPerformanceLevel | null;
  teacherNote?: string | null;
  finalScore?: number | null;
};

export type SpeakingMetrics = {
  durationMs: number;
  activeSpeechDurationMs: number;
  wordCount: number;
  wordsPerMinute: number;
  totalSilenceMs: number;
  longestSilenceMs?: number;
  silenceRatio?: number;
  longPauseCount: number;
  fillerCount: number;
  repetitionCount: number;
  durationScore: number;
  expectedMinDurationMs: number;
  sampleDurationSufficient: boolean;
  measurementConfidence: SpeakingConfidence;
  clippedSampleCount?: number;
  clippingEventCount?: number;
  clippingRatio?: number;
  peakLevel?: number;
  rmsLevel?: number;
  lowVolumeRatio?: number;
  audioQualityConfidence?: SpeakingConfidence;
  warnings: string[];
};

export type SpeakingTranscriptSegment = {
  segmentId: string;
  startMs: number;
  endMs: number;
  text: string;
  rawText?: string | null;
  cleanedText?: string | null;
  confidence?: number | null;
};

export type SpeakingAttempt = {
  id: string;
  examId: string;
  studentId: string;
  attemptNumber: number;
  state: SpeakingAttemptState;
  startedAt: string;
  endedAt?: string | null;
  audioPath?: string | null;
  sourceHistoryId?: number | null;
  rawTranscript: string;
  readableTranscript: string;
  cleanupCandidate?: string | null;
  transcriptForScoring?: string | null;
  approvedTranscript?: string | null;
  cleanupStatus: SpeakingTranscriptCleanupStatus;
  cleanupChanges: Array<{
    segmentId: string;
    original: string;
    replacement: string;
    changeType: string;
    meaningChanged: boolean;
    confidence?: number | null;
  }>;
  cleanupDiagnostics?: unknown | null;
  cleanupModelProvenance?: unknown | null;
  evaluationModelProvenance?: unknown | null;
  evaluationInputHash?: string | null;
  scoringPolicyVersion: string;
  evaluationPromptVersion: string;
  transcriptCleanup: SpeakingTranscriptCleanup;
  transcriptSegments: SpeakingTranscriptSegment[];
  metrics: SpeakingMetrics;
  criterionScores: SpeakingCriterionScore[];
  evaluationJobId?: string | null;
  evaluationError?: string | null;
  teacherNote?: string | null;
  finalScore?: number | null;
  teacherApprovedAt?: string | null;
  modelId: string;
  promptVersion: string;
  rubricVersion: string;
};

export type SpeakingExam = {
  id: string;
  title: string;
  classId?: string | null;
  assignedClassIds?: string[];
  examType: SpeakingExamType;
  taskText: string;
  targetDurationSeconds: number;
  minDurationSeconds: number;
  maxDurationSeconds: number;
  rubricVersion: string;
  scoringPolicyVersion: string;
  cleanupPromptVersion: string;
  evaluationPromptVersion: string;
  frozenModelFileHash?: string | null;
  rubricLabel: string;
  criteria: SpeakingCriterion[];
  aiEvaluationEnabled: boolean;
  selfAssessmentEnabled: boolean;
  status: string;
  createdAt: string;
  updatedAt: string;
  activeStudentId?: string | null;
  completedAt?: string | null;
  attempts: SpeakingAttempt[];
};

export type AssessmentKind = 'written' | 'speaking';
export type AnalysisStatus = 'generating' | 'ready' | 'partial' | 'failed';

export type AnalysisCriterionSummary = {
  id: string;
  label: string;
  averageScore: number;
  maxScore: number;
  percentage: number;
  sampleCount: number;
};

export type AnalysisStudentSummary = {
  studentId: string;
  displayName: string;
  score: number;
  maxScore: number;
  percentage: number;
};

export type AnalysisScoreBand = {
  label: string;
  minimum: number;
  maximum: number;
  count: number;
};

export type AssessmentAnalysis = {
  id: string;
  projectId: string;
  kind: AssessmentKind;
  sourceId?: string | null;
  title: string;
  classId?: string | null;
  status: AnalysisStatus;
  studentCount: number;
  criteria: AnalysisCriterionSummary[];
  students: AnalysisStudentSummary[];
  scoreBands: AnalysisScoreBand[];
  modelReport?: string | null;
  modelReportError?: string | null;
  createdAt: string;
  completedAt?: string | null;
};

export type FinishAssessmentOutput = {
  analysisId: string;
  jobId: string;
  status: 'queued';
};

export type SpeakingAttemptSyncOutput = {
  attempt: SpeakingAttempt;
  ready: boolean;
};

export type DocumentRole = 'student_scan' | 'exam_source' | 'answer_key' | 'rubric' | 'export';

export type Document = {
  id: string;
  role: DocumentRole;
  fileName: string;
  storedPath: string;
  pageCount: number;
  addedAt: string;
  checksum?: string;
  preview?: PdfPreviewState | null;
};

export type PdfPreviewStatus = 'missing' | 'queued' | 'running' | 'ready' | 'failed';

export type PdfPreviewState = {
  status: PdfPreviewStatus;
  renderedAt?: string | null;
  pageCount?: number | null;
  jobId?: string | null;
  errorMessage?: string | null;
};

export type PdfPagePreview = {
  documentId: string;
  pageNumber: number;
  imagePath: string;
  width: number;
  height: number;
  renderedAt: string;
};

export type PdfPreviewStatusSnapshot = {
  documentId: string;
  status: PdfPreviewStatus;
  pageCount: number;
  renderedAt?: string | null;
  jobId?: string | null;
  previewCount: number;
  message: string;
  errorMessage?: string | null;
};

export type StartPdfPreviewRenderOutput = {
  jobId: string;
  status: 'queued' | 'running';
};

export type PdfRendererStatus = {
  available: boolean;
  backend: 'poppler' | 'macos_fallback' | 'none';
  pdfinfoPath?: string;
  pdftoppmPath?: string;
  searchedPaths: string[];
  pathEnv?: string;
  installHint?: string;
  warnings: string[];
};

export type TextFieldSource = 'manual' | 'exam_pdf' | 'student_pdf' | 'imported_template' | 'unknown';

export type TextFieldStatus = 'missing' | 'suggested' | 'confirmed' | 'edited' | 'failed';

export type TextFieldState = {
  value: string;
  source: TextFieldSource;
  status: TextFieldStatus;
  confidence?: number;
  warnings: string[];
  updatedAt?: string;
};

export type AnswerType =
  | 'general_text'
  | 'short_text'
  | 'essay'
  | 'table'
  | 'correction_table'
  | 'fill_blank'
  | 'matching'
  | 'multiple_choice'
  | 'true_false'
  | 'ordering'
  | 'numeric'
  | 'diagram_labeling'
  | 'sentence_annotation'
  | 'grammar_analysis';

export type RubricSource = 'manual' | 'json' | 'answer_key_pdf' | 'generated' | 'unknown';

export type RubricStatus = 'missing' | 'suggested' | 'imported' | 'manual' | 'confirmed' | 'invalid' | 'legacy';

export type RubricCriterion = {
  id: string;
  label: string;
  description: string;
  points: number;
};

export type ScoringReviewStatus = 'pending_review' | 'approved' | 'edited' | 'invalidated';

export type ScoringCriterionScore = {
  criterionId: string;
  criterionTitle: string;
  criterionMaxScore: number;
  awardedScore: number;
  rationale: string;
  evidenceQuote?: string | null;
};

export type ScoringParseDiagnostics = {
  rawModelOutput: string;
  parseError?: string | null;
  parsedJson?: unknown | null;
  salvagedRationale?: string | null;
  parseStrategy: string;
  modelRequestMetadata?: unknown | null;
};

export type ScoringReconciliationDiagnostics = {
  modelAwardedScore: number;
  criterionSum?: number | null;
  criterionMaxSum?: number | null;
  questionMaxScore: number;
  correctedAwardedScore: number;
  needsReview: boolean;
  warnings: string[];
  notes: string[];
};

export type ScoringRecord = {
  id: string;
  runId: string;
  submissionId: string;
  studentId: string;
  studentDisplayName?: string | null;
  studentNumber?: string | null;
  studentClassName?: string | null;
  questionId: string;
  questionNumber: number;
  maxScore: number;
  awardedScore: number | null;
  scoringApplied: boolean;
  criterionScores: ScoringCriterionScore[];
  rationale: string;
  confidence: number;
  needsReview: boolean;
  reviewReasons: string[];
  warnings: string[];
  rawModelOutput: string;
  parseDiagnostics?: ScoringParseDiagnostics | null;
  reconciliationDiagnostics?: ScoringReconciliationDiagnostics | null;
  sourceHash: string;
  packageHash: string;
  ocrRecordHash: string;
  questionTextHash: string;
  rubricHash: string;
  teacherReviewStatus: ScoringReviewStatus;
  teacherManualScore?: number | null;
  teacherReviewedAt?: string | null;
  teacherNotes?: string | null;
  invalidatedAt?: string | null;
  invalidationReason?: string | null;
  createdAt: string;
  updatedAt: string;
};

export type RubricState = {
  source?: RubricSource | null;
  maxScore?: number | null;
  expectedAnswer?: string | null;
  criteria: RubricCriterion[];
  partialCreditHints: string[];
  zeroScoreConditions: string[];
  commonMistakes: string[];
  status: RubricStatus;
  warnings: string[];
  updatedAt?: string;
};

export type RubricValidationIssue = {
  code: string;
  message: string;
};

export type RubricValidationSnapshot = {
  valid: boolean;
  confirmable: boolean;
  warnings: string[];
  issues: RubricValidationIssue[];
  totalPoints?: number | null;
};

export type RubricQuestionSnapshot = {
  question: Question;
  validation: RubricValidationSnapshot;
};

export type RubricStateSnapshot = {
  projectId: string;
  currentStage: string;
  items: RubricQuestionSnapshot[];
  missingCount: number;
  importedCount: number;
  manualCount: number;
  confirmedCount: number;
  invalidCount: number;
  warnings: string[];
  summary: string;
};

export type RubricValidationQuestionSnapshot = {
  questionId: string;
  number: number;
  status: string;
  valid: boolean;
  warnings: string[];
  issues: RubricValidationIssue[];
  totalPoints?: number | null;
};

export type RubricValidationReport = {
  projectId: string;
  valid: boolean;
  confirmable: boolean;
  warnings: string[];
  blockingQuestions: number[];
  questions: RubricValidationQuestionSnapshot[];
};

export type CropTemplate = {
  x: number;
  y: number;
  width: number;
  height: number;
  pageIndex: number;
};

export type Question = {
  id: string;
  number: number;
  maxScore: number;
  answerType: AnswerType;
  questionText: TextFieldState;
  rubric: RubricState;
  cropTemplate?: CropTemplate;
};

export type ImportRubricJsonInput = {
  projectId: string;
  documentId?: string;
  filePath?: string;
};

export type StartRubricPdfImportInput = {
  projectId: string;
  documentId?: string;
  expectedQuestionCount: number;
};

export type StartExamPackageBuildInput = {
  projectId: string;
  expectedQuestionCount: number;
};

export type StartExamPackageBuildOutput = {
  jobId: string;
  status: 'queued' | 'running';
};

export type StartScoringOutput = {
  jobId: string;
  status: 'queued' | 'running';
  rerun: boolean;
};

export type GradedExamAnnotationStatus = 'model_score' | 'needs_review';

export type GradedExamPlacement = 'right_of_answer' | 'inside_top_right';

export type GradedExamScorePart = {
  title: string;
  awardedScore: number;
  maxScore: number;
};

export type GradedExamAnnotation = {
  recordId: string;
  questionId: string;
  questionNumber: number;
  modelScore?: number | null;
  maxScore: number;
  label: string;
  x: number;
  y: number;
  width: number;
  height: number;
  placement: GradedExamPlacement;
  status: GradedExamAnnotationStatus;
  needsReview: boolean;
  scoreParts: GradedExamScorePart[];
  reviewGuidance: string[];
};

export type GradedExamUnplacedScore = {
  recordId: string;
  questionId: string;
  questionNumber: number;
  modelScore?: number | null;
  maxScore: number;
  reason: string;
};

export type GradedExamPage = {
  pageNumber: number;
  imagePath: string;
  width: number;
  height: number;
  annotations: GradedExamAnnotation[];
};

export type GradedExamReview = {
  projectId: string;
  submissionId: string;
  documentId: string;
  studentDisplayName: string;
  studentNumber?: string | null;
  studentClassName?: string | null;
  scoringRunId?: string | null;
  modelTotalScore?: number | null;
  maxTotalScore: number;
  needsReviewCount: number;
  pages: GradedExamPage[];
  unplacedScores: GradedExamUnplacedScore[];
};

export type StartStudentAnswerOcrOutput = {
  jobId: string;
  status: 'queued' | 'running';
  rerun: boolean;
};

export type StudentAnswerOcrIssueCorrectionDecision =
  | 'suggest_correction'
  | 'no_change'
  | 'needs_teacher_review';

export type StudentAnswerOcrIssueCorrectionScope = 'single_word' | 'short_phrase';

export type StudentAnswerOcrIssueCorrectionSuggestion = {
  decision: StudentAnswerOcrIssueCorrectionDecision;
  originalText: string;
  suggestedText: string | null;
  scope: StudentAnswerOcrIssueCorrectionScope;
  visualReading: string | null;
  contextReason: string;
  confidence: number;
  requiresTeacherApproval: true;
  warnings: string[];
};

export type SuggestStudentAnswerOcrIssueCorrectionWithModelInput = {
  projectPath: string;
  ocrRecordId: string;
  issueId?: string | null;
  observedText: string;
  suggestedTextFromAnalyzer: string;
  questionNumber: number;
  highlightRegion?: StudentAnswerOcrCropBBox | null;
  cropRef?: string | null;
  modelInputCropRef?: string | null;
};

export type SuggestStudentAnswerOcrIssueCorrectionWithModelOutput = {
  suggestion: StudentAnswerOcrIssueCorrectionSuggestion;
  rawModelOutput: string;
  usedImageRef: string | null;
  promptVersion: string;
  modelRequestMetadata?: unknown | null;
};

export type RebuildStudentAnswerOcrIssuesOutput = {
  projectId: string;
  updatedRecords: number;
  updatedIssues: number;
};

export type StartStudentIdentityOcrOutput = {
  jobId: string;
  status: 'queued' | 'running' | string;
};

export type ExamPackageBuildQuestionTextResult = {
  skipped: boolean;
  confirmed: number[];
  extracted: number[];
  missing: number[];
  partialSuccess: boolean;
};

export type ExamPackageBuildRubricResult = {
  skipped: boolean;
  imported: number[];
  missing: number[];
  failed: number[];
  partialSuccess: boolean;
};

export type ExamPackageBuildResult = {
  expectedQuestionCount: number;
  questionText: ExamPackageBuildQuestionTextResult;
  rubric: ExamPackageBuildRubricResult;
  nextRoute?: string | null;
};

export type ImportRubricJsonOutput = {
  importedCount: number;
  missingCount: number;
  invalidCount: number;
  warnings: string[];
};

export type UpdateQuestionRubricInput = {
  projectId: string;
  questionId: string;
  answerType: AnswerType;
  maxScore: number | null;
  expectedAnswer: string | null;
  criteria: RubricCriterion[];
  partialCreditHints: string[];
  zeroScoreConditions: string[];
  commonMistakes: string[];
};

export type QuestionTextSuggestion = {
  questionId: string;
  number: number;
  text: string;
  confidence: number;
  source: 'exam_pdf';
  status: 'suggested' | 'confirmed' | 'edited' | 'failed' | 'missing';
  warnings: string[];
};

export type QuestionTextExtractionStatus = {
  projectId: string;
  documentId?: string | null;
  previewStatus: 'missing' | 'queued' | 'running' | 'ready' | 'failed';
  previewReady: boolean;
  currentStage: string;
  blockingReasons: string[];
  nextActions: string[];
  detectedQuestionCount?: number | null;
  suggestedCount: number;
  confirmedCount: number;
  missingCount: number;
  missingQuestionNumbers: number[];
  coverageOk: boolean;
  extractionMethod?: string | null;
  visionFallbackAvailable: boolean;
  runningJobId?: string | null;
  latestJobStatus?: string | null;
  summary?: string | null;
};

export type Section = {
  id: string;
  name: string;
  students: Student[];
};

export type SchoolClassStatus = 'active' | 'archived';

export type SchoolClass = {
  id: string;
  name: string;
  normalizedName: string;
  academicYear?: string | null;
  gradeLevel?: number | null;
  section?: string | null;
  displayOrder: number;
  status: SchoolClassStatus;
  createdAt: string;
  updatedAt: string;
};

export type StudentScanBatch = {
  id: string;
  classId: string;
  documentId: string;
  originalFileName: string;
  displayName: string;
  pagesPerStudent?: number | null;
  groupingMode?: PageGroupingMode | null;
  groupingCompletedAt?: string | null;
  createdAt: string;
  updatedAt: string;
};

export type SchoolClassOverview = {
  schoolClass: SchoolClass;
  scanBatchCount: number;
  submissionCount: number;
  identityVerifiedCount: number;
  ocrCompleteCount: number;
  scoringCompleteCount: number;
  reviewRequiredCount: number;
};

export type SchoolClassOverviewSnapshot = {
  classes: SchoolClassOverview[];
  unassignedBatchCount: number;
  unassignedSubmissionCount: number;
};

export type ImportStudentScanBatchInput = {
  projectId: string;
  classId: string;
  sourcePath: string;
  displayName?: string;
  pagesPerStudent?: number;
  groupingMode?: PageGroupingMode;
};

export type ImportStudentScanBatchOutput = {
  document: Document;
  batch: StudentScanBatch;
};

export type ExamPackageFreezeStatus = 'frozen' | 'invalidated';

export type ExamPackageFreeze = {
  examPackageVersion: number;
  freezeStatus: ExamPackageFreezeStatus;
  frozenAt: string;
  frozenBy?: string | null;
  sourceHash: string;
  rubricHash: string;
  questionTextHash: string;
  invalidatedAt?: string | null;
  invalidationReason?: string | null;
};

export type ProjectSnapshot = {
  id: string;
  name: string;
  createdAt: string;
  updatedAt: string;
  rootPath: string;
  expectedQuestionCount?: number | null;
  examPackageFreeze?: ExamPackageFreeze | null;
  sections: Section[];
  schoolClasses: SchoolClass[];
  studentScanBatches: StudentScanBatch[];
  students: Student[];
  studentSubmissions: StudentSubmission[];
  studentAnswerOcrRecords: StudentAnswerOcrRecord[];
  scoringRecords: ScoringRecord[];
  speakingExams: SpeakingExam[];
  studentAnswerCropTemplate: StudentAnswerCropTemplate;
  studentIdentityCropTemplate?: StudentIdentityCropTemplate | null;
  studentScanDocumentId?: string | null;
  studentGroupingMode?: PageGroupingMode | null;
  studentPagesPerStudent?: number | null;
  studentGroupingCompleteAt?: string | null;
  latestScoringRunId?: string | null;
  documents: Document[];
  questions: Question[];
  workflow: WorkflowSnapshot;
};

export type ProjectListItem = {
  id: string;
  name: string;
  path: string;
  createdAt?: string;
  updatedAt?: string;
  questionCount?: number;
  documentRoles?: string[];
  statusSummary?: {
    hasExamSource: boolean;
    hasAnswerKeyOrRubric: boolean;
    hasStudentScan: boolean;
    questionTextCoverage?: string;
    rubricCoverage?: string;
  };
};

export type ListProjectsSkippedProject = {
  path: string;
  reason: string;
  technicalDetails?: string | null;
};

export type ListProjectsOutput = {
  projects: ProjectListItem[];
  warnings: string[];
  skippedProjects: ListProjectsSkippedProject[];
};

export type RemoveDocumentInput = {
  projectId: string;
  documentId: string;
};

export type CreateProjectInput = {
  name: string;
  rootPath: string;
};

export type CreateProjectOutput = {
  project: ProjectSnapshot;
  projectPath: string;
  warnings: string[];
};

export type OpenProjectInput = {
  projectPath?: string;
  rootPath?: string;
};

export type OpenProjectOutput = {
  project: ProjectSnapshot;
  projectPath: string;
  warnings: string[];
};

export type ModelSuggestedAction = {
  code: string;
  label: string;
};

export type ModelStatus = {
  profileId: string;
  displayName: string;
  mode: 'external' | 'managed';
  baseUrl: string;
  serverPathExists: boolean;
  modelPathExists: boolean;
  mmprojPathExists: boolean;
  serverRunning: boolean;
  healthOk: boolean;
  completionProbeOk: boolean;
  managedProcessPid?: number | null;
  startedByApp: boolean;
  logPath?: string | null;
  lastError?: AppError | null;
  warnings: string[];
  
  canStartFromApp: boolean;
  canStopFromApp: boolean;
  startRequiresModeChange: boolean;
  startDisabledReason?: string | null;
  suggestedActions: ModelSuggestedAction[];
};

export type ImportDocumentInput = {
  projectId: string;
  sourcePath: string;
};

export type ModelServerArgsPreview = {
  profileId: string;
  displayName: string;
  mode: 'external' | 'managed';
  baseUrl: string;
  command: string;
  args: string[];
  supportedFlags: string[];
  unsupportedFlags: string[];
  logPath: string;
};

export type StartModelServerOutput = {
  started: boolean;
  mode: 'managed';
  pid?: number | null;
  baseUrl: string;
  logPath: string;
  healthOk: boolean;
  message: string;
};

export type StopModelServerOutput = {
  stopped: boolean;
  message: string;
};

export type QuestionTextSource = 'exam_pdf';

export type PageGroupingMode = 'one_pdf_one_student' | 'fixed_pages_per_student' | 'manual';

export type Student = {
  id: string;
  displayName: string | null;
  number: string | null;
  className: string | null;
  warnings: string[];
  identityOcr?: StudentIdentityOcrRecord | null;
};

export type StudentSubmissionStatus =
  | 'grouped'
  | 'identity_missing'
  | 'ready_for_ocr'
  | 'ocr_running'
  | 'ocr_suggested'
  | 'ocr_confirmed'
  | 'failed';

export type StudentAnswerSlotStatus = 'empty' | 'pending_ocr' | 'ocr_suggested' | 'confirmed' | 'edited' | 'failed';

export type StudentAnswerOcrStatus =
  | 'pending'
  | 'running'
  | 'succeeded'
  | 'partial'
  | 'failed'
  | 'review_needed'
  | 'parse_failed'
  | 'crop_missing'
  | 'partial_answer_suspected'
  | 'printed_text_leak_suspected'
  | 'model_error'
  | 'teacher_corrected'
  | 'teacher_approved';

export type StudentAnswerOcrCropBBox = {
  x: number;
  y: number;
  width: number;
  height: number;
  pageIndex: number;
};

export type StudentAnswerCropTemplateItem = {
  questionId: string;
  questionNumber: number;
  pageIndexWithinSubmission: number;
  bbox: StudentAnswerOcrCropBBox;
  label?: string | null;
  note?: string | null;
};

export type StudentAnswerCropTemplate = {
  items: StudentAnswerCropTemplateItem[];
  updatedAt?: string | null;
};

export type StudentIdentityCropTemplate = {
  pageIndexWithinSubmission: number;
  bbox: StudentAnswerOcrCropBBox;
  label: string;
  note?: string | null;
  updatedAt?: string | null;
};

export type OcrImagePreprocessMode =
  | 'original'
  | 'clean_grayscale'
  | 'handwriting_enhanced'
  | 'high_contrast'
  | 'high_contrast_bw'
  | 'high_contrast_bw_optional';

export type OcrImagePreprocessDiagnostics = {
  mode: OcrImagePreprocessMode;
  preprocessVersion: string;
  sourceImagePath: string;
  outputImagePath: string;
  sourceWidth: number;
  sourceHeight: number;
  outputWidth: number;
  outputHeight: number;
  sourceBytes: number;
  outputBytes: number;
  cacheHit: boolean;
  applied: boolean;
  warnings: string[];
  errorMessage?: string | null;
  technicalDetails?: string | null;
};

export type OcrImagePreprocessResult = {
  outputImagePath: string;
  diagnostics: OcrImagePreprocessDiagnostics;
};

export type StudentIdentityOcrRecord = {
  displayName?: string | null;
  number?: string | null;
  className?: string | null;
  confidence: number;
  needsReview: boolean;
  warnings: string[];
  rawModelOutput: string;
  cropRefs: string[];
  originalCropRefs?: string[];
  preprocessedCropRefs?: string[];
  modelInputCropRef?: string | null;
  preprocessMode?: OcrImagePreprocessMode | null;
  preprocessVersion?: string | null;
  preprocessApplied?: boolean;
  preprocessWarnings?: string[];
  preprocessDiagnostics?: OcrImagePreprocessDiagnostics[] | null;
  availablePreprocessVariants?: OcrImagePreprocessMode[];
  sourcePageNumbers: number[];
  modelRequestMetadata?: unknown | null;
  createdAt: string;
  updatedAt: string;
};

export type StudentAnswerOcrParseDiagnostics = {
  rawModelOutput: string;
  parseError?: string | null;
  parsedJson?: unknown | null;
  salvagedAnswerText?: string | null;
  parseStrategy: string;
  modelRequestMetadata?: unknown | null;
};

export type OcrUncertainSpan = {
  text: string;
  start?: number | null;
  end?: number | null;
  alternatives: string[];
  confidence?: number | null;
  reason: string;
  highlightRegion?: StudentAnswerOcrCropBBox | null;
};

export type OcrSuggestedCorrection = {
  originalText: string;
  suggestedText: string;
  reason: string;
  confidence?: number | null;
  applied: boolean;
  highlightRegion?: StudentAnswerOcrCropBBox | null;
};

export type OcrCriticalTermWarning = {
  observedText: string;
  expectedOrRelatedTerm: string;
  reason: string;
  warningCode: string;
  highlightRegion?: StudentAnswerOcrCropBBox | null;
};

export type StudentAnswerOcrRenderDiagnostics = {
  cropRefs: string[];
  fullPagePreviewRefs: string[];
  cropBBox?: StudentAnswerOcrCropBBox | null;
  cropWidth?: number | null;
  cropHeight?: number | null;
  sourcePageCount?: number | null;
  answerRegionSource?: string | null;
  questionRegionStart?: number | null;
  questionRegionEnd?: number | null;
  nextQuestionAnchor?: string | null;
  cropWasClamped: boolean;
  cropMarginApplied: boolean;
  renderedCropExists: boolean;
  renderedPagePreviewExists: boolean;
  cropMissing: boolean;
  pagePreviewMissing: boolean;
  partialAnswerSuspected: boolean;
  printedTextMixed: boolean;
  printedQuestionLeakDetected: boolean;
};

export type StudentAnswerOcrRecord = {
  id: string;
  submissionId: string;
  questionId: string;
  questionNumber: number;
  sourcePageNumbers: number[];
  sourceImageRefs: string[];
  cropRefs: string[];
  originalCropRefs?: string[];
  preprocessedCropRefs?: string[];
  modelInputCropRef?: string | null;
  preprocessMode?: OcrImagePreprocessMode | null;
  preprocessVersion?: string | null;
  preprocessApplied?: boolean;
  preprocessWarnings?: string[];
  preprocessDiagnostics?: OcrImagePreprocessDiagnostics[] | null;
  availablePreprocessVariants?: OcrImagePreprocessMode[];
  fullPagePreviewRefs: string[];
  answerText: string;
  structuredAnswer?: unknown | null;
  confidence?: number | null;
  uncertainSpans: OcrUncertainSpan[];
  suggestedCorrections: OcrSuggestedCorrection[];
  criticalTermWarnings: OcrCriticalTermWarning[];
  ocrSemanticWarnings: string[];
  criticalKeywordUncertain: boolean;
  status: StudentAnswerOcrStatus;
  needsReview: boolean;
  reviewReasons: string[];
  warnings: string[];
  modelName?: string | null;
  promptVersion: string;
  createdAt: string;
  updatedAt: string;
  teacherCorrectedText?: string | null;
  teacherReviewedAt?: string | null;
  parseDiagnostics?: StudentAnswerOcrParseDiagnostics | null;
  renderDiagnostics?: StudentAnswerOcrRenderDiagnostics | null;
};

export type PreprocessOcrImageInput = {
  projectId: string;
  imagePath: string;
  mode?: OcrImagePreprocessMode;
};

export type StudentAnswerSlot = {
  questionId: string;
  questionNumber: number;
  status: StudentAnswerSlotStatus;
  text: string | null;
  confidence: number | null;
  warnings: string[];
};

export type StudentSubmission = {
  id: string;
  studentId: string;
  documentId: string;
  classId?: string | null;
  scanBatchId?: string | null;
  classMembershipSource?: 'inherited_from_batch' | 'teacher_override' | null;
  pageNumbers: number[];
  status: StudentSubmissionStatus;
  answerSlots: StudentAnswerSlot[];
  warnings: string[];
  updatedAt: string | null;
};

export type CreateStudentPageGroupsInput = {
  projectId: string;
  documentId: string;
  pagesPerStudent: number;
  batchId?: string;
};

export type CreateStudentPageGroupsOutput = {
  groupsCreated: number;
  totalPages: number;
  pagesPerStudent: number;
  remainderPages: number;
  needsReview: boolean;
  submissions: StudentSubmission[];
  warnings: string[];
};

export type UpdateStudentIdentityInput = {
  projectId: string;
  submissionId: string;
  displayName?: string | null;
  number?: string | null;
  className?: string | null;
};

export type UpdateSubmissionPagesInput = {
  projectId: string;
  submissionId: string;
  pageNumbers: number[];
};

export type DeleteStudentSubmissionInput = {
  projectId: string;
  submissionId: string;
};

export type MarkStudentGroupingCompleteInput = {
  projectId: string;
  batchId?: string;
};

export type StudentScanReadinessSnapshot = {
  projectId: string;
  documentId?: string | null;
  ready: boolean;
  currentStage: string;
  blockingReasons: string[];
  nextActions: string[];
  submissionCount: number;
  previewReady: boolean;
  groupingComplete: boolean;
  warnings: string[];
  message: string;
};

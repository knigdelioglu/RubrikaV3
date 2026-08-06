import type { ModelInvocationContract, ModelProvenance, OcrReviewPolicyDto } from './types.model';
import type { StudentAnswerOcrStatus } from './types.student';

export type StartStudentAnswerOcrOutput = {
  jobId: string;
  status: 'queued' | 'running';
  rerun: boolean;
  mode: StudentAnswerOcrJobMode;
};

export type OcrGenerationStatus =
  | 'candidate'
  | 'ready_for_review'
  | 'active'
  | 'rejected'
  | 'failed'
  | 'stale'
  | 'interrupted'
  | 'superseded';

export type OcrTeacherReviewStatus = 'not_required' | 'pending' | 'approved' | 'rejected';

export type OcrGeneration = {
  generationId: string;
  submissionId: string;
  sourceFingerprint: string;
  createdAt: string;
  modelName?: string | null;
  promptVersion: string;
  status: OcrGenerationStatus;
  result: StudentAnswerOcrRecord[];
  diagnostics?: unknown | null;
  teacherReviewStatus: OcrTeacherReviewStatus;
  createdByJobId: string;
  sourceDocumentId: string;
  sourceStorageRevision: number;
  failureReason?: string | null;
  jobMode?: StudentAnswerOcrJobMode;
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

export type StudentAnswerOcrCropBBox = {
  x: number;
  y: number;
  width: number;
  height: number;
  pageIndex: number;
};

export type NormalizedBBox = Omit<StudentAnswerOcrCropBBox, 'pageIndex'>;

export type AnswerRegionRole = 'primary' | 'continuation' | 'supporting';
export type ContinuationPolicy = 'independent' | 'continues_previous' | 'optional';

export type QuestionAnswerRegion = {
  regionId: string;
  pageOffset: number;
  order: number;
  normalizedBBox: NormalizedBBox;
  regionRole: AnswerRegionRole;
  continuationPolicy: ContinuationPolicy;
  label?: string | null;
  note?: string | null;
};

export type QuestionAnswerTemplate = {
  questionId: string;
  regions: QuestionAnswerRegion[];
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
  templates: QuestionAnswerTemplate[];
  updatedAt?: string | null;
};

export type StudentAnswerOcrJobMode = 'production' | 'experimental_full_page_review_only';

export type StructuredAnswer =
  | { kind: 'multiple_choice'; selections: Array<{ option: string; selected: boolean }> }
  | { kind: 'matching'; pairs: Array<{ left: string; right: string }> }
  | { kind: 'ordered_slots'; slots: Array<{ index: number; value: string }> }
  | { kind: 'numeric'; value?: string | null; unit?: string | null }
  | { kind: 'table'; rows: Array<{ index: number; cells: string[] }> }
  | { kind: 'correction_table'; rows: Array<{ index: number; original: string; correction: string; explanation?: string | null }> }
  | { kind: 'sentence_annotation'; annotations: Array<{ text: string; annotation: string; start?: number | null; end?: number | null }> }
  | { kind: 'grammar_analysis'; items: Array<{ text: string; label: string; explanation?: string | null }> }
  | { kind: 'open_text'; text: string }
  | { kind: 'legacy_unparsed'; raw: unknown; reason: string };

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
  modelProvenance?: ModelProvenance | null;
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
  regionIds: string[];
  regionOrders: number[];
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

export type OcrRegionProvenance = { regionId: string; order: number; pageOffset: number };
export type OcrResizeDimensions = { width: number; height: number };
export type OcrInputBudget = {
  maxTokens?: number | null;
  timeoutSeconds?: number | null;
  maxImages?: number | null;
  maxInputBytes?: number | null;
  actualImageCount: number;
  actualInputBytes: number;
};
export type StudentAnswerOcrProvenance = {
  schemaVersion: string;
  sourceChecksum?: string | null;
  sourcePageNumbers: number[];
  regionIds: string[];
  regionOrders: number[];
  regions: OcrRegionProvenance[];
  renderDpi?: number | null;
  renderer?: string | null;
  preprocessPolicy?: string | null;
  preprocessVariant?: OcrImagePreprocessMode | null;
  preprocessVersion?: string | null;
  resizeDimensions: OcrResizeDimensions[];
  jpegCacheKeys: string[];
  invocation?: ModelInvocationContract | null;
  budget?: OcrInputBudget | null;
  responseDiagnostics?: unknown | null;
  approvableForScoring: boolean;
  provenanceNotes: string[];
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
  structuredAnswer?: StructuredAnswer | null;
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
  reviewPolicy?: OcrReviewPolicyDto | null;
  modelProvenance?: ModelProvenance | null;
  ocrProvenance?: StudentAnswerOcrProvenance | null;
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

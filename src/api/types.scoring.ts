export type ScoringReviewStatus = 'pending_review' | 'approved' | 'edited' | 'invalidated';
export type ScoringDecisionState =
  | 'provisional'
  | 'model_candidate'
  | 'deterministic_accepted'
  | 'auto_accepted'
  | 'teacher_approved'
  | 'rejected'
  | 'failed';

export type ScoringCriterionScore = {
  criterionId: string;
  criterionTitle: string;
  criterionMaxScore: number;
  awardedScore: number;
  rationale: string;
  evidenceQuote?: string | null;
};

export type SemanticCriterionDecision = {
  criterionId: string;
  levelId: string;
  exactEvidence?: string | null;
  missingRequirements: string[];
  contradiction: boolean;
  rationale: string;
};

export type ScoringExecutionDiagnostics = {
  kind: 'deterministic' | 'model' | 'candidate_cache' | 'exact_duplicate_reuse';
  modelCalled: boolean;
  modelCallCount: number;
  scorerVersion: string;
  policyVersion: string;
  cacheHit: boolean;
  cacheFingerprint?: string | null;
  notes: string[];
};

export type ScoringCacheProvenance = {
  fingerprint: string;
  artifactSchemaVersion: string;
  cacheHit: boolean;
  source: string;
  artifactPath?: string | null;
};

export type ScoringReuseProvenance = {
  sourceRecordId: string;
  sourceDecisionVersion: string;
  targetDecisionVersion: string;
  matchKey: string;
  reason: string;
};

export type ScoringConsistencyReview = {
  reasonCode: string;
  teacherMessage: string;
  clusterKey: string;
  conflictingRecordIds: string[];
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
  decisionState: ScoringDecisionState;
  decisionVersion?: string;
  criterionScores: ScoringCriterionScore[];
  semanticDecisions?: SemanticCriterionDecision[];
  rationale: string;
  confidence: number;
  needsReview: boolean;
  reviewReasons: string[];
  warnings: string[];
  rawModelOutput: string;
  parseDiagnostics?: ScoringParseDiagnostics | null;
  reconciliationDiagnostics?: ScoringReconciliationDiagnostics | null;
  executionDiagnostics?: ScoringExecutionDiagnostics | null;
  cacheProvenance?: ScoringCacheProvenance | null;
  reuseProvenance?: ScoringReuseProvenance | null;
  consistencyReview?: ScoringConsistencyReview | null;
  scoringFingerprint?: string;
  policyVersion?: string;
  answerNormalizedHash?: string;
  answerRawHash?: string;
  ocrGeneration?: string;
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

export type ScoringAnchorStatus = 'active' | 'revoked';
export type ScoringAnchorEligibility = 'eligible' | 'stale' | 'ineligible' | 'revoked';
export type ScoringAnchorActionKind = 'created' | 'revoked';

export type ScoringAnchorAction = {
  action: ScoringAnchorActionKind;
  actorKind: 'teacher' | string;
  occurredAt: string;
  reason?: string | null;
};

export type ScoringAnchorEvidence = {
  answerNormalizedHash: string;
  answerRawHash: string;
  ocrRecordHash: string;
  awardedScore: number;
  maxScore: number;
  rationale: string;
  criterionScores: ScoringCriterionScore[];
  teacherNotes?: string | null;
};

export type ScoringAnchor = {
  id: string;
  version: string;
  sourceRecordId: string;
  questionId: string;
  questionNumber: number;
  qepFingerprint: string;
  questionTextHash: string;
  rubricHash: string;
  policyVersion: string;
  scoringFingerprint: string;
  calibrationVersion: string;
  finalScore: number;
  maxScore: number;
  evidence: ScoringAnchorEvidence;
  status: ScoringAnchorStatus;
  actions: ScoringAnchorAction[];
  createdAt: string;
  revokedAt?: string | null;
  revokedReason?: string | null;
  eligibility: ScoringAnchorEligibility;
  eligibilityReasons: string[];
};

export type ScoringSubmissionSummary = {
  submissionId: string;
  provisionalScore: number;
  acceptedScore: number;
  finalScore: number | null;
  maxScore: number;
  isComplete: boolean;
  expectedRecordCount: number;
  acceptedRecordCount: number;
  provisionalRecordCount: number;
  reviewRequiredCount: number;
};

export type ScoringSummaryDto = {
  provisionalScore: number;
  acceptedScore: number;
  finalScore: number | null;
  maxScore: number;
  isComplete: boolean;
  expectedRecordCount: number;
  acceptedRecordCount: number;
  provisionalRecordCount: number;
  reviewRequiredCount: number;
  submissions: ScoringSubmissionSummary[];
};

export type StartScoringOutput = {
  jobId: string;
  status: 'queued' | 'running';
  rerun: boolean;
};

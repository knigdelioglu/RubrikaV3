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
  assessmentActivityId?: string;
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
  assessmentActivityId?: string;
  classApplicationId?: string;
  studentId: string;
  action: 'start' | 'pause' | 'resume' | 'stop' | 'cancel';
};

export type ToggleSpeakingCaptureOutput = {
  action: 'start' | 'pause' | 'resume' | 'stop' | 'cancel';
  accepted: boolean;
  attemptId?: string | null;
  message: string;
};

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
  assessmentActivityId?: string | null;
  classApplicationId?: string | null;
  schoolClassId?: string | null;
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
  speakingConfigSnapshot?: SpeakingConfigurationSnapshot | null;
};

export type SpeakingExam = {
  id: string;
  assessmentActivityId?: string | null;
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
  activeClassApplicationId?: string | null;
  completedAt?: string | null;
  attempts: SpeakingAttempt[];
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

export type SpeakingConfigurationSnapshot = {
  speakingType: string;
  taskText: string;
  targetDurationSeconds: number;
  minDurationSeconds: number;
  maxDurationSeconds: number;
  rubricVersion: string;
  scoringPolicyVersion: string;
  cleanupPromptVersion: string;
  evaluationPromptVersion: string;
  frozenModelFileHash?: string | null;
  rubricSnapshot: unknown;
};

import type { RubricState } from './types.rubric';

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

export type StartExamPackageBuildInput = {
  projectId: string;
  expectedQuestionCount: number;
};

export type StartExamPackageBuildOutput = {
  jobId: string;
  status: 'queued' | 'running';
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

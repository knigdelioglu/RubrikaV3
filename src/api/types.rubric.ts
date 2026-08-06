import type { AnswerType, Question } from './types.question';

export type RubricSource = 'manual' | 'json' | 'answer_key_pdf' | 'rubric_pdf' | 'generated' | 'gemma_draft' | 'unknown';

export type RubricStatus = 'missing' | 'suggested' | 'imported' | 'manual' | 'confirmed' | 'invalid' | 'legacy';

export type RubricLevel = {
  id: string;
  title: string;
  requiredConditions: string[];
  disqualifyingConditions: string[];
  score: number;
  evidenceRequired: boolean;
  version: string;
};

export type RubricCriterion = {
  id: string;
  label: string;
  description: string;
  points: number;
  /** Optional while opening legacy numeric-only project files. */
  levels?: RubricLevel[];
};

export type RubricState = {
  source?: RubricSource | null;
  maxScore?: number | null;
  expectedAnswer?: string | null;
  keyConcepts: string[];
  criteria: RubricCriterion[];
  partialCreditHints: string[];
  zeroScoreConditions: string[];
  commonMistakes: string[];
  status: RubricStatus;
  warnings: string[];
  updatedAt?: string;
};

export type MigrateRubricLevelsOutput = {
  migratedCount: number;
  teacherConfirmationRequired: boolean;
  qepInvalidated: boolean;
  warnings: string[];
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
  keyConcepts: string[];
  criteria: RubricCriterion[];
  partialCreditHints: string[];
  zeroScoreConditions: string[];
  commonMistakes: string[];
};

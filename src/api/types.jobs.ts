import type { AppError } from './errors';

export type JobKind =
  | 'document_import'
  | 'question_text_extraction'
  | 'pdf_preview_render'
  | 'rubric_pdf_import'
  | 'exam_package_build'
  | 'student_answer_ocr'
  | 'student_identity_ocr'
  | 'scoring'
  | 'speaking_evaluation'
  | 'assessment_analysis'
  | 'project_backup'
  | 'project_restore'
  | 'project_recovery';

export type JobStatus =
  | 'queued'
  | 'running'
  | 'succeeded'
  | 'partial'
  | 'failed'
  | 'cancelled'
  | 'interrupted';

export type JobProgress = {
  current: number;
  total: number;
  message: string;
};

export type JobSnapshot = {
  id: string;
  schemaVersion?: number;
  projectId: string;
  kind: JobKind;
  displayLabel?: string;
  status: JobStatus;
  cancellationRequested?: boolean;
  cancellationRequestedAt?: string;
  progress: JobProgress;
  startedAt?: string;
  finishedAt?: string;
  lastMessage?: string;
  correlationId?: string;
  idempotencyKey?: string;
  cancellable?: boolean;
  retryOfJobId?: string;
  result?: unknown;
  error?: AppError;
  createdAt: string;
  updatedAt: string;
};

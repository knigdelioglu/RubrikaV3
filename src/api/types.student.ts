import type { OcrReviewPolicyDto } from './types.model';
import type { StudentIdentityOcrRecord } from './types.ocr';

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

export type Section = {
  id: string;
  name: string;
  students: Student[];
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
  ocrReviewPolicy?: OcrReviewPolicyDto;
};

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

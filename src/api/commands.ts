import { invoke } from '@tauri-apps/api/core';
import type {
  AppStatus,
  WorkflowSnapshot,
  ProjectSnapshot,
  CreateProjectOutput,
  ListProjectsOutput,
  CreateProjectInput,
  OpenProjectInput,
  OpenProjectOutput,
  ModelStatus,
  Document,
  JobSnapshot,
  Question,
  ModelServerArgsPreview,
  StartModelServerOutput,
  StopModelServerOutput,
  PdfPagePreview,
  PdfPreviewStatusSnapshot,
  StartPdfPreviewRenderOutput,
  PdfRendererStatus,
  QuestionTextExtractionStatus,
  QuestionTextSuggestion,
  ImportRubricJsonInput,
  ImportRubricJsonOutput,
  RubricStateSnapshot,
  RubricValidationReport,
  UpdateQuestionRubricInput,
  StartRubricPdfImportInput,
  StartExamPackageBuildInput,
  StartExamPackageBuildOutput,
  StartStudentAnswerOcrOutput,
  StartStudentIdentityOcrOutput,
  RebuildStudentAnswerOcrIssuesOutput,
  StartScoringOutput,
  RubricQuestionSnapshot,
  CreateStudentPageGroupsInput,
  CreateStudentPageGroupsOutput,
  DeleteStudentSubmissionInput,
  MarkStudentGroupingCompleteInput,
  StudentSubmission,
  StudentAnswerCropTemplateItem,
  StudentIdentityCropTemplate,
  StudentAnswerOcrRecord,
  SuggestStudentAnswerOcrIssueCorrectionWithModelInput,
  SuggestStudentAnswerOcrIssueCorrectionWithModelOutput,
  StudentScanReadinessSnapshot,
  UpdateStudentIdentityInput,
  UpdateSubmissionPagesInput,
  ScoringRecord,
  OcrImagePreprocessResult,
  PreprocessOcrImageInput,
  GradedExamReview,
  SchoolClass,
  SchoolClassOverviewSnapshot,
  StudentScanBatch,
  ImportStudentScanBatchInput,
  ImportStudentScanBatchOutput,
  PageGroupingMode,
  StartSpeakingExamInput,
  StartSpeakingExamOutput,
  ToggleSpeakingCaptureOutput,
  SpeakingExam,
  SpeakingAttemptSyncOutput,
  SpeakingAttempt,
  SpeakingEngineRuntimeStatus,
  MicrophoneDevice,
  SpeakingPerformanceLevel,
  Student,
  AssessmentKind,
  AssessmentAnalysis,
  FinishAssessmentOutput,
  AssessmentActivity,
  AssessmentSequenceOptions,
  AssessmentClassApplication,
  AssessmentType,
  AssessmentStatus,
  ListeningDetails,
  SpeakingConfigurationSnapshot,
  TeachingAssignment,
  UpdateCourseInfoInput,
  BatchCreateTeachingAssignmentsInput,
  GcReport,
  StartBackupJobOutput,
  StartRestoreJobOutput,
  StartRecoveryCopyJobOutput,
  DataLossPreflightReport,
} from './types';
import type { AppError } from './errors';

function handleInvokeError(e: unknown): never {
  const msg = String(e);
  if (msg.includes('command') && msg.includes('not found')) {
    throw {
      code: 'COMMAND_NOT_AVAILABLE',
      safeMessage: 'Bu komut henüz backend tarafında uygulanmadı.',
      recoveryAction: 'Uygulamayı güncelleyip yeniden deneyin.',
      correlationId: crypto.randomUUID?.() || 'unknown',
      retryable: true,
      detailsAvailable: false,
    } as AppError;
  }
  
  if (typeof e === 'object' && e !== null && 'code' in e) {
    throw e as AppError;
  }

  throw {
    code: 'UNKNOWN_ERROR',
    safeMessage: 'Bilinmeyen bir hata oluştu.',
    recoveryAction: undefined,
    correlationId: crypto.randomUUID?.() || 'unknown',
    retryable: false,
    detailsAvailable: false,
  } as AppError;
}

export const commands = {
  updateCourseInfo: async (input: UpdateCourseInfoInput): Promise<ProjectSnapshot> => {
    try {
      return await invoke<ProjectSnapshot>('update_course_info', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  batchCreateTeachingAssignments: async (input: BatchCreateTeachingAssignmentsInput): Promise<TeachingAssignment[]> => {
    try {
      return await invoke<TeachingAssignment[]>('batch_create_teaching_assignments', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getAppStatus: async (): Promise<AppStatus> => {
    try {
      return await invoke<AppStatus>('get_app_status');
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getWorkflowSnapshot: async (projectId?: string): Promise<WorkflowSnapshot> => {
    try {
      return await invoke<WorkflowSnapshot>('get_workflow_snapshot', { input: { projectId } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  createProject: async (input: CreateProjectInput): Promise<CreateProjectOutput> => {
    try {
      return await invoke<CreateProjectOutput>('create_project', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  listProjects: async (): Promise<ListProjectsOutput> => {
    try {
      return await invoke<ListProjectsOutput>('list_projects');
    } catch (e) {
      handleInvokeError(e);
    }
  },
  openProject: async (input: OpenProjectInput): Promise<OpenProjectOutput> => {
    try {
      return await invoke<OpenProjectOutput>('open_project', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  migrateProjectWithVerifiedBackup: async (projectPath: string): Promise<OpenProjectOutput> => {
    try {
      return await invoke<OpenProjectOutput>('migrate_project_with_verified_backup', {
        input: { projectPath },
      });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getDataLossPreflight: async (projectPath: string): Promise<DataLossPreflightReport> => {
    try {
      return await invoke<DataLossPreflightReport>('get_data_loss_preflight', {
        input: { projectPath },
      });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getProjectSnapshot: async (projectId: string): Promise<ProjectSnapshot> => {
    try {
      return await invoke<ProjectSnapshot>('get_project_snapshot', { input: { projectId } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  startSpeakingExam: async (input: StartSpeakingExamInput): Promise<StartSpeakingExamOutput> => {
    try {
      return await invoke<StartSpeakingExamOutput>('start_speaking_exam', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  listSpeakingExamMicrophones: async (): Promise<MicrophoneDevice[]> => {
    try {
      return await invoke<MicrophoneDevice[]>('list_speaking_exam_microphones');
    } catch (e) {
      handleInvokeError(e);
    }
  },
  selectSpeakingExamMicrophone: async (microphoneId: string): Promise<void> => {
    try {
      await invoke('select_speaking_exam_microphone', { input: { microphoneId } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getSpeakingExamRuntimeStatus: async (): Promise<SpeakingEngineRuntimeStatus> => {
    try {
      return await invoke<SpeakingEngineRuntimeStatus>('get_speaking_exam_runtime_status');
    } catch (e) {
      handleInvokeError(e);
    }
  },
  toggleSpeakingCapture: async (input: { projectId: string; examId: string; assessmentActivityId?: string; classApplicationId?: string; studentId: string; action: 'start' | 'pause' | 'resume' | 'stop' | 'cancel' }): Promise<ToggleSpeakingCaptureOutput> => {
    try {
      return await invoke<ToggleSpeakingCaptureOutput>('toggle_speaking_capture', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  startSpeakingExamAttempt: async (input: { projectId: string; examId: string; assessmentActivityId?: string; classApplicationId?: string; studentId: string }): Promise<ToggleSpeakingCaptureOutput> => {
    try {
      return await invoke<ToggleSpeakingCaptureOutput>('start_speaking_exam_attempt', { input: { ...input, action: 'start' } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  stopSpeakingExamAttempt: async (input: { projectId: string; examId: string; assessmentActivityId?: string; classApplicationId?: string; studentId: string }): Promise<ToggleSpeakingCaptureOutput> => {
    try {
      return await invoke<ToggleSpeakingCaptureOutput>('stop_speaking_exam_attempt', { input: { ...input, action: 'stop' } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  pauseSpeakingExamAttempt: async (input: { projectId: string; examId: string; assessmentActivityId?: string; classApplicationId?: string; studentId: string }): Promise<ToggleSpeakingCaptureOutput> => {
    try {
      return await invoke<ToggleSpeakingCaptureOutput>('pause_speaking_exam_attempt', { input: { ...input, action: 'pause' } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  resumeSpeakingExamAttempt: async (input: { projectId: string; examId: string; assessmentActivityId?: string; classApplicationId?: string; studentId: string }): Promise<ToggleSpeakingCaptureOutput> => {
    try {
      return await invoke<ToggleSpeakingCaptureOutput>('resume_speaking_exam_attempt', { input: { ...input, action: 'resume' } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  cancelSpeakingExamAttempt: async (input: { projectId: string; examId: string; assessmentActivityId?: string; classApplicationId?: string; studentId: string }): Promise<ToggleSpeakingCaptureOutput> => {
    try {
      return await invoke<ToggleSpeakingCaptureOutput>('cancel_speaking_exam_attempt', { input: { ...input, action: 'cancel' } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getSpeakingExam: async (projectId: string, examId: string, assessmentActivityId?: string, classApplicationId?: string): Promise<SpeakingExam> => {
    try {
      return await invoke<SpeakingExam>('get_speaking_exam', { input: { projectId, examId, assessmentActivityId, classApplicationId } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  syncSpeakingAttempt: async (input: { projectId: string; examId: string; attemptId: string }): Promise<SpeakingAttemptSyncOutput> => {
    try {
      return await invoke<SpeakingAttemptSyncOutput>('sync_speaking_attempt', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  updateSpeakingCriterionScore: async (input: { projectId: string; examId: string; attemptId: string; criterionId: string; score: number; note?: string }): Promise<SpeakingAttempt> => {
    try {
      return await invoke<SpeakingAttempt>('update_speaking_criterion_score', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  updateSpeakingCriterionLevel: async (input: {
    projectId: string;
    examId: string;
    attemptId: string;
    criterionId: string;
    level: SpeakingPerformanceLevel;
    note?: string;
  }): Promise<SpeakingAttempt> => {
    try {
      return await invoke<SpeakingAttempt>('update_speaking_criterion_level', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  selectSpeakingExamClass: async (input: {
    projectId: string;
    examId: string;
    classId?: string;
    assessmentActivityId?: string;
    classApplicationId?: string;
  }): Promise<SpeakingExam> => {
    try {
      return await invoke<SpeakingExam>('select_speaking_exam_class', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  selectSpeakingExamStudent: async (input: {
    projectId: string;
    examId: string;
    assessmentActivityId?: string;
    classApplicationId?: string;
    studentId: string;
  }): Promise<SpeakingExam> => {
    try {
      return await invoke<SpeakingExam>('select_speaking_exam_student', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  updateSpeakingAttemptNote: async (input: {
    projectId: string;
    examId: string;
    attemptId: string;
    teacherNote?: string;
  }): Promise<SpeakingAttempt> => {
    try {
      return await invoke<SpeakingAttempt>('update_speaking_attempt_note', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  approveSpeakingAttempt: async (input: { projectId: string; examId: string; attemptId: string; teacherNote?: string }): Promise<SpeakingAttempt> => {
    try {
      return await invoke<SpeakingAttempt>('approve_speaking_attempt', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getDefaultProjectPath: async (projectName: string, academicYearId?: string): Promise<{ path: string }> => {
    try {
      return await invoke<{ path: string }>('get_default_project_path', {
        input: { projectName, academicYearId },
      });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  importExamSourcePdf: async (input: { projectId: string; sourcePath: string }) => {
    try {
      return await invoke<Document>('import_exam_source_pdf', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  importAnswerKeyPdf: async (input: { projectId: string; sourcePath: string }) => {
    try {
      return await invoke<Document>('import_answer_key_pdf', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  listDocuments: async (projectId: string): Promise<Document[]> => {
    try {
      return await invoke<Document[]>('list_documents', { input: { projectId } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  removeDocument: async (input: { projectId: string; documentId: string }) => {
    try {
      return await invoke<void>('remove_document', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  importStudentScanPdf: async (input: { projectId: string; sourcePath: string }) => {
    try {
      return await invoke<Document>('import_student_scan_pdf', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  listSchoolClasses: async (input: { projectId: string; includeArchived?: boolean }): Promise<SchoolClass[]> => {
    try {
      return await invoke<SchoolClass[]>('list_school_classes', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  listClassStudents: async (input: { projectId: string; classId: string }): Promise<Student[]> => {
    try {
      return await invoke<Student[]>('list_class_students', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  createClassStudent: async (input: { projectId: string; classId: string; displayName?: string; number?: string }): Promise<Student> => {
    try {
      return await invoke<Student>('create_class_student', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  updateClassStudent: async (input: { projectId: string; classId: string; studentId: string; displayName?: string; number?: string }): Promise<Student> => {
    try {
      return await invoke<Student>('update_class_student', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  finishAssessment: async (input: {
    projectId: string;
    kind: AssessmentKind;
    sourceId?: string;
  }): Promise<FinishAssessmentOutput> => {
    try {
      return await invoke<FinishAssessmentOutput>('finish_assessment', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getAssessmentAnalysis: async (
    projectId: string,
    analysisId: string,
  ): Promise<AssessmentAnalysis> => {
    try {
      return await invoke<AssessmentAnalysis>('get_assessment_analysis', {
        input: { projectId, analysisId },
      });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  listAssessmentAnalyses: async (projectId: string): Promise<AssessmentAnalysis[]> => {
    try {
      return await invoke<AssessmentAnalysis[]>('list_assessment_analyses', {
        input: { projectId },
      });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  createSchoolClass: async (input: {
    projectId: string;
    name: string;
    academicYear?: string;
    gradeLevel?: number;
    section?: string;
    displayOrder?: number;
  }): Promise<SchoolClass> => {
    try {
      return await invoke<SchoolClass>('create_school_class', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  updateSchoolClass: async (input: {
    projectId: string;
    classId: string;
    name?: string;
    academicYear?: string;
    gradeLevel?: number;
    section?: string;
    displayOrder?: number;
  }): Promise<SchoolClass> => {
    try {
      return await invoke<SchoolClass>('update_school_class', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  archiveSchoolClass: async (input: { projectId: string; classId: string }): Promise<SchoolClass> => {
    try {
      return await invoke<SchoolClass>('archive_school_class', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  restoreSchoolClass: async (input: { projectId: string; classId: string }): Promise<SchoolClass> => {
    try {
      return await invoke<SchoolClass>('restore_school_class', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getSchoolClassOverview: async (projectId: string): Promise<SchoolClassOverviewSnapshot> => {
    try {
      return await invoke<SchoolClassOverviewSnapshot>('get_school_class_overview', { input: { projectId } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  listAssessmentActivities: async (input: {
    projectId: string;
    academicYearId?: string;
    courseId?: string;
    gradeLevel?: number;
    term?: number;
    assessmentType?: AssessmentType;
    status?: AssessmentStatus;
  }): Promise<AssessmentActivity[]> => {
    try {
      return await invoke<AssessmentActivity[]>('list_assessment_activities', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getAssessmentSequenceOptions: async (input: {
    projectId: string;
    academicYearId: string;
    courseId: string;
    term: number;
    assessmentType: AssessmentType;
  }): Promise<AssessmentSequenceOptions> => {
    try {
      return await invoke<AssessmentSequenceOptions>('get_assessment_sequence_options', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getAssessmentActivity: async (input: { projectId: string; activityId: string }): Promise<AssessmentActivity> => {
    try {
      return await invoke<AssessmentActivity>('get_assessment_activity', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  updateAssessmentActivity: async (input: { projectId: string; activityId: string; title?: string; speakingConfiguration?: SpeakingConfigurationSnapshot; status?: AssessmentStatus }): Promise<AssessmentActivity> => {
    try {
      return await invoke<AssessmentActivity>('update_assessment_activity', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getAssessmentClassApplications: async (input: { projectId: string; activityId: string }): Promise<AssessmentClassApplication[]> => {
    try {
      return await invoke<AssessmentClassApplication[]>('get_assessment_class_applications', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getClassApplicationStudents: async (input: { projectId: string; activityId: string; applicationId: string }): Promise<Student[]> => {
    try {
      return await invoke<Student[]>('get_class_application_students', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  listAssessmentClasses: async (input: {
    projectId: string;
    academicYearId: string;
    courseId: string;
    gradeLevel: number;
  }): Promise<SchoolClass[]> => {
    try {
      return await invoke<SchoolClass[]>('list_assessment_classes', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  createAssessmentActivity: async (input: {
    projectId: string;
    academicYearId: string;
    courseId: string;
    courseName: string;
    gradeLevel: number;
    term: number;
    assessmentType: AssessmentType;
    sequenceNumber: number;
    schoolClassIds: string[];
    title?: string;
    speakingConfiguration?: SpeakingConfigurationSnapshot;
    listeningDetails?: ListeningDetails;
  }): Promise<AssessmentActivity> => {
    try {
      return await invoke<AssessmentActivity>('create_assessment_activity', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  addAssessmentClassApplication: async (input: {
    projectId: string;
    activityId: string;
    schoolClassId: string;
    scheduledAt?: string;
    applicationDate?: string;
    notes?: string;
  }): Promise<AssessmentClassApplication> => {
    try {
      return await invoke<AssessmentClassApplication>('add_assessment_class_application', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  archiveAssessmentClassApplication: async (input: {
    projectId: string;
    activityId: string;
    applicationId: string;
  }): Promise<AssessmentClassApplication> => {
    try {
      return await invoke<AssessmentClassApplication>('archive_assessment_class_application', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  removeAssessmentClassApplication: async (input: { projectId: string; activityId: string; applicationId: string }): Promise<AssessmentClassApplication> => {
    try {
      return await invoke<AssessmentClassApplication>('remove_assessment_class_application', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  attachAssessmentDocument: async (input: {
    projectId: string;
    activityId: string;
    documentId: string;
    applicationId?: string;
  }): Promise<AssessmentActivity> => {
    try {
      return await invoke<AssessmentActivity>('attach_assessment_document', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  listTeachingAssignments: async (input: {
    projectId: string;
    academicYearId?: string;
    includeInactive?: boolean;
  }): Promise<TeachingAssignment[]> => {
    try {
      return await invoke<TeachingAssignment[]>('list_teaching_assignments', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  createTeachingAssignment: async (input: {
    projectId: string;
    academicYearId: string;
    courseId: string;
    courseName: string;
    classSectionId: string;
    teacherId?: string;
  }): Promise<TeachingAssignment> => {
    try {
      return await invoke<TeachingAssignment>('create_teaching_assignment', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  archiveTeachingAssignment: async (input: { projectId: string; assignmentId: string }): Promise<TeachingAssignment> => {
    try {
      return await invoke<TeachingAssignment>('archive_teaching_assignment', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  importStudentScanBatch: async (input: ImportStudentScanBatchInput): Promise<ImportStudentScanBatchOutput> => {
    try {
      return await invoke<ImportStudentScanBatchOutput>('import_student_scan_batch', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  createStudentScanBatch: async (input: {
    projectId: string;
    classId: string;
    documentId: string;
    displayName?: string;
    pagesPerStudent?: number;
    groupingMode?: PageGroupingMode;
  }): Promise<StudentScanBatch> => {
    try {
      return await invoke<StudentScanBatch>('create_student_scan_batch', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  listStudentScanBatches: async (input: { projectId: string; classId?: string }): Promise<StudentScanBatch[]> => {
    try {
      return await invoke<StudentScanBatch[]>('list_student_scan_batches', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  moveStudentScanBatch: async (input: {
    projectId: string;
    batchId: string;
    targetClassId: string;
  }): Promise<StudentScanBatch> => {
    try {
      return await invoke<StudentScanBatch>('move_student_scan_batch', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  removeStudentScanBatch: async (input: { projectId: string; batchId: string }): Promise<StudentScanBatch> => {
    try {
      return await invoke<StudentScanBatch>('remove_student_scan_batch', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  listStudentScanDocuments: async (projectId: string): Promise<Document[]> => {
    try {
      return await invoke<Document[]>('list_student_scan_documents', { input: { projectId } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getPdfPageCount: async (input: { projectId: string; documentId: string }): Promise<number> => {
    try {
      return await invoke<number>('get_pdf_page_count', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  startPdfPreviewRender: async (input: { projectId: string; documentId: string }): Promise<StartPdfPreviewRenderOutput> => {
    try {
      return await invoke<StartPdfPreviewRenderOutput>('start_pdf_preview_render', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getPdfPreviewStatus: async (input: { projectId: string; documentId: string }): Promise<PdfPreviewStatusSnapshot> => {
    try {
      return await invoke<PdfPreviewStatusSnapshot>('get_pdf_preview_status', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getPdfPagePreview: async (input: { projectId: string; documentId: string; pageNumber: number }): Promise<PdfPagePreview> => {
    try {
      return await invoke<PdfPagePreview>('get_pdf_page_preview', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  listPdfPagePreviews: async (input: { projectId: string; documentId: string }): Promise<PdfPagePreview[]> => {
    try {
      return await invoke<PdfPagePreview[]>('list_pdf_page_previews', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getPdfRendererStatus: async (): Promise<PdfRendererStatus> => {
    try {
      return await invoke<PdfRendererStatus>('get_pdf_renderer_status');
    } catch (e) {
      handleInvokeError(e);
    }
  },
  startStudentScanPreviewRender: async (input: { projectId: string; documentId: string }): Promise<StartPdfPreviewRenderOutput> => {
    try {
      return await invoke<StartPdfPreviewRenderOutput>('start_student_scan_preview_render', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getStudentScanPreviewStatus: async (input: { projectId: string; documentId: string }): Promise<PdfPreviewStatusSnapshot> => {
    try {
      return await invoke<PdfPreviewStatusSnapshot>('get_student_scan_preview_status', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  createStudentPageGroups: async (input: CreateStudentPageGroupsInput): Promise<CreateStudentPageGroupsOutput> => {
    try {
      return await invoke<CreateStudentPageGroupsOutput>('create_student_page_groups', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  listStudentSubmissions: async (projectId: string): Promise<StudentSubmission[]> => {
    try {
      return await invoke<StudentSubmission[]>('list_student_submissions', { input: { projectId } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  updateStudentIdentity: async (input: UpdateStudentIdentityInput): Promise<StudentSubmission> => {
    try {
      return await invoke<StudentSubmission>('update_student_identity', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  updateSubmissionPages: async (input: UpdateSubmissionPagesInput): Promise<StudentSubmission> => {
    try {
      return await invoke<StudentSubmission>('update_submission_pages', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  deleteStudentSubmission: async (input: DeleteStudentSubmissionInput) => {
    try {
      return await invoke<void>('delete_student_submission', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  markStudentGroupingComplete: async (input: MarkStudentGroupingCompleteInput): Promise<ProjectSnapshot> => {
    try {
      return await invoke<ProjectSnapshot>('mark_student_grouping_complete', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getOcrReadiness: async (projectId: string, batchId?: string): Promise<StudentScanReadinessSnapshot> => {
    try {
      return await invoke<StudentScanReadinessSnapshot>('get_ocr_readiness', { input: { projectId, batchId } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  startQuestionTextExtraction: async (input: { projectId: string; documentId?: string }) => {
    try {
      return await invoke<JobSnapshot>('start_question_text_extraction', { input: { ...input, source: 'exam_pdf' as const } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  startQuestionTextVisionFallback: async (input: { projectId: string }) => {
    try {
      return await invoke<JobSnapshot>('start_question_text_vision_fallback', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getQuestionTextExtractionStatus: async (input: { projectId: string }): Promise<QuestionTextExtractionStatus> => {
    try {
      return await invoke<QuestionTextExtractionStatus>('get_question_text_extraction_status', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  listQuestionTextSuggestions: async (input: { projectId: string }): Promise<QuestionTextSuggestion[]> => {
    try {
      return await invoke<QuestionTextSuggestion[]>('list_question_text_suggestions', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  confirmQuestionText: async (input: { projectId: string; questionId: string }): Promise<Question> => {
    try {
      return await invoke<Question>('confirm_question_text', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  confirmAllQuestionTexts: async (input: { projectId: string }): Promise<ProjectSnapshot> => {
    try {
      return await invoke<ProjectSnapshot>('confirm_all_question_texts', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  editQuestionText: async (input: { projectId: string; questionId: string; text: string }): Promise<Question> => {
    try {
      return await invoke<Question>('edit_question_text', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  startStudentAnswerOcr: async (input: { projectId: string; forceRerun?: boolean }): Promise<StartStudentAnswerOcrOutput> => {
    try {
      return await invoke<StartStudentAnswerOcrOutput>('start_student_answer_ocr', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  acceptStudentAnswerOcrGeneration: async (input: { projectId: string; generationId: string }): Promise<import('./types').OcrGeneration> => {
    try {
      return await invoke<import('./types').OcrGeneration>('accept_student_answer_ocr_generation', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  rejectStudentAnswerOcrGeneration: async (input: { projectId: string; generationId: string }): Promise<import('./types').OcrGeneration> => {
    try {
      return await invoke<import('./types').OcrGeneration>('reject_student_answer_ocr_generation', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  startStudentIdentityOcr: async (input: { projectId: string }): Promise<StartStudentIdentityOcrOutput> => {
    try {
      return await invoke<StartStudentIdentityOcrOutput>('start_student_identity_ocr', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  rebuildStudentAnswerOcrIssues: async (input: { projectId: string }): Promise<RebuildStudentAnswerOcrIssuesOutput> => {
    try {
      return await invoke<RebuildStudentAnswerOcrIssuesOutput>('rebuild_student_answer_ocr_issues', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  suggestOcrIssueCorrectionWithModel: async (
    input: SuggestStudentAnswerOcrIssueCorrectionWithModelInput,
  ): Promise<SuggestStudentAnswerOcrIssueCorrectionWithModelOutput> => {
    try {
      return await invoke<SuggestStudentAnswerOcrIssueCorrectionWithModelOutput>('suggest_ocr_issue_correction_with_model', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  updateStudentAnswerOcrText: async (input: { projectId: string; submissionId: string; questionId: string; text: string }): Promise<StudentAnswerOcrRecord> => {
    try {
      return await invoke<StudentAnswerOcrRecord>('update_student_answer_ocr_text', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  markStudentAnswerOcrReviewed: async (input: { projectId: string; submissionId: string; questionId: string }): Promise<StudentAnswerOcrRecord> => {
    try {
      return await invoke<StudentAnswerOcrRecord>('mark_student_answer_ocr_reviewed', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  markAllStudentAnswerOcrReviewed: async (input: { projectId: string }): Promise<ProjectSnapshot> => {
    try {
      return await invoke<ProjectSnapshot>('mark_all_student_answer_ocr_reviewed', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  saveStudentAnswerCropTemplate: async (input: { projectId: string; items: StudentAnswerCropTemplateItem[] }): Promise<ProjectSnapshot> => {
    try {
      return await invoke<ProjectSnapshot>('save_student_answer_crop_template', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  saveStudentIdentityCropTemplate: async (input: { projectId: string; template: StudentIdentityCropTemplate }): Promise<ProjectSnapshot> => {
    try {
      return await invoke<ProjectSnapshot>('save_student_identity_crop_template', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  preprocessOcrImage: async (input: PreprocessOcrImageInput): Promise<OcrImagePreprocessResult> => {
    try {
      return await invoke<OcrImagePreprocessResult>('preprocess_ocr_image', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  importRubricJson: async (input: ImportRubricJsonInput): Promise<ImportRubricJsonOutput> => {
    try {
      return await invoke<ImportRubricJsonOutput>('import_rubric_json', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getRubricState: async (projectId: string): Promise<RubricStateSnapshot> => {
    try {
      return await invoke<RubricStateSnapshot>('get_rubric_state', { input: { projectId } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  listRubricItems: async (projectId: string): Promise<RubricQuestionSnapshot[]> => {
    try {
      return await invoke<RubricQuestionSnapshot[]>('list_rubric_items', { input: { projectId } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  updateQuestionRubric: async (input: UpdateQuestionRubricInput): Promise<Question> => {
    try {
      return await invoke<Question>('update_question_rubric', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  confirmQuestionRubric: async (input: { projectId: string; questionId: string }): Promise<Question> => {
    try {
      return await invoke<Question>('confirm_question_rubric', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  confirmAllRubrics: async (input: { projectId: string }): Promise<ProjectSnapshot> => {
    try {
      return await invoke<ProjectSnapshot>('confirm_all_rubrics', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  validateRubrics: async (input: { projectId: string }): Promise<RubricValidationReport> => {
    try {
      return await invoke<RubricValidationReport>('validate_rubrics', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  startRubricPdfImport: async (input: StartRubricPdfImportInput): Promise<{ jobId: string; status: string }> => {
    try {
      return await invoke<{ jobId: string; status: string }>('start_rubric_pdf_import', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  startExamPackageBuild: async (input: StartExamPackageBuildInput): Promise<StartExamPackageBuildOutput> => {
    try {
      return await invoke<StartExamPackageBuildOutput>('start_exam_package_build', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  startScoringJob: async (input: { projectId: string; forceRerun?: boolean }): Promise<StartScoringOutput> => {
    try {
      return await invoke<StartScoringOutput>('start_scoring_job', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  updateScoringRecord: async (input: {
    projectId: string;
    recordId: string;
    teacherManualScore?: number | null;
    teacherNotes?: string | null;
    teacherApproved?: boolean;
  }): Promise<ScoringRecord> => {
    try {
      return await invoke<ScoringRecord>('update_scoring_record', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getGradedExamReview: async (input: { projectId: string; submissionId: string }): Promise<GradedExamReview> => {
    try {
      return await invoke<GradedExamReview>('get_graded_exam_review', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getJobSnapshot: async (jobId: string): Promise<JobSnapshot> => {
    try {
      return await invoke<JobSnapshot>('get_job_snapshot', { input: { jobId } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  listJobs: async (projectId: string, projectRootPath?: string): Promise<JobSnapshot[]> => {
    try {
      return await invoke<JobSnapshot[]>('list_jobs', { input: { projectId, projectRootPath } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  cancelJob: async (jobId: string): Promise<JobSnapshot> => {
    try {
      return await invoke<JobSnapshot>('cancel_job', { input: { jobId } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  retryJob: async (jobId: string): Promise<JobSnapshot> => {
    try {
      return await invoke<JobSnapshot>('retry_job', { input: { jobId } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  cleanupJobHistory: async (projectRootPath: string, maxTerminalJobs?: number): Promise<unknown> => {
    try {
      return await invoke('cleanup_job_history', { input: { projectRootPath, maxTerminalJobs } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  getModelStatus: async (): Promise<ModelStatus> => {
    try {
      return await invoke<ModelStatus>('get_model_status');
    } catch (e) {
      handleInvokeError(e);
    }
  },
  probeModelServer: async (): Promise<ModelStatus> => {
    try {
      return await invoke<ModelStatus>('probe_model_server');
    } catch (e) {
      handleInvokeError(e);
    }
  },
  startModelServer: async (profileId?: string): Promise<StartModelServerOutput> => {
    try {
      return await invoke<StartModelServerOutput>('start_model_server', {
        input: { profileId },
      });
    } catch (e) {
      handleInvokeError(e);
    }
  },

  stopModelServer: async (profileId?: string): Promise<StopModelServerOutput> => {
    try {
      return await invoke<StopModelServerOutput>('stop_model_server', {
        input: { profileId },
      });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  setModelMode: async (input: { profileId?: string; mode: 'external' | 'managed' }): Promise<ModelStatus> => {
    try {
      return await invoke<ModelStatus>('set_model_mode', {
        input: { profileId: input.profileId, mode: input.mode },
      });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  resetModelProfile: async (): Promise<ModelStatus> => {
    try {
      return await invoke<ModelStatus>('reset_model_profile');
    } catch (e) {
      handleInvokeError(e);
    }
  },
  previewModelServerArgs: async (profileId?: string): Promise<ModelServerArgsPreview> => {
    try {
      return await invoke<ModelServerArgsPreview>('preview_model_server_args', {
        input: { profileId },
      });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  runGenerationGc: async (projectId: string, dryRun = false): Promise<GcReport> => {
    try {
      return await invoke<GcReport>('run_generation_gc', { input: { projectId, dryRun } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  startBackupJob: async (projectId: string): Promise<StartBackupJobOutput> => {
    try {
      return await invoke<StartBackupJobOutput>('start_backup_job', { input: { projectId } });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  startRestoreJob: async (archivePath: string, destinationPath: string): Promise<StartRestoreJobOutput> => {
    try {
      return await invoke<StartRestoreJobOutput>('start_restore_job', {
        input: { archivePath, destinationPath },
      });
    } catch (e) {
      handleInvokeError(e);
    }
  },
  startRecoveryCopyJob: async (input: {
    sourceProjectPath: string;
    backupPath: string;
    destinationPath: string;
    dryRun?: boolean;
  }): Promise<StartRecoveryCopyJobOutput> => {
    try {
      return await invoke<StartRecoveryCopyJobOutput>('start_recovery_copy_job', { input });
    } catch (e) {
      handleInvokeError(e);
    }
  },
};

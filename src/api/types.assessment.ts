import type { SpeakingAttempt, SpeakingConfigurationSnapshot } from './types.speaking';

export type AssessmentType = 'written' | 'listening' | 'speaking' | 'legacy_performance';
export type WorkflowFamily = 'written' | 'speaking' | 'legacy_performance';
export type AssessmentStatus = 'draft' | 'scheduled' | 'active' | 'completed' | 'archived';
export type ClassApplicationStatus = 'scheduled' | 'active' | 'completed' | 'archived';

export type ListeningDetails = {
  audioDocumentId?: string | null;
  transcriptDocumentId?: string | null;
  playCount?: number | null;
  durationSeconds?: number | null;
  instruction?: string | null;
};

export type ClassApplication = {
  id: string;
  activityId: string;
  schoolClassId: string;
  scheduledAt?: string | null;
  applicationDate?: string | null;
  status: ClassApplicationStatus;
  notes?: string | null;
  documentIds: string[];
  studentScopeIds: string[];
  speakingAttempts: SpeakingAttempt[];
  createdAt: string;
  updatedAt: string;
};

export type AssessmentClassApplication = ClassApplication;

export type AssessmentActivity = {
  id: string;
  academicYearId: string;
  courseId: string;
  courseName: string;
  title: string;
  gradeLevel: number;
  term: number;
  assessmentType: AssessmentType;
  workflowFamily: WorkflowFamily;
  sequenceNumber: number;
  status: AssessmentStatus;
  commonDocumentIds: string[];
  listeningDetails?: ListeningDetails | null;
  speakingConfiguration?: SpeakingConfigurationSnapshot | null;
  classApplications: AssessmentClassApplication[];
  createdAt: string;
  updatedAt: string;
};

export type AssessmentSequenceOptions = {
  options: number[];
  suggested: number;
};

export type UpdateCourseInfoInput = {
  projectId: string;
  academicYearId: string;
  courseId: string;
  courseName: string;
  expectedRevision?: number;
};

export type BatchCreateTeachingAssignmentsInput = {
  projectId: string;
  academicYearId: string;
  courseId: string;
  courseName: string;
  classSectionIds: string[];
};

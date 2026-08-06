import type { Document } from './types.document';
import type { PageGroupingMode } from './types.student';

export type SchoolClassStatus = 'active' | 'archived';

export type SchoolClass = {
  id: string;
  name: string;
  displayName?: string;
  normalizedName: string;
  academicYear?: string | null;
  academicYearId?: string | null;
  gradeLevel?: number | null;
  section?: string | null;
  displayOrder: number;
  status: SchoolClassStatus;
  createdAt: string;
  updatedAt: string;
};

export type TeachingAssignment = {
  id: string;
  academicYearId: string;
  courseId: string;
  courseName: string;
  classSectionId: string;
  teacherId?: string | null;
  isActive: boolean;
  createdAt: string;
  updatedAt: string;
};

export type StudentScanBatch = {
  id: string;
  classId: string;
  documentId: string;
  originalFileName: string;
  displayName: string;
  pagesPerStudent?: number | null;
  groupingMode?: PageGroupingMode | null;
  groupingCompletedAt?: string | null;
  createdAt: string;
  updatedAt: string;
};

export type SchoolClassOverview = {
  schoolClass: SchoolClass;
  scanBatchCount: number;
  submissionCount: number;
  identityVerifiedCount: number;
  ocrCompleteCount: number;
  scoringCompleteCount: number;
  reviewRequiredCount: number;
};

export type SchoolClassOverviewSnapshot = {
  classes: SchoolClassOverview[];
  unassignedBatchCount: number;
  unassignedSubmissionCount: number;
};

export type ImportStudentScanBatchInput = {
  projectId: string;
  classId: string;
  sourcePath: string;
  displayName?: string;
  pagesPerStudent?: number;
  groupingMode?: PageGroupingMode;
};

export type ImportStudentScanBatchOutput = {
  document: Document;
  batch: StudentScanBatch;
};

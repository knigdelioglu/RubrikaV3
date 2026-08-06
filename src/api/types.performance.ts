export type PerformanceSkillArea = 'reading' | 'listening_watching' | 'speaking' | 'writing';

export type PerformanceWorkMode = 'individual' | 'group';

export type PerformanceAssessmentStatus = 'in_progress' | 'approved' | 'not_performed' | 'missing';

export type PerformanceLevel = {
  id: string;
  name: string;
  points: number;
  description: string;
};

export type LevelDescription = {
  levelId: string;
  description: string;
};

export type PerformanceCriterion = {
  id: string;
  name: string;
  description: string;
  levelDescriptions: LevelDescription[];
};

export type PerformanceRubric = {
  id: string;
  name: string;
  version: number;
  criteria: PerformanceCriterion[];
  levels: PerformanceLevel[];
  createdAt: string;
};

export type PerformanceDetails = {
  theme: string;
  learningOutcomes: string[];
  skillArea: PerformanceSkillArea;
  taskInstruction: string;
  workMode: PerformanceWorkMode;
  dueDate?: string | null;
  evidenceTypes: string[];
  rubricVersions?: PerformanceRubric[];
};

export type CriterionRating = {
  criterionId: string;
  levelId: string;
  note?: string | null;
};

export type PerformanceAssessment = {
  id: string;
  studentId: string;
  rubricId: string;
  rubricVersion: number;
  ratings: CriterionRating[];
  provisionalTotal: number;
  feedback?: string | null;
  status: PerformanceAssessmentStatus;
  assessedAt?: string | null;
  approvedAt?: string | null;
  createdAt: string;
  updatedAt: string;
};

export type PerformanceReportCriterionScore = {
  criterionId: string;
  criterionName: string;
  levelId?: string | null;
  levelName?: string | null;
  points?: number | null;
};

export type PerformanceReportStudentRow = {
  studentId: string;
  studentName: string;
  studentNumber?: string | null;
  status?: PerformanceAssessmentStatus | null;
  criterionScores: PerformanceReportCriterionScore[];
  total?: number | null;
  provisionalTotal?: number | null;
  feedback?: string | null;
  assessedAt?: string | null;
  approvedAt?: string | null;
};

export type PerformanceReportSummary = {
  studentCount: number;
  assessedCount: number;
  approvedCount: number;
  missingCount: number;
  notPerformedCount: number;
  unratedCount: number;
};

export type PerformanceReport = {
  taskTitle: string;
  courseName: string;
  gradeLevel: number;
  term: number;
  sequenceNumber: number;
  theme?: string | null;
  skillArea?: PerformanceSkillArea | null;
  workMode?: PerformanceWorkMode | null;
  className: string;
  teacherId?: string | null;
  rubricId: string;
  rubricName: string;
  rubricVersion: number;
  criteria: PerformanceCriterion[];
  levels: PerformanceLevel[];
  maxPoints: number;
  generatedAt: string;
  summary: PerformanceReportSummary;
  rows: PerformanceReportStudentRow[];
};

export type PerformanceStatus = {
  hasPublishedRubric: boolean;
  publishedRubricVersion?: number | null;
  hasDraftRubric: boolean;
  hasTaskDetails: boolean;
  totalStudents: number;
  approvedCount: number;
  inProgressCount: number;
  missingCount: number;
  notPerformedCount: number;
  allApproved: boolean;
};

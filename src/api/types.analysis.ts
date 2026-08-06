export type AssessmentKind = 'written' | 'speaking';
export type AnalysisStatus = 'generating' | 'ready' | 'partial' | 'failed';
export type AnalysisMetricUnit = 'count' | 'score' | 'percentage';
export type AnalysisEvidenceStatus = 'supported' | 'review' | 'unsupported';

export type AnalysisCriterionSummary = {
  id: string;
  label: string;
  averageScore: number;
  maxScore: number;
  percentage: number;
  sampleCount: number;
};

export type AnalysisStudentSummary = {
  studentId: string;
  displayName: string;
  score: number;
  maxScore: number;
  percentage: number;
};

export type AnalysisScoreBand = {
  label: string;
  minimum: number;
  maximum: number;
  count: number;
};

export type AnalysisMetric = {
  id: string;
  label: string;
  value: number;
  unit: AnalysisMetricUnit;
  description: string;
};

export type AnalysisMetricRef = {
  metricId: string;
  label: string;
  value: number;
  unit: AnalysisMetricUnit;
};

export type AnalysisClaim = {
  id: string;
  claim: string;
  metricRefs: AnalysisMetricRef[];
  recommendation: string;
  evidenceStatus: AnalysisEvidenceStatus;
  teacherVisibleExplanation: string;
};

export type AssessmentAnalysis = {
  id: string;
  projectId: string;
  kind: AssessmentKind;
  sourceId?: string | null;
  title: string;
  classId?: string | null;
  status: AnalysisStatus;
  studentCount: number;
  criteria: AnalysisCriterionSummary[];
  students: AnalysisStudentSummary[];
  scoreBands: AnalysisScoreBand[];
  metrics: AnalysisMetric[];
  claims: AnalysisClaim[];
  modelReport?: string | null;
  modelReportError?: string | null;
  createdAt: string;
  completedAt?: string | null;
};

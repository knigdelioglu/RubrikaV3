export type GradedExamAnnotationStatus = 'model_score' | 'needs_review';

export type GradedExamPlacement = 'right_of_answer' | 'inside_top_right';

export type GradedExamScorePart = {
  title: string;
  awardedScore: number;
  maxScore: number;
};

export type GradedExamAnnotation = {
  recordId: string;
  questionId: string;
  questionNumber: number;
  modelScore?: number | null;
  maxScore: number;
  label: string;
  x: number;
  y: number;
  width: number;
  height: number;
  placement: GradedExamPlacement;
  status: GradedExamAnnotationStatus;
  needsReview: boolean;
  scoreParts: GradedExamScorePart[];
  reviewGuidance: string[];
};

export type GradedExamUnplacedScore = {
  recordId: string;
  questionId: string;
  questionNumber: number;
  modelScore?: number | null;
  maxScore: number;
  reason: string;
};

export type GradedExamPage = {
  pageNumber: number;
  imagePath: string;
  width: number;
  height: number;
  annotations: GradedExamAnnotation[];
};

export type GradedExamReview = {
  projectId: string;
  submissionId: string;
  documentId: string;
  studentDisplayName: string;
  studentNumber?: string | null;
  studentClassName?: string | null;
  scoringRunId?: string | null;
  modelTotalScore?: number | null;
  maxTotalScore: number;
  needsReviewCount: number;
  pages: GradedExamPage[];
  unplacedScores: GradedExamUnplacedScore[];
};

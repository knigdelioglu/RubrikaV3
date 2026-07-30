import type { ProjectSnapshot, ScoringRecord, ScoringReviewStatus, StudentSubmission } from '../api/types';
import { scoringReviewStatusLabels } from '../utils/labels.ts';
import { getSubmissionClassName } from './studentOperations.ts';

export function compareScoringRecords(left: ScoringRecord, right: ScoringRecord): number {
  if (left.updatedAt !== right.updatedAt) return left.updatedAt.localeCompare(right.updatedAt);
  if (left.createdAt !== right.createdAt) return left.createdAt.localeCompare(right.createdAt);
  if (left.runId !== right.runId) return left.runId.localeCompare(right.runId);
  return left.id.localeCompare(right.id);
}

export function dedupeScoringRecords(records: ScoringRecord[]): ScoringRecord[] {
  const grouped = new Map<string, ScoringRecord>();
  for (const record of [...records].sort(compareScoringRecords)) {
    grouped.set(`${record.submissionId}::${record.questionId}`, record);
  }
  return [...grouped.values()].sort((left, right) => {
    if (left.submissionId !== right.submissionId) return left.submissionId.localeCompare(right.submissionId);
    return left.questionNumber - right.questionNumber;
  });
}

export function resolveActiveScoringRunId(project?: { latestScoringRunId?: string | null; scoringRecords?: ScoringRecord[] } | null): string | null {
  if (!project) return null;
  const explicit = project.latestScoringRunId?.trim();
  if (explicit) return explicit;

  const sorted = [...(project.scoringRecords ?? [])]
    .filter((record) => record.runId.trim().length > 0)
    .sort(compareScoringRecords);
  const latestRecord = sorted[sorted.length - 1];
  return latestRecord?.runId ?? null;
}

export function getReviewStatusLabel(status: ScoringReviewStatus): string {
  return scoringReviewStatusLabels[status] ?? 'Onay bekliyor';
}

export function getStudentDisplayValue(value: string | null | undefined, fallback: string): string {
  const trimmed = value?.trim();
  return trimmed && trimmed.length > 0 ? trimmed : fallback;
}

export function getStudentSummaryBadges(input: {
  hasRecords: boolean;
  needsReview: boolean;
  warningCount: number;
  approvedCount: number;
  pendingCount: number;
}): string[] {
  if (!input.hasRecords) return ['Henüz notlandırılmadı'];

  const badges: string[] = [];
  if (input.needsReview) badges.push('İnceleme gerekli');
  if (input.warningCount > 0) badges.push('Uyarı var');
  if (input.approvedCount > 0 && input.pendingCount === 0 && !input.needsReview && input.warningCount === 0) {
    badges.push('Onaylandı');
  } else {
    badges.push('Onay bekliyor');
  }

  return [...new Set(badges)];
}

export function getSubmissionSortKey(project: ProjectSnapshot, submission: StudentSubmission): string {
  const student = project.students.find((item) => item.id === submission.studentId);
  return [
    getSubmissionClassName(project, submission),
    getStudentDisplayValue(student?.number, ''),
    getStudentDisplayValue(student?.displayName, submission.studentId),
    submission.id,
  ]
    .join('::')
    .toLowerCase();
}

export function buildStudentSummary(project: ProjectSnapshot, submission: StudentSubmission, records: ScoringRecord[]) {
  const student = project.students.find((item) => item.id === submission.studentId);
  const maxScore = project.questions.reduce((sum, question) => sum + (question.rubric.maxScore ?? question.maxScore), 0);
  const scoredRecords = records.filter((record) => (
    record.scoringApplied && (record.teacherManualScore ?? record.awardedScore) !== null
  ));
  const expectedRecordCount = project.questions.length;
  const isComplete = expectedRecordCount > 0 && scoredRecords.length === expectedRecordCount;
  const totalScore = !isComplete
    ? null
    : Math.min(
      scoredRecords.reduce((sum, record) => {
        const score = record.teacherManualScore ?? record.awardedScore;
        return sum + Math.min(score ?? 0, record.maxScore);
      }, 0),
      maxScore,
    );
  const approvedCount = records.filter((record) => record.teacherReviewStatus === 'approved' || record.teacherReviewStatus === 'edited').length;
  const pendingCount = records.filter((record) => record.teacherReviewStatus === 'pending_review').length;
  const warningCount = records.reduce((sum, record) => sum + record.warnings.length, 0);
  const needsReview = records.some((record) => record.needsReview || record.warnings.length > 0);
  const badges = getStudentSummaryBadges({
    hasRecords: records.length > 0,
    needsReview,
    warningCount,
    approvedCount,
    pendingCount,
  });

  return {
    submission,
    student,
    records,
    totalScore,
    maxScore,
    approvedCount,
    pendingCount,
    warningCount,
    needsReview,
    scoredCount: scoredRecords.length,
    unscoredCount: Math.max(0, expectedRecordCount - scoredRecords.length),
    isComplete,
    duplicateCount: Math.max(0, project.scoringRecords.filter((record) => record.submissionId === submission.id).length - records.length),
    badges,
    reviewLabel: records.length === 0 ? 'Henüz notlandırılmadı' : badges[0] ?? 'Onay bekliyor',
  };
}

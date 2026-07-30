import type { GradedExamAnnotation, ProjectSnapshot } from '../../api/types.ts';
import { dedupeScoringRecords, resolveActiveScoringRunId } from '../../pages/scoringViewModel.ts';

export function formatReviewScore(value: number | null | undefined): string {
  if (value === null || value === undefined || Number.isNaN(value)) return '—';
  return value.toFixed(2).replace(/\.00$/, '').replace(/(\.\d)0$/, '$1');
}

export function annotationColors(annotation: Pick<GradedExamAnnotation, 'status' | 'needsReview'>) {
  if (annotation.status === 'needs_review') {
    return { background: '#fff7ed', border: '#f97316', color: '#9a3412' };
  }
  if (annotation.needsReview) {
    return { background: '#fffbeb', border: '#eab308', color: '#854d0e' };
  }
  return { background: '#fffafa', border: '#dc2626', color: '#991b1b' };
}

export function calculateFitScale(containerWidth: number, containerHeight: number, pageWidth: number, pageHeight: number, padding = 24): number {
  if (containerWidth <= padding || containerHeight <= padding || pageWidth <= 0 || pageHeight <= 0) return 1;
  return Math.min((containerWidth - padding) / pageWidth, (containerHeight - padding) / pageHeight);
}

export function calculatePreservedPageScale(referenceFitScale: number, referencePageWidth: number, currentPageWidth: number): number {
  if (referenceFitScale <= 0 || referencePageWidth <= 0 || currentPageWidth <= 0) return 1;
  return (referencePageWidth * referenceFitScale) / currentPageWidth;
}

export function getGradedExamReviewQueue(project: ProjectSnapshot): string[] {
  const activeRunId = resolveActiveScoringRunId(project);
  const activeRecords = dedupeScoringRecords(
    activeRunId
      ? project.scoringRecords.filter((record) => record.runId === activeRunId)
      : project.scoringRecords,
  );
  const reviewable = new Set(activeRecords.map((record) => record.submissionId));
  return [...project.studentSubmissions]
    .filter((submission) => reviewable.has(submission.id))
    .sort((left, right) => {
      const leftPage = left.pageNumbers[0] ?? Number.MAX_SAFE_INTEGER;
      const rightPage = right.pageNumbers[0] ?? Number.MAX_SAFE_INTEGER;
      return leftPage - rightPage || left.id.localeCompare(right.id);
    })
    .map((submission) => submission.id);
}

export function scoreBreakdown(annotation: Pick<GradedExamAnnotation, 'scoreParts'>): string {
  return annotation.scoreParts
    .map((part) => formatReviewScore(part.awardedScore))
    .join('+');
}

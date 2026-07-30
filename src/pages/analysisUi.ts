import type { AnalysisStatus, AssessmentAnalysis, AssessmentKind } from '../api/types';

export function clampAnalysisPercentage(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(100, value));
}

export function percentageLabel(value: number): string {
  return `%${Math.round(clampAnalysisPercentage(value))}`;
}

export function latestAnalysisId(
  analyses: AssessmentAnalysis[] | undefined,
  kind: AssessmentKind,
): string {
  return analyses?.find((analysis) => analysis.kind === kind)?.id ?? '';
}

export function analysisStatusLabel(status: AnalysisStatus): string {
  switch (status) {
    case 'generating':
      return 'Gemma raporu hazırlanıyor';
    case 'ready':
      return 'rapor hazır';
    case 'partial':
      return 'grafikler hazır, rapor kısmi';
    case 'failed':
      return 'analiz tamamlanamadı';
  }
}

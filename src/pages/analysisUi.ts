import type {
  AnalysisEvidenceStatus,
  AnalysisMetricUnit,
  AnalysisStatus,
  AssessmentAnalysis,
  AssessmentKind,
} from '../api/types';

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

export function analysisEvidenceStatusLabel(status: AnalysisEvidenceStatus): string {
  switch (status) {
    case 'supported':
      return 'Metrikle destekleniyor';
    case 'review':
      return 'Öğretmen incelemesi gerekli';
    case 'unsupported':
      return 'Desteklenmiyor';
  }
}

export function analysisMetricValueLabel(value: number, unit: AnalysisMetricUnit): string {
  if (!Number.isFinite(value)) return '—';
  switch (unit) {
    case 'count':
      return String(Math.round(value));
    case 'score':
      return value.toLocaleString('tr-TR', { maximumFractionDigits: 1 });
    case 'percentage':
      return percentageLabel(value);
  }
}

export function analysisMetricAnchorId(metricId: string): string {
  return 'analysis-metric-' + metricId.replace(/[^a-zA-Z0-9_-]/g, '-');
}

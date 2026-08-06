import type { PerformanceAssessmentStatus, PerformanceReport } from '../api/types';

export function performanceReportStatusLabel(
  status: PerformanceAssessmentStatus | null | undefined,
): string {
  switch (status) {
    case 'approved':
      return 'Onaylandı';
    case 'in_progress':
      return 'Taslak';
    case 'missing':
      return 'Eksik (teslim edilmedi)';
    case 'not_performed':
      return 'Gösterilmedi';
    default:
      return 'Değerlendirilmedi';
  }
}

export function performanceReportRowTotal(
  row: PerformanceReport['rows'][number],
): number | null {
  return row.total ?? null;
}

export function performanceReportCsvFileName(report: PerformanceReport, classSuffix: string): string {
  const safeTitle = report.taskTitle
    .trim()
    .replace(/[\\/:*?"<>|]/g, '-')
    .slice(0, 60) || 'performans';
  const safeClass = classSuffix.trim().replace(/[\\/:*?"<>|]/g, '-') || 'sinif';
  return `performans-sonuclari_${safeClass}_${safeTitle}.csv`;
}

// Türkçe Excel uyumu: noktalı virgül ayraçlı CSV + UTF-8 BOM. Hücreler
// `;`, `"` veya yeni satır içeriyorsa çift tırnakla sarılır (çift tırnaklar
// iki katlanır). Formül enjeksiyonu riskine karşı `=`, `+`, `-`, `@`, sekme
// ve satır başı ile başlayan hücreler tek tırnak önekiyle kaçışlanır.
export function buildPerformanceCsv(report: PerformanceReport): string {
  const escapeCell = (value: string): string => {
    if (/^[=+\-@\t\r]/.test(value)) {
      return `'${value}`;
    }
    if (/[";\n\r]/.test(value)) {
      return `"${value.replace(/"/g, '""')}"`;
    }
    return value;
  };

  const header = [
    'Öğrenci No',
    'Öğrenci',
    'Durum',
    ...report.criteria.map((criterion) => criterion.name),
    'Toplam',
    'Geri Bildirim',
  ]
    .map(escapeCell)
    .join(';');

  const rows = report.rows.map((row) => {
    const pointsByCriterion = new Map(
      row.criterionScores.map((score) => [score.criterionId, score.points]),
    );
    const criterionCells = report.criteria.map((criterion) => {
      const points = pointsByCriterion.get(criterion.id);
      return points != null ? String(points) : '';
    });
    const total = performanceReportRowTotal(row);
    const feedback = row.feedback?.trim() ?? '';
    return [
      row.studentNumber ?? '',
      row.studentName,
      performanceReportStatusLabel(row.status),
      ...criterionCells,
      total != null ? String(total) : '',
      feedback,
    ]
      .map(escapeCell)
      .join(';');
  });

  return `\uFEFF${[header, ...rows].join('\r\n')}\r\n`;
}

export function downloadTextFile(fileName: string, content: string, mime: string): void {
  const blob = new Blob([content], { type: mime });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = fileName;
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(url);
}

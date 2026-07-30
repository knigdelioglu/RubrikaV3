import type { WorkflowStep } from '../../api/types';

export type OverviewArea = 'exam' | 'students' | 'ocr' | 'grading';
export type OverviewStatus = WorkflowStep['status'];

export type OverviewAreaSummary = {
  area: OverviewArea;
  label: string;
  status: OverviewStatus;
  message: string;
  current?: number;
  total?: number;
};

const definitions: Array<{ area: OverviewArea; label: string; stepCodes: string[] }> = [
  { area: 'exam', label: 'Sınav Hazırlığı', stepCodes: ['pdf_preview_render', 'question_text_extraction', 'rubric_pdf_import'] },
  { area: 'students', label: 'Öğrenci Hazırlığı', stepCodes: ['student_scan_preview_render'] },
  { area: 'ocr', label: 'OCR ve Kontrol', stepCodes: ['student_answer_ocr'] },
  { area: 'grading', label: 'Notlandırma', stepCodes: ['scoring'] },
];

const statusPriority: OverviewStatus[] = ['failed', 'running', 'partial', 'pending', 'succeeded'];

export function summarizeWorkflowAreas(steps: WorkflowStep[]): OverviewAreaSummary[] {
  return definitions.map((definition) => {
    const matching = steps.filter((step) => definition.stepCodes.includes(step.code));
    if (matching.length === 0) {
      return { area: definition.area, label: definition.label, status: 'pending', message: 'Henüz başlanmadı' };
    }

    const status = statusPriority.find((candidate) => matching.some((step) => step.status === candidate)) ?? 'pending';
    const representative = matching.find((step) => step.status === status) ?? matching[0]!;
    const totals = matching.filter((step) => step.total !== undefined && step.current !== undefined);

    return {
      area: definition.area,
      label: definition.label,
      status,
      message: representative.message,
      current: totals.length === 1 ? totals[0]?.current : undefined,
      total: totals.length === 1 ? totals[0]?.total : undefined,
    };
  });
}


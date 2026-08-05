import type { AssessmentActivity, AssessmentType, SpeakingAttempt } from '../api/types';

export const assessmentTypeLabels: Record<AssessmentType, string> = {
  written: 'Yazılı Sınav',
  listening: 'Dinleme Sınavı',
  speaking: 'Konuşma Sınavı',
  performance: 'Performans Görevi',
};

export function recommendedAssessmentSlots(type: AssessmentType): number[] {
  return type === 'listening' ? [1] : [1, 2];
}

export function assessmentSequenceOptions(
  activities: AssessmentActivity[],
  courseId: string,
  term: number,
  type: AssessmentType,
  editingActivityId?: string | null,
): number[] {
  const used = new Set(
    activities
      .filter((activity) => activity.id !== editingActivityId)
      .filter((activity) => activity.courseId === courseId && activity.term === term && activity.assessmentType === type)
      .map((activity) => activity.sequenceNumber),
  );
  const options = recommendedAssessmentSlots(type).filter((value) => !used.has(value));
  if (options.length > 0) return options;
  const next = Math.max(0, ...used) + 1;
  return [next];
}

export function formatDurationRange(minSeconds: number, maxSeconds: number): string {
  if (!Number.isFinite(minSeconds) || !Number.isFinite(maxSeconds) || minSeconds <= 0 || maxSeconds <= 0) return 'Süre belirtilmedi';
  return `${Math.round(minSeconds / 60)}–${Math.round(maxSeconds / 60)} dakika`;
}

export function workflowFamilyLabel(type: AssessmentType): string {
  switch (type) {
    case 'speaking':
      return 'konuşma';
    case 'performance':
      return 'performans';
    default:
      return 'yazılı';
  }
}

export function canonicalClassApplicationIds(activity: AssessmentActivity): string[] {
  return activity.classApplications
    .filter((application) => application.status !== 'archived')
    .map((application) => application.id);
}

export function speakingAttemptsForApplication(
  attempts: SpeakingAttempt[],
  classApplicationId: string,
): SpeakingAttempt[] {
  return attempts.filter((attempt) => attempt.classApplicationId === classApplicationId);
}

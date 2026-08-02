import type { AssessmentActivity, AssessmentType } from '../api/types';
import { getCanonicalWorkspaceStepPath, getExamStepDefinitions } from './examWorkspace.ts';

const compactAssessmentTypeLabels: Record<AssessmentType, string> = {
  written: 'Yazılı',
  listening: 'Dinleme',
  speaking: 'Konuşma',
};

type ProjectSwitcherActivity = Pick<AssessmentActivity, 'id' | 'term' | 'sequenceNumber' | 'assessmentType'>;

export function formatAssessmentContext(
  activity: Pick<AssessmentActivity, 'term' | 'sequenceNumber'>,
): string {
  return `${activity.term}. Dönem · ${activity.sequenceNumber}. Sınav`;
}

export function formatAssessmentOption(
  activity: ProjectSwitcherActivity,
): string {
  return `${formatAssessmentContext(activity)} · ${compactAssessmentTypeLabels[activity.assessmentType]}`;
}

export function getProjectSwitcherContextLabel(
  activities: readonly ProjectSwitcherActivity[],
  activeActivityId: string,
  loading: boolean,
): string {
  const activeActivity = activities.find((activity) => activity.id === activeActivityId);
  if (activeActivity) return formatAssessmentContext(activeActivity);
  if (loading) return 'Sınavlar yükleniyor…';
  if (activities.length === 1) return formatAssessmentContext(activities[0]!);
  if (activities.length > 1) return `${activities.length} sınav · seçim yap`;
  return 'Sınav seçilmedi';
}

export function getAssessmentActivityIdFromLocation(pathname: string, rawSearch = ''): string {
  const encodedActivityId = pathname.match(/^\/project\/[^/]+\/activities\/([^/]+)(?:\/|$)/)?.[1];
  if (encodedActivityId) {
    try {
      return decodeURIComponent(encodedActivityId);
    } catch {
      return '';
    }
  }

  return new URLSearchParams(rawSearch).get('assessmentActivityId')?.trim() ?? '';
}

export function projectActivityPath(
  projectId: string,
  activity: Pick<AssessmentActivity, 'id' | 'assessmentType'>,
): string {
  const firstStep = getExamStepDefinitions(activity.assessmentType)[0]?.id ?? 'prep';
  return getCanonicalWorkspaceStepPath(projectId, activity.id, firstStep);
}

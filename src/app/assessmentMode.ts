export type AssessmentMode = 'written' | 'listening' | 'speaking' | 'performance';

export function getAssessmentMode(pathname: string, search = ''): AssessmentMode {
  if (/^\/project\/[^/]+\/speaking(?:\/|$)/.test(pathname)) return 'speaking';
  if (/^\/project\/[^/]+\/performance(?:\/|$)/.test(pathname)) return 'performance';
  if (
    /^\/project\/[^/]+\/activities(?:\/|$)/.test(pathname) &&
    new URLSearchParams(search).get('assessmentType') === 'listening'
  ) {
    return 'listening';
  }
  if (
    /^\/project\/[^/]+\/activities(?:\/|$)/.test(pathname) &&
    new URLSearchParams(search).get('assessmentType') === 'performance'
  ) {
    return 'performance';
  }
  return 'written';
}

export function getAssessmentModePath(mode: AssessmentMode, projectId: string): string {
  if (mode === 'speaking' && projectId) {
    return `/project/${encodeURIComponent(projectId)}/speaking`;
  }
  if (mode === 'listening' && projectId) {
    return `/project/${encodeURIComponent(projectId)}/activities?assessmentType=listening`;
  }
  if (mode === 'performance' && projectId) {
    return `/project/${encodeURIComponent(projectId)}/performance`;
  }
  return projectId ? `/project/${encodeURIComponent(projectId)}/overview` : '/projects';
}

export function shouldShowProjectNavigation(_pathname: string): boolean {
  return true;
}

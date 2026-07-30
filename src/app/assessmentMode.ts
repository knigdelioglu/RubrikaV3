export type AssessmentMode = 'written' | 'speaking';

export function getAssessmentMode(pathname: string): AssessmentMode {
  return /^\/project\/[^/]+\/speaking(?:\/|$)/.test(pathname) ? 'speaking' : 'written';
}

export function getAssessmentModePath(mode: AssessmentMode, projectId: string): string {
  if (mode === 'speaking' && projectId) {
    return `/project/${encodeURIComponent(projectId)}/speaking`;
  }
  return projectId ? `/project/${encodeURIComponent(projectId)}/overview` : '/projects';
}

export function shouldShowProjectNavigation(pathname: string): boolean {
  return getAssessmentMode(pathname) === 'written';
}

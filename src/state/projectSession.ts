const LAST_PROJECT_ID_KEY = 'lastProjectId';
const LAST_PROJECT_PATH_KEY = 'lastProjectPath';

function readStorage(key: string): string {
  if (typeof window === 'undefined') return '';
  return window.localStorage.getItem(key) || '';
}

function writeStorage(key: string, value: string) {
  if (typeof window === 'undefined') return;
  if (!value) {
    window.localStorage.removeItem(key);
    return;
  }
  window.localStorage.setItem(key, value);
}

export function getLastProjectId(): string {
  return readStorage(LAST_PROJECT_ID_KEY);
}

export function getLastProjectPath(): string {
  return readStorage(LAST_PROJECT_PATH_KEY);
}

export function setActiveProject(projectId: string, projectPath: string) {
  writeStorage(LAST_PROJECT_ID_KEY, projectId);
  writeStorage(LAST_PROJECT_PATH_KEY, projectPath);
}

export function clearActiveProject() {
  writeStorage(LAST_PROJECT_ID_KEY, '');
  writeStorage(LAST_PROJECT_PATH_KEY, '');
}

export function resolveActiveProjectId(projectId: string | null | undefined): string {
  return projectId?.trim() || getLastProjectId();
}

export function resolveProjectIdFromProjects(
  projects: Array<{ id: string; path: string }>,
  projectPath: string,
): string {
  const normalizedPath = projectPath.trim();
  if (!normalizedPath) return '';

  return projects.find((project) => project.path === normalizedPath)?.id || '';
}

export function selectStartupProject<T extends { id: string; path: string }>(
  projects: T[],
  lastProjectId: string,
  lastProjectPath: string,
): T | undefined {
  return (
    projects.find((project) => project.id === lastProjectId) ??
    projects.find((project) => project.path === lastProjectPath) ??
    projects[0]
  );
}

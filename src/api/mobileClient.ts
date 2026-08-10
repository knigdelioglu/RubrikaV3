import type { ListProjectsOutput, ProjectSnapshot } from './types';

export type MobileHealth = {
  appVersion: string;
  platform: string;
  tauriReady: boolean;
  rustBackendReady: boolean;
  mobileApiReady: boolean;
};

const baseUrlKey = 'rubrika.mobileApiBaseUrl';
const tokenKey = 'rubrika.mobileApiToken';

export function getMobileConnection(): { baseUrl: string; token: string } {
  const host = typeof window !== 'undefined' && window.location.hostname && window.location.hostname !== 'localhost'
    ? window.location.hostname
    : '127.0.0.1';
  return {
    baseUrl: localStorage.getItem(baseUrlKey) ?? `http://${host}:8787`,
    token: localStorage.getItem(tokenKey) ?? '',
  };
}

export function saveMobileConnection(baseUrl: string, token: string): void {
  localStorage.setItem(baseUrlKey, baseUrl.trim().replace(/\/$/, ''));
  localStorage.setItem(tokenKey, token.trim());
}

async function request<T>(path: string): Promise<T> {
  const { baseUrl, token } = getMobileConnection();
  const response = await fetch(`${baseUrl}${path}`, {
    headers: token ? { 'X-Rubrika-Token': token } : undefined,
  });
  if (!response.ok) {
    const body = await response.text();
    throw new Error(body || `Mobil API hatası (${response.status})`);
  }
  return (await response.json()) as T;
}

export const mobileClient = {
  health: () => request<MobileHealth>('/api/mobile/health'),
  listProjects: () => request<ListProjectsOutput>('/api/mobile/projects'),
  getProject: (projectId: string) => request<ProjectSnapshot>(`/api/mobile/projects/${encodeURIComponent(projectId)}`),
};

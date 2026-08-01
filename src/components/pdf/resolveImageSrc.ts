import { convertFileSrc } from '@tauri-apps/api/core';

export function resolveImageSrc(imagePath: string, projectId?: string) {
  if (
    imagePath.startsWith('asset://')
    || imagePath.startsWith('http://')
    || imagePath.startsWith('https://')
    || imagePath.startsWith('data:')
    || imagePath.startsWith('managed-asset://')
  ) {
    return imagePath;
  }
  if (projectId) {
    const segments = imagePath.split('/').map((segment) => encodeURIComponent(segment)).join('/');
    return `managed-asset://localhost/${encodeURIComponent(projectId)}/${segments}`;
  }
  return convertFileSrc(imagePath);
}

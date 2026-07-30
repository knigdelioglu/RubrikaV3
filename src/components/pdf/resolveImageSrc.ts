import { convertFileSrc } from '@tauri-apps/api/core';

export function resolveImageSrc(imagePath: string) {
  if (imagePath.startsWith('asset://') || imagePath.startsWith('http://') || imagePath.startsWith('https://')) {
    return imagePath;
  }
  return convertFileSrc(imagePath);
}

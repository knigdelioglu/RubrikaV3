import { readFile, stat } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repositoryRoot = fileURLToPath(new URL('../', import.meta.url));
const sourceIconPath = path.join(repositoryRoot, 'icon.png');
const tauriRoot = path.join(repositoryRoot, 'src-tauri');
const generatedIconRoot = path.join(tauriRoot, 'icons');
const tauriConfigPath = path.join(tauriRoot, 'tauri.conf.json');

const pngSignature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

const requiredGeneratedFiles = [
  '32x32.png',
  '64x64.png',
  '128x128.png',
  '128x128@2x.png',
  'icon.png',
  'icon.icns',
  'icon.ico',
  'StoreLogo.png',
  'Square310x310Logo.png',
  'ios/AppIcon-512@2x.png',
  'android/mipmap-mdpi/ic_launcher.png',
  'android/mipmap-mdpi/ic_launcher_foreground.png',
  'android/mipmap-anydpi-v26/ic_launcher.xml',
  'android/values/ic_launcher_background.xml',
];

async function requireFile(filePath, label) {
  const fileStat = await stat(filePath);
  if (!fileStat.isFile() || fileStat.size === 0) {
    throw new Error(`${label} boş veya normal bir dosya değil: ${filePath}`);
  }
  return fileStat;
}

async function readPngDimensions(filePath, label) {
  const contents = await readFile(filePath);
  if (contents.length < 24 || !contents.subarray(0, 8).equals(pngSignature)) {
    throw new Error(`${label} geçerli bir PNG imzasına sahip değil: ${filePath}`);
  }
  if (contents.toString('ascii', 12, 16) !== 'IHDR') {
    throw new Error(`${label} PNG IHDR başlığına sahip değil: ${filePath}`);
  }
  return {
    width: contents.readUInt32BE(16),
    height: contents.readUInt32BE(20),
  };
}

async function validateContainerSignature(filePath, expected, label) {
  const contents = await readFile(filePath);
  if (contents.length < expected.length || !contents.subarray(0, expected.length).equals(expected)) {
    throw new Error(`${label} beklenen dosya imzasına sahip değil: ${filePath}`);
  }
}

async function main() {
  await requireFile(sourceIconPath, 'Kaynak ikon');
  const sourceDimensions = await readPngDimensions(sourceIconPath, 'Kaynak ikon');
  if (sourceDimensions.width !== sourceDimensions.height || sourceDimensions.width < 1024) {
    throw new Error(
      `Kaynak ikon kare ve en az 1024×1024 olmalı; bulunan: ${sourceDimensions.width}×${sourceDimensions.height}`,
    );
  }

  for (const relativePath of requiredGeneratedFiles) {
    await requireFile(path.join(generatedIconRoot, relativePath), `Üretilen ikon (${relativePath})`);
  }

  await validateContainerSignature(
    path.join(generatedIconRoot, 'icon.icns'),
    Buffer.from('icns', 'ascii'),
    'macOS ICNS',
  );
  await validateContainerSignature(
    path.join(generatedIconRoot, 'icon.ico'),
    Buffer.from([0x00, 0x00, 0x01, 0x00]),
    'Windows ICO',
  );

  const tauriConfig = JSON.parse(await readFile(tauriConfigPath, 'utf8'));
  const configuredIcons = tauriConfig.bundle?.icon;
  if (!Array.isArray(configuredIcons) || configuredIcons.length === 0) {
    throw new Error(`Tauri bundle.icon listesi eksik veya boş: ${tauriConfigPath}`);
  }

  for (const configuredIcon of configuredIcons) {
    if (typeof configuredIcon !== 'string' || configuredIcon.trim() === '') {
      throw new Error(`Tauri bundle.icon listesinde geçersiz bir hedef var: ${String(configuredIcon)}`);
    }
    await requireFile(path.resolve(tauriRoot, configuredIcon), `Tauri bundle ikonu (${configuredIcon})`);
  }

  console.log(`Icon source OK: ${path.relative(repositoryRoot, sourceIconPath)} (${sourceDimensions.width}x${sourceDimensions.height})`);
  console.log(`Generated icon set OK: ${path.relative(repositoryRoot, generatedIconRoot)}`);
  console.log(`Tauri bundle icon targets OK: ${configuredIcons.length}`);
}

main().catch((error) => {
  const message = error instanceof Error ? error.message : String(error);
  console.error(`Icon validation failed: ${message}`);
  process.exitCode = 1;
});

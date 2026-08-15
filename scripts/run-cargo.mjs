import { execFileSync, spawn } from 'node:child_process';

const args = process.argv.slice(2);
if (args.length === 0) {
  console.error('Kullanım: node scripts/run-cargo.mjs <cargo-komutu> [argümanlar]');
  process.exit(2);
}

function findExecutable(candidates) {
  for (const candidate of candidates) {
    try {
      const path = execFileSync('which', [candidate], { encoding: 'utf8' }).trim();
      if (path) return { name: candidate, path };
    } catch {
      // Try the next supported linker name.
    }
  }
  return null;
}

function withFastLinker(env) {
  if (process.platform !== 'darwin' || process.arch !== 'arm64') return env;

  const mode = env.RUBRIKA_FAST_LINKER ?? 'auto';
  if (!['auto', 'lld', 'off'].includes(mode)) {
    console.error('RUBRIKA_FAST_LINKER yalnızca auto, lld veya off olabilir.');
    process.exit(2);
  }
  if (mode === 'off' || /-fuse-ld=/.test(env.RUSTFLAGS ?? '')) return env;

  const linker = findExecutable(['ld64.lld', 'lld', 'ld.lld', 'llvm-lld']);
  if (!linker) {
    if (mode === 'lld') {
      console.error('RUBRIKA_FAST_LINKER=lld istendi ancak uyumlu lld linker PATH içinde bulunamadı.');
      process.exit(1);
    }
    console.warn('[cargo-dev] lld bulunamadı; varsayılan macOS linker kullanılıyor.');
    return env;
  }

  const rustflags = env.RUSTFLAGS ? `${env.RUSTFLAGS} ` : '';
  return { ...env, RUSTFLAGS: `${rustflags}-C link-arg=-fuse-ld=${linker.name}` };
}

const child = spawn('cargo', args, {
  stdio: 'inherit',
  env: withFastLinker({ ...process.env }),
});

child.on('error', (error) => {
  console.error(`cargo başlatılamadı: ${error.message}`);
  process.exit(1);
});

child.on('exit', (code, signal) => {
  if (signal) process.exit(1);
  process.exit(code ?? 1);
});

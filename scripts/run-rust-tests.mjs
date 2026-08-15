import { spawn, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const manifestArgs = ['--manifest-path', 'src-tauri/Cargo.toml'];
const extraArgs = process.argv.slice(2);
const requestedRunner = process.env.RUBRIKA_TEST_RUNNER ?? 'auto';

if (!['auto', 'nextest', 'legacy'].includes(requestedRunner)) {
  console.error('RUBRIKA_TEST_RUNNER yalnızca auto, nextest veya legacy olabilir.');
  process.exit(2);
}

const nextestAvailable = spawnSync('cargo', ['nextest', '--version'], {
  stdio: 'ignore',
}).status === 0;

let args;
if (requestedRunner === 'nextest' || (requestedRunner === 'auto' && nextestAvailable)) {
  if (!nextestAvailable) {
    console.error('cargo-nextest bulunamadı. `cargo install cargo-nextest --locked` komutunu çalıştırın.');
    process.exit(1);
  }
  args = ['nextest', 'run', ...manifestArgs, ...extraArgs];
  console.log('[rust-tests] cargo-nextest kullanılıyor.');
} else {
  args = ['test', ...manifestArgs, ...extraArgs];
  if (requestedRunner === 'auto') {
    console.warn('[rust-tests] cargo-nextest bulunamadı; cargo test kullanılıyor.');
  }
}

const cargoRunner = fileURLToPath(new URL('./run-cargo.mjs', import.meta.url));
const child = spawn(process.execPath, [cargoRunner, ...args], {
  stdio: 'inherit',
  env: process.env,
});
child.on('error', (error) => {
  console.error(`Rust test çalıştırıcısı başlatılamadı: ${error.message}`);
  process.exit(1);
});
child.on('exit', (code, signal) => {
  if (signal) process.exit(1);
  process.exit(code ?? 1);
});

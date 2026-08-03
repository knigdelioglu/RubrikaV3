import { spawn } from 'node:child_process';

const args = process.argv.slice(2);
const smokeIndex = args.indexOf('--smoke');
const smokeEnabled = smokeIndex >= 0;
if (smokeEnabled) {
  args.splice(smokeIndex, 1);
}

const child = spawn('tauri', ['dev', ...args], {
  stdio: 'inherit',
  env: {
    ...process.env,
    RUBRIKA_SMOKE: smokeEnabled ? '1' : process.env.RUBRIKA_SMOKE,
    // Current local projects are disposable development fixtures. Keep the
    // production preflight available, but do not make backup/release proof
    // markers block normal writes while running the dev app.
    RUBRIKA_ALLOW_UNVERIFIED_PROJECT_WRITES:
      process.env.RUBRIKA_ALLOW_UNVERIFIED_PROJECT_WRITES ?? '1',
  },
});

child.on('exit', (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 0);
});

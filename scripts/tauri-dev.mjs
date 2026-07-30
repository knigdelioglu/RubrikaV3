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
  },
});

child.on('exit', (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 0);
});

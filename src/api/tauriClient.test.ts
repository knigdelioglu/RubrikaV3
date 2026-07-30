import assert from 'node:assert/strict';
import test from 'node:test';
import { createIdempotentListenerCleanup } from './tauriClient.ts';

test('listener cleanup is single-flight and absorbs teardown races', async () => {
  let successfulCalls = 0;
  let rejectedCalls = 0;
  const cleanup = createIdempotentListenerCleanup([
    async () => {
      successfulCalls += 1;
    },
    async () => {
      rejectedCalls += 1;
      throw new Error('listener already removed by the webview');
    },
  ]);

  cleanup();
  cleanup();
  await new Promise((resolve) => setTimeout(resolve, 0));

  assert.equal(successfulCalls, 1);
  assert.equal(rejectedCalls, 1);
});

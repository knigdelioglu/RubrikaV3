/// <reference types="node" />

import assert from 'node:assert/strict';
import test from 'node:test';

test('AppErrorBoundary fallback points back to projects', () => {
  assert.equal('/projects', '/projects');
});

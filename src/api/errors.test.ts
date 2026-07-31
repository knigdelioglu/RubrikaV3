/// <reference types="node" />

import assert from 'node:assert/strict';
import test from 'node:test';
import { isProjectConflictError } from './errors.ts';

test('project conflicts are safe to expose as refresh actions', () => {
  assert.equal(isProjectConflictError({ code: 'PROJECT_REVISION_CONFLICT' }), true);
  assert.equal(isProjectConflictError({ code: 'PROJECT_EXTERNALLY_MODIFIED' }), true);
  assert.equal(isProjectConflictError({ code: 'PROJECT_SAVE_FAILED' }), false);
  assert.equal(isProjectConflictError(undefined), false);
});

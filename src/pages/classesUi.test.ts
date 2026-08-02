/// <reference types="node" />

import assert from 'node:assert/strict';
import test from 'node:test';
import { getClassesSetupTargetId } from './classesUi.ts';

test('class selection query changes do not change the setup target', () => {
  const initialSearch = new URLSearchParams('setup=classes');
  const afterClassChange = new URLSearchParams('setup=classes&classId=class-11-b');

  assert.equal(
    getClassesSetupTargetId(initialSearch.get('setup')),
    getClassesSetupTargetId(afterClassChange.get('setup')),
  );
  assert.equal(getClassesSetupTargetId(afterClassChange.get('setup')), 'setup-step-classes');
});

test('unknown setup targets do not switch away from the current tab', () => {
  assert.equal(getClassesSetupTargetId(null), null);
  assert.equal(getClassesSetupTargetId('roster'), null);
});

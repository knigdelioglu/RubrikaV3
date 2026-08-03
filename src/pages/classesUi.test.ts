/// <reference types="node" />

import assert from 'node:assert/strict';
import test from 'node:test';
import { getClassesSetupTargetId, getClassesTab, setClassesTab } from './classesUi.ts';

test('roster tab survives class selection query updates', () => {
  const rosterSearch = setClassesTab(new URLSearchParams('setup=classes'), 'roster');
  rosterSearch.set('classId', 'class-11-c');

  assert.equal(getClassesTab(rosterSearch), 'roster');
  assert.equal(rosterSearch.get('setup'), 'classes');
  assert.equal(rosterSearch.get('classId'), 'class-11-c');
});

test('returning to class setup removes the roster tab marker', () => {
  const classSearch = setClassesTab(new URLSearchParams('tab=roster&classId=class-11-c'), 'classes');

  assert.equal(getClassesTab(classSearch), 'classes');
  assert.equal(classSearch.get('tab'), null);
  assert.equal(classSearch.get('classId'), 'class-11-c');
});

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

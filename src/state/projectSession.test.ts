/// <reference types="node" />

import assert from 'node:assert/strict';
import test from 'node:test';

import {
  clearActiveProject,
  getLastProjectId,
  getLastProjectPath,
  resolveProjectIdFromProjects,
  selectStartupProject,
  setActiveProject,
} from './projectSession.ts';

test('resolveProjectIdFromProjects finds a project by path', () => {
  const result = resolveProjectIdFromProjects(
    [
      { id: 'a', path: '/tmp/alpha' },
      { id: 'b', path: '/tmp/bravo' },
    ],
    ' /tmp/bravo ',
  );

  assert.equal(result, 'b');
});

test('resolveProjectIdFromProjects returns empty string when path is missing', () => {
  const result = resolveProjectIdFromProjects([{ id: 'a', path: '/tmp/alpha' }], '/tmp/missing');

  assert.equal(result, '');
});

test('setActiveProject stores last project id and path', () => {
  const storage = new Map<string, string>();
  const originalWindow = globalThis.window;

  // ponytail: tiny in-memory localStorage mock keeps the check focused on the helper contract.
  globalThis.window = {
    localStorage: {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => {
        storage.set(key, value);
      },
      removeItem: (key: string) => {
        storage.delete(key);
      },
    },
  } as typeof window;

  try {
    setActiveProject('project-1', '/tmp/project-1');

    assert.equal(getLastProjectId(), 'project-1');
    assert.equal(getLastProjectPath(), '/tmp/project-1');

    clearActiveProject();

    assert.equal(getLastProjectId(), '');
    assert.equal(getLastProjectPath(), '');
  } finally {
    globalThis.window = originalWindow;
  }
});

test('startup prefers the last project id and falls back to the newest project', () => {
  const projects = [
    { id: 'newest', path: '/tmp/newest' },
    { id: 'last', path: '/tmp/last' },
  ];

  assert.equal(selectStartupProject(projects, 'last', '')?.id, 'last');
  assert.equal(selectStartupProject(projects, 'missing', '/tmp/last')?.id, 'last');
  assert.equal(selectStartupProject(projects, 'missing', '/tmp/missing')?.id, 'newest');
  assert.equal(selectStartupProject([], 'missing', '/tmp/missing'), undefined);
});

/// <reference types="node" />

import assert from 'node:assert/strict';
import test from 'node:test';
import {
  formatAssessmentContext,
  formatAssessmentOption,
  getAssessmentActivityIdFromLocation,
  getProjectSwitcherContextLabel,
  projectActivityPath,
} from './projectSwitcher.ts';

test('assessment selector keeps period and exam sequence visible', () => {
  const activity = { id: 'activity-1', term: 2, sequenceNumber: 1, assessmentType: 'written' as const };

  assert.equal(formatAssessmentContext(activity), '2. Dönem · 1. Sınav');
  assert.equal(formatAssessmentOption(activity), '2. Dönem · 1. Sınav · Yazılı');
});

test('assessment selector summarizes an unselected project without guessing', () => {
  const activities = [
    { id: 'activity-1', term: 1, sequenceNumber: 1, assessmentType: 'written' as const },
    { id: 'activity-2', term: 2, sequenceNumber: 1, assessmentType: 'written' as const },
  ];

  assert.equal(getProjectSwitcherContextLabel(activities, '', false), '2 sınav · seçim yap');
  assert.equal(getProjectSwitcherContextLabel(activities, 'activity-2', false), '2. Dönem · 1. Sınav');
  assert.equal(getProjectSwitcherContextLabel([], '', true), 'Sınavlar yükleniyor…');
});

test('assessment activity id is read from canonical routes or legacy query state', () => {
  assert.equal(
    getAssessmentActivityIdFromLocation('/project/proj%201/activities/activity%2F1/prep'),
    'activity/1',
  );
  assert.equal(
    getAssessmentActivityIdFromLocation('/project/proj%201/overview', '?assessmentActivityId=activity-2'),
    'activity-2',
  );
  assert.equal(getAssessmentActivityIdFromLocation('/project/proj%201/activities'), '');
});

test('assessment selector opens each activity at its first valid workspace step', () => {
  const activity = {
    id: 'activity 1',
    assessmentType: 'listening' as const,
  };

  assert.equal(
    projectActivityPath('project 1', activity),
    '/project/project%201/activities/activity%201/listening_content',
  );
});

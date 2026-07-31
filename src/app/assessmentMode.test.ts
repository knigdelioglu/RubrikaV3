/// <reference types="node" />

import assert from 'node:assert/strict';
import test from 'node:test';
import { getAssessmentMode, getAssessmentModePath, shouldShowProjectNavigation } from './assessmentMode.ts';

test('assessment mode defaults to written and detects the isolated speaking route', () => {
  assert.equal(getAssessmentMode('/project/p1/overview'), 'written');
  assert.equal(getAssessmentMode('/project/p1/activities', '?assessmentType=listening'), 'listening');
  assert.equal(getAssessmentMode('/project/p1/speaking'), 'speaking');
  assert.equal(getAssessmentMode('/project/p1/speaking/session'), 'speaking');
});

test('assessment mode paths keep the written route intact', () => {
  assert.equal(getAssessmentModePath('speaking', 'project 1'), '/project/project%201/speaking');
  assert.equal(getAssessmentModePath('listening', 'project 1'), '/project/project%201/activities?assessmentType=listening');
  assert.equal(getAssessmentModePath('written', 'project 1'), '/project/project%201/overview');
  assert.equal(getAssessmentModePath('written', ''), '/projects');
});

test('project navigation is accessible across all project routes', () => {
  assert.equal(shouldShowProjectNavigation('/project/p1/speaking'), true);
  assert.equal(shouldShowProjectNavigation('/project/p1/speaking/session'), true);
  assert.equal(shouldShowProjectNavigation('/project/p1/overview'), true);
});

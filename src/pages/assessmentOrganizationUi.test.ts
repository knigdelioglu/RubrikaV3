import assert from 'node:assert/strict';
import test from 'node:test';
import { assessmentSequenceOptions, assessmentTypeLabels, canonicalClassApplicationIds, formatDurationRange, recommendedAssessmentSlots, speakingAttemptsForApplication, workflowFamilyLabel } from './assessmentOrganizationUi.ts';

test('assessment organization keeps listening visible while reusing written slots', () => {
  assert.equal(assessmentTypeLabels.listening, 'Dinleme Sınavı');
  assert.deepEqual(recommendedAssessmentSlots('listening'), [1]);
  assert.equal(workflowFamilyLabel('listening'), 'yazılı');
});

test('written and speaking expose two default period slots', () => {
  assert.deepEqual(recommendedAssessmentSlots('written'), [1, 2]);
  assert.deepEqual(recommendedAssessmentSlots('speaking'), [1, 2]);
  assert.equal(workflowFamilyLabel('speaking'), 'konuşma');
});

test('execution scope is derived only from active class applications', () => {
  const activity = {
    classApplications: [
      { id: 'app-a', status: 'scheduled' },
      { id: 'app-b', status: 'archived' },
    ],
  } as never;
  assert.deepEqual(canonicalClassApplicationIds(activity), ['app-a']);
  assert.deepEqual(speakingAttemptsForApplication([
    { id: 'attempt-a', classApplicationId: 'app-a' },
    { id: 'attempt-b', classApplicationId: 'app-b' },
  ] as never, 'app-a').map((attempt) => attempt.id), ['attempt-a']);
});

test('sequence suggestions leave the next available activity slot', () => {
  const activities = [
    { id: 'written-1', courseId: 'tde', term: 1, assessmentType: 'written', sequenceNumber: 1 },
    { id: 'written-2', courseId: 'tde', term: 1, assessmentType: 'written', sequenceNumber: 2 },
  ] as never;
  assert.deepEqual(assessmentSequenceOptions(activities, 'tde', 1, 'written'), [3]);
  assert.deepEqual(assessmentSequenceOptions(activities, 'tde', 1, 'written', 'written-2'), [2]);
  assert.equal(formatDurationRange(120, 240), '2–4 dakika');
});

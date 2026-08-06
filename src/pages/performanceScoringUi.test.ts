import assert from 'node:assert/strict';
import test from 'node:test';
import { derivePerformanceActionAvailability } from './performanceScoringUi.ts';

test('approve and status actions stay unavailable while a save is in flight', () => {
  const availability = derivePerformanceActionAvailability({
    rubricPresent: true,
    isApproved: false,
    isNonRatedStatus: false,
    hasSelectedAssessment: true,
    missingCriteriaCount: 0,
    savePending: true,
    approvePending: false,
    statusPending: false,
  });
  assert.equal(availability.canApprove, false);
  assert.equal(availability.canChangeStatus, false);
  assert.equal(availability.canRevert, false);
});

test('revert of a non-rated record stays disabled while status mutation is pending', () => {
  const availability = derivePerformanceActionAvailability({
    rubricPresent: true,
    isApproved: false,
    isNonRatedStatus: true,
    hasSelectedAssessment: true,
    missingCriteriaCount: 0,
    savePending: false,
    approvePending: false,
    statusPending: true,
  });
  assert.equal(availability.canRevert, false);
  assert.equal(availability.canChangeStatus, false);
});

test('non-rated record allows revert only when no mutation is in flight', () => {
  const availability = derivePerformanceActionAvailability({
    rubricPresent: true,
    isApproved: false,
    isNonRatedStatus: true,
    hasSelectedAssessment: true,
    missingCriteriaCount: 0,
    savePending: false,
    approvePending: false,
    statusPending: false,
  });
  assert.equal(availability.canRevert, true);
  assert.equal(availability.canChangeStatus, false);
});

test('approve is blocked while approve mutation itself is pending (no duplicate submit)', () => {
  const availability = derivePerformanceActionAvailability({
    rubricPresent: true,
    isApproved: false,
    isNonRatedStatus: false,
    hasSelectedAssessment: true,
    missingCriteriaCount: 0,
    savePending: false,
    approvePending: true,
    statusPending: false,
  });
  assert.equal(availability.canApprove, false);
  assert.equal(availability.canChangeStatus, true);
});

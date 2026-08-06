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
});

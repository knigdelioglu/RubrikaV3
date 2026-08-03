import assert from 'node:assert/strict';
import test from 'node:test';
import { canConfirmExternalModel, getModelPrivacyWarning } from './settingsUi.ts';

test('model privacy warning is visible for external or blocked status only', () => {
  assert.equal(getModelPrivacyWarning({ mode: 'managed', privacyBlocked: false }).visible, false);
  assert.equal(getModelPrivacyWarning({ mode: 'external', privacyBlocked: false }).visible, true);
  assert.equal(getModelPrivacyWarning({ mode: 'managed', privacyBlocked: true }).visible, true);
});

test('external model confirmation requires consent and an idle mutation', () => {
  assert.equal(canConfirmExternalModel(false, false), false);
  assert.equal(canConfirmExternalModel(true, true), false);
  assert.equal(canConfirmExternalModel(true, false), true);
});

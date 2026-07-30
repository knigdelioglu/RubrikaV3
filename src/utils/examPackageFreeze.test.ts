import assert from 'node:assert/strict';
import test from 'node:test';

import { getExamPackageFreezeUiState } from './examPackageFreeze.ts';

test('exam package freeze is ready when rubrics are confirmable, regardless of student intake state', () => {
  const state = getExamPackageFreezeUiState(
    { examPackageFreeze: null },
    { summary: { readiness: { examPackageFreeze: true, studentIntake: false, scoring: false }, steps: [] } },
    false,
  );

  assert.equal(state.frozen, false);
  assert.equal(state.readyToFreeze, true);
  assert.equal(state.statusText, 'Sınav paketi dondurmaya hazır');
  assert.equal(state.freezeButtonDisabledReason, undefined);
});

test('exam package freeze stays independent from student intake once frozen', () => {
  const state = getExamPackageFreezeUiState(
    {
      examPackageFreeze: {
        examPackageVersion: 1,
        freezeStatus: 'frozen',
        frozenAt: '2026-06-30T00:00:00Z',
        frozenBy: null,
        sourceHash: 'abc',
        rubricHash: 'def',
        questionTextHash: 'ghi',
        invalidatedAt: null,
        invalidationReason: null,
      },
    },
    { summary: { readiness: { examPackageFreeze: true, studentIntake: false, scoring: false }, steps: [] } },
    false,
  );

  assert.equal(state.frozen, true);
  assert.equal(state.readyToFreeze, false);
  assert.equal(state.statusText, 'Sınav paketi donduruldu');
  assert.equal(state.nextStepText, 'Sonraki adım: Öğrenci PDF’i yükleyin.');
  assert.equal(state.freezeButtonDisabledReason, 'Sınav paketi zaten donduruldu.');
});

test('exam package freeze never invents readiness from validation-like frontend data', () => {
  const state = getExamPackageFreezeUiState(
    { examPackageFreeze: null },
    { summary: { readiness: { examPackageFreeze: false, studentIntake: true, scoring: false }, steps: [] } },
    false,
  );

  assert.equal(state.readyToFreeze, false);
  assert.match(state.freezeButtonDisabledReason ?? '', /Backend/);
});

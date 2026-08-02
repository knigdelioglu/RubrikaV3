import assert from 'node:assert/strict';
import test from 'node:test';
import type { Question, RubricQuestionSnapshot, WorkflowSnapshot } from '../api/types.ts';
import {
  buildExamPackageQuestionItems,
  buildExamPackageWorkspaceSummary,
  createSingleFlightAction,
  mergePersistedDrafts,
  normalizeExamPackageTab,
  resolveSelectedQuestionId,
  rubricDraftFromItem,
} from './examPackageWorkspace.ts';

function question(overrides: Partial<Question> & Pick<Question, 'id' | 'number'>): Question {
  return {
    id: overrides.id,
    number: overrides.number,
    maxScore: overrides.maxScore ?? 0,
    answerType: overrides.answerType ?? 'general_text',
    questionText: overrides.questionText ?? {
      value: `Soru ${overrides.number}`,
      source: 'exam_pdf',
      status: 'confirmed',
      warnings: [],
    },
    rubric: overrides.rubric ?? {
      source: 'manual',
      maxScore: 10,
      expectedAnswer: 'Yanıt',
      criteria: [{ id: `c-${overrides.id}`, label: 'Ölçüt', description: 'Açıklama', points: 10 }],
      partialCreditHints: [],
      zeroScoreConditions: [],
      commonMistakes: [],
      status: 'confirmed',
      warnings: [],
    },
  };
}

function rubricItem(value: Question, valid = true): RubricQuestionSnapshot {
  return {
    question: value,
    validation: {
      valid,
      confirmable: valid,
      warnings: [],
      issues: valid ? [] : [{ code: 'RUBRIC_INVALID', message: 'Rubrik geçersiz.' }],
      totalPoints: value.rubric.maxScore,
    },
  };
}

function workflow(freezeReady: boolean, blockers: string[] = []): WorkflowSnapshot {
  return {
    currentStage: 'exam_package_review_needed',
    currentStageLabel: 'Sınav paketi inceleme bekliyor',
    blockingReasons: blockers,
    nextActions: [],
    summary: {
      steps: [],
      readiness: { examPackageFreeze: freezeReady, studentIntake: false, scoring: false },
    },
  };
}

test('question list shows question and rubric presentation states together', () => {
  const ready = question({ id: 'q1', number: 1 });
  const review = question({
    id: 'q2',
    number: 2,
    questionText: { value: 'Taslak', source: 'exam_pdf', status: 'suggested', warnings: [] },
    rubric: {
      criteria: [], partialCreditHints: [], zeroScoreConditions: [], commonMistakes: [], status: 'missing', warnings: [],
    },
  });
  const items = buildExamPackageQuestionItems([review, ready], [rubricItem(ready), rubricItem(review, false)]);

  assert.deepEqual(items.map((item) => item.number), [1, 2]);
  assert.equal(items[0]?.needsReview, false);
  assert.equal(items[1]?.questionLabel, 'Soru metni kontrol bekliyor');
  assert.equal(items[1]?.rubricLabel, 'Rubrik eksik veya geçersiz');
});

test('deep-link selection opens a valid question and invalid ids safely fall back', () => {
  const questions = [question({ id: 'q2', number: 2 }), question({ id: 'q1', number: 1 })];
  assert.equal(resolveSelectedQuestionId(questions, 'q2'), 'q2');
  assert.equal(resolveSelectedQuestionId(questions, 'missing'), 'q1');
  assert.equal(resolveSelectedQuestionId([], 'missing'), null);
  assert.equal(normalizeExamPackageTab('rubric'), 'rubric');
  assert.equal(normalizeExamPackageTab('freeze'), 'freeze');
  assert.equal(normalizeExamPackageTab('unexpected'), 'question');
});

test('freeze readiness is copied only from the backend workflow snapshot', () => {
  const questions = [question({ id: 'q1', number: 1 })];
  const summary = buildExamPackageWorkspaceSummary(
    { questions, examPackageFreeze: null },
    workflow(false),
    { blockingQuestions: [] },
  );

  assert.equal(summary.readyQuestionCount, 1);
  assert.equal(summary.readyRubricCount, 1);
  assert.equal(summary.freezeReady, false);
});

test('frozen and invalidated package states remain distinct without exposing hashes in summary', () => {
  const questions = [question({ id: 'q1', number: 1 })];
  const freezeBase = {
    examPackageVersion: 2,
    frozenAt: '2026-07-02T08:17:09Z',
    frozenBy: null,
    sourceHash: 'source-secret',
    rubricHash: 'rubric-secret',
    questionTextHash: 'question-secret',
    invalidatedAt: null,
    invalidationReason: null,
  };
  const frozen = buildExamPackageWorkspaceSummary(
    { questions, examPackageFreeze: { ...freezeBase, freezeStatus: 'frozen' } },
    workflow(true),
    null,
  );
  const invalidated = buildExamPackageWorkspaceSummary(
    { questions, examPackageFreeze: { ...freezeBase, freezeStatus: 'invalidated' } },
    workflow(true),
    null,
  );

  assert.equal(frozen.frozen, true);
  assert.equal(frozen.invalidated, false);
  assert.equal(invalidated.frozen, false);
  assert.equal(invalidated.invalidated, true);
  assert.equal('sourceHash' in frozen, false);
});

test('single-flight actions turn duplicate confirmations into one mutation', async () => {
  let calls = 0;
  let release: (() => void) | undefined;
  const guarded = createSingleFlightAction(async () => {
    calls += 1;
    await new Promise<void>((resolve) => { release = resolve; });
    return 'done';
  });

  const first = guarded();
  const second = guarded();
  assert.equal(calls, 1);
  release?.();
  assert.equal(await first, 'done');
  assert.equal(await second, undefined);
});

test('closing a confirmation without invoking the guarded action performs zero mutations', () => {
  let calls = 0;
  createSingleFlightAction(async () => {
    calls += 1;
  });

  assert.equal(calls, 0);
});

test('proof_45_frontend_failed_save_preserves_teacher_draft', () => {
  const current = { q1: 'Öğretmenin yeni rubriği' };
  const afterRefetch = mergePersistedDrafts(current, [['q1', 'Eski rubrik']], new Set(['q1']));
  assert.equal(afterRefetch.q1, 'Öğretmenin yeni rubriği');

  const afterSuccess = mergePersistedDrafts(current, [['q1', 'Kaydedilen rubrik']], new Set());
  assert.equal(afterSuccess.q1, 'Kaydedilen rubrik');
});

test('rubric draft carries the canonical question type for OCR', () => {
  const item = rubricItem(question({ id: 'q-match', number: 4, answerType: 'matching' }));
  const draft = rubricDraftFromItem(item);
  assert.equal(draft.answerType, 'matching');
});

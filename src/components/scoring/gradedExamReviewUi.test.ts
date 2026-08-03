/// <reference types="node" />

import assert from 'node:assert/strict';
import test from 'node:test';
import { annotationColors, calculateFitScale, calculatePreservedPageScale, formatReviewScore, getGradedExamReviewQueue, scoreBreakdown } from './gradedExamReviewUi.ts';
import type { ProjectSnapshot, ScoringRecord } from '../../api/types.ts';

test('missing model score is shown as pending review, never zero', () => {
  assert.equal(formatReviewScore(null), '—');
  assert.equal(formatReviewScore(0), '0');
});

test('annotation tone keeps review-required scores visually distinct', () => {
  assert.equal(annotationColors({ status: 'needs_review', needsReview: true }).border, '#f97316');
  assert.equal(annotationColors({ status: 'model_score', needsReview: false }).border, '#dc2626');
});

test('fit scale contains the complete portrait page without horizontal scrolling', () => {
  const scale = calculateFitScale(900, 700, 1200, 1800, 24);
  assert.ok((1200 * scale) <= 876);
  assert.ok((1800 * scale) <= 676);
});

test('page changes preserve the displayed paper width when source pixel sizes differ', () => {
  const firstPageWidth = 2400;
  const firstPageFitScale = 0.3;
  const secondPageWidth = 1600;
  const secondPageScale = calculatePreservedPageScale(firstPageFitScale, firstPageWidth, secondPageWidth);

  assert.equal(firstPageWidth * firstPageFitScale, secondPageWidth * secondPageScale);
});

test('review queue follows submission page order and ignores historical scoring runs', () => {
  const record = (submissionId: string, runId: string, id: string): ScoringRecord => ({
    id, runId, submissionId, studentId: submissionId, questionId: 'q1', questionNumber: 1,
    maxScore: 10, awardedScore: 8, scoringApplied: true, decisionState: 'auto_accepted', criterionScores: [], rationale: '', confidence: 1,
    needsReview: false, reviewReasons: [], warnings: [], rawModelOutput: '{}', sourceHash: '', packageHash: '',
    ocrRecordHash: '', questionTextHash: '', rubricHash: '', teacherReviewStatus: 'pending_review', createdAt: id, updatedAt: id,
  });
  const project = {
    latestScoringRunId: 'run-new',
    scoringRecords: [record('submission-old', 'run-old', '1'), record('submission-b', 'run-new', '2'), record('submission-a', 'run-new', '3')],
    studentSubmissions: [
      { id: 'submission-b', studentId: 'b', documentId: 'd', pageNumbers: [5], status: 'grouped', answerSlots: [], warnings: [] },
      { id: 'submission-old', studentId: 'old', documentId: 'd', pageNumbers: [1], status: 'grouped', answerSlots: [], warnings: [] },
      { id: 'submission-a', studentId: 'a', documentId: 'd', pageNumbers: [3], status: 'grouped', answerSlots: [], warnings: [] },
    ],
  } as ProjectSnapshot;
  assert.deepEqual(getGradedExamReviewQueue(project), ['submission-a', 'submission-b']);
});

test('criterion scores are shown as an additive breakdown', () => {
  assert.equal(scoreBreakdown({ scoreParts: [
    { title: 'K1', awardedScore: 4, maxScore: 4 },
    { title: 'K2', awardedScore: 4, maxScore: 5 },
    { title: 'K3', awardedScore: 3, maxScore: 5 },
    { title: 'K4', awardedScore: 4, maxScore: 6 },
  ] }), '4+4+3+4');
});

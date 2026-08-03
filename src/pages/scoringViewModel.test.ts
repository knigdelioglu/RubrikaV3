/// <reference types="node" />

import assert from 'node:assert/strict';
import test from 'node:test';
import type { ProjectSnapshot, ScoringRecord } from '../api/types';
import {
  buildStudentSummary,
  dedupeScoringRecords,
  getDecisionStateLabel,
  getScoringAnchorEligibilityLabel,
  getStudentSummaryBadges,
  resolveActiveScoringRunId,
} from './scoringViewModel.ts';

function createProject(): ProjectSnapshot {
  const student = {
    id: 'student-1',
    displayName: 'Öğrenci A',
    number: '501',
    className: '11-C',
    warnings: [],
  };

  const questions = Array.from({ length: 6 }, (_, index) => ({
    id: `q-${index + 1}`,
    number: index + 1,
    maxScore: 10,
    scoringApplied: true,
    answerType: 'general_text' as const,
    questionText: {
      value: `Soru ${index + 1}`,
      source: 'manual' as const,
      status: 'confirmed' as const,
      warnings: [],
    },
    rubric: {
      status: 'confirmed' as const,
      source: 'manual' as const,
      maxScore: 10,
      expectedAnswer: null,
      keyConcepts: [],
      criteria: [],
      partialCreditHints: [],
      zeroScoreConditions: [],
      commonMistakes: [],
      warnings: [],
    },
  }));

  return {
    id: 'project-1',
    name: '11_46',
    createdAt: '2026-07-04T00:00:00Z',
    updatedAt: '2026-07-04T00:00:00Z',
    rootPath: '/tmp/11_46',
    sections: [],
    schoolClasses: [{
      id: 'class-11-c',
      name: '11-C',
      normalizedName: '11-C',
      displayOrder: 1,
      status: 'active',
      createdAt: '2026-07-04T00:00:00Z',
      updatedAt: '2026-07-04T00:00:00Z',
    }],
    studentScanBatches: [{
      id: 'batch-11-c',
      classId: 'class-11-c',
      documentId: 'doc-1',
      originalFileName: '11-C.pdf',
      displayName: '11-C.pdf',
      createdAt: '2026-07-04T00:00:00Z',
      updatedAt: '2026-07-04T00:00:00Z',
    }],
    students: [student],
    studentSubmissions: [
      {
        id: 'submission-1',
        studentId: student.id,
        documentId: 'doc-1',
        classId: 'class-11-c',
        scanBatchId: 'batch-11-c',
        classMembershipSource: 'inherited_from_batch',
        pageNumbers: [1, 2],
        status: 'grouped',
        answerSlots: [],
        warnings: [],
        updatedAt: '2026-07-04T00:00:00Z',
      },
    ],
    studentAnswerOcrRecords: [],
    studentAnswerCropTemplate: { templates: [], updatedAt: null },
    studentIdentityCropTemplate: null,
    studentScanDocumentId: null,
    studentGroupingMode: null,
    studentPagesPerStudent: null,
    studentGroupingCompleteAt: null,
    expectedQuestionCount: 6,
    examPackageFreeze: null,
    documents: [],
    questions,
    scoringRecords: [],
    latestScoringRunId: null,
    workflow: {
      currentStage: 'scoring_ready',
      currentStageLabel: 'Notlandırma Hazır',
      blockingReasons: [],
      nextActions: [],
      summary: { text: null, steps: [], readiness: { examPackageFreeze: true, studentIntake: true, scoring: true } },
    },
  } as ProjectSnapshot;
}

function createRecord(overrides: Partial<ScoringRecord> & Pick<ScoringRecord, 'id' | 'runId' | 'submissionId' | 'questionId' | 'questionNumber' | 'awardedScore' | 'createdAt' | 'updatedAt'>): ScoringRecord {
  return {
    studentId: 'student-1',
    studentDisplayName: 'Öğrenci A',
    studentNumber: '501',
    studentClassName: '11-C',
    maxScore: 10,
    criterionScores: [],
    rationale: '',
    confidence: 0.9,
    needsReview: false,
    warnings: [],
    rawModelOutput: '{}',
    parseDiagnostics: null,
    reconciliationDiagnostics: null,
    sourceHash: 'source',
    packageHash: 'package',
    ocrRecordHash: 'ocr',
    questionTextHash: 'qt',
    rubricHash: 'rubric',
    teacherReviewStatus: 'pending_review',
    decisionState: 'auto_accepted',
    teacherManualScore: null,
    teacherReviewedAt: null,
    teacherNotes: null,
    invalidatedAt: null,
    invalidationReason: null,
    ...overrides,
    scoringApplied: overrides.scoringApplied ?? true,
    reviewReasons: overrides.reviewReasons ?? [],
  };
}

test('resolveActiveScoringRunId prefers explicit latest run id', () => {
  assert.equal(
    resolveActiveScoringRunId({
      latestScoringRunId: 'run-new',
      scoringRecords: [
        createRecord({
          id: 'old',
          runId: 'run-old',
          submissionId: 'submission-1',
          questionId: 'q-1',
          questionNumber: 1,
          awardedScore: 4,
          createdAt: '2026-07-04T00:00:00Z',
          updatedAt: '2026-07-04T00:00:00Z',
        }),
      ],
    }),
    'run-new',
  );
});

test('decision states stay teacher-facing and do not expose backend codes', () => {
  assert.equal(getDecisionStateLabel('model_candidate'), 'Model adayı · onay bekliyor');
  assert.equal(getDecisionStateLabel('deterministic_accepted'), 'Deterministik kabul');
  assert.equal(getDecisionStateLabel('teacher_approved'), 'Öğretmen finali');
});

test('anchor eligibility uses teacher-facing labels', () => {
  assert.equal(getScoringAnchorEligibilityLabel('eligible'), 'Kullanıma uygun anchor');
  assert.equal(getScoringAnchorEligibilityLabel('stale'), 'Güncelliğini yitirdi; yeniden değerlendirme gerekli');
  assert.equal(getScoringAnchorEligibilityLabel('revoked'), 'Anchor statüsü kaldırıldı');
});

test('dedupeScoringRecords keeps the latest record per student-question key', () => {
  const activeRunRecords = [
    createRecord({
      id: 'old-1',
      runId: 'run-old',
      submissionId: 'submission-1',
      questionId: 'q-1',
      questionNumber: 1,
      awardedScore: 4,
      createdAt: '2026-07-04T00:00:01Z',
      updatedAt: '2026-07-04T00:00:01Z',
    }),
    createRecord({
      id: 'new-1',
      runId: 'run-new',
      submissionId: 'submission-1',
      questionId: 'q-1',
      questionNumber: 1,
      awardedScore: 8,
      createdAt: '2026-07-04T00:01:00Z',
      updatedAt: '2026-07-04T00:01:00Z',
    }),
    createRecord({
      id: 'new-2',
      runId: 'run-new',
      submissionId: 'submission-1',
      questionId: 'q-2',
      questionNumber: 2,
      awardedScore: 9,
      createdAt: '2026-07-04T00:01:01Z',
      updatedAt: '2026-07-04T00:01:01Z',
    }),
  ];

  const deduped = dedupeScoringRecords(activeRunRecords);
  assert.equal(deduped.length, 2);
  assert.equal(deduped[0].questionId, 'q-1');
  assert.equal(deduped[0].awardedScore, 8);
  assert.equal(deduped[1].questionId, 'q-2');
});

test('buildStudentSummary keeps duplicate history out of the active total', () => {
  const project = createProject();
  project.latestScoringRunId = 'run-new';

  const history = project.questions.flatMap((question, index) => {
    const oldScore = index === 0 ? 4 : 10;
    const newScore = index === 0 ? 8 : 10;
    return [
      createRecord({
        id: `old-${index + 1}`,
        runId: 'run-old',
        submissionId: 'submission-1',
        questionId: question.id,
        questionNumber: question.number,
        awardedScore: oldScore,
        createdAt: `2026-07-04T00:00:${String(index).padStart(2, '0')}Z`,
        updatedAt: `2026-07-04T00:00:${String(index).padStart(2, '0')}Z`,
      }),
      createRecord({
        id: `new-${index + 1}`,
        runId: 'run-new',
        submissionId: 'submission-1',
        questionId: question.id,
        questionNumber: question.number,
        awardedScore: newScore,
        createdAt: `2026-07-04T00:01:${String(index).padStart(2, '0')}Z`,
        updatedAt: `2026-07-04T00:01:${String(index).padStart(2, '0')}Z`,
      }),
    ];
  });

  project.scoringRecords = history;
  const activeRecords = dedupeScoringRecords(
    project.scoringRecords.filter((record) => record.runId === resolveActiveScoringRunId(project)),
  );
  const summary = buildStudentSummary(project, project.studentSubmissions[0], activeRecords, {
    submissionId: 'submission-1',
    provisionalScore: 58,
    acceptedScore: 58,
    finalScore: 58,
    maxScore: 60,
    isComplete: true,
    expectedRecordCount: 6,
    acceptedRecordCount: 6,
    provisionalRecordCount: 0,
    reviewRequiredCount: 0,
  });

  assert.equal(activeRecords.length, 6);
  assert.equal(summary.records.length, 6);
  assert.equal(summary.totalScore, 58);
  assert.equal(summary.duplicateCount, 6);
  assert.deepEqual(summary.badges, ['Onay bekliyor']);
});

test('buildStudentSummary does not turn an unapplied model failure into zero points', () => {
  const project = createProject();
  const question = project.questions[0];
  const submission = project.studentSubmissions[0];
  assert.ok(question);
  assert.ok(submission);
  const record = createRecord({
    id: 'failed-1',
    runId: 'run-failed',
    submissionId: 'submission-1',
    questionId: question.id,
    questionNumber: 1,
    awardedScore: null,
    scoringApplied: false,
    decisionState: 'failed',
    needsReview: true,
    reviewReasons: ['scoring_json_parse_failed'],
    createdAt: '2026-07-04T00:00:00Z',
    updatedAt: '2026-07-04T00:00:00Z',
  });

  const summary = buildStudentSummary(project, submission, [record], {
    submissionId: submission.id,
    provisionalScore: 0,
    acceptedScore: 0,
    finalScore: null,
    maxScore: 60,
    isComplete: false,
    expectedRecordCount: 6,
    acceptedRecordCount: 0,
    provisionalRecordCount: 0,
    reviewRequiredCount: 1,
  });

  assert.equal(summary.totalScore, null);
  assert.equal(summary.scoredCount, 0);
  assert.equal(summary.unscoredCount, 6);
  assert.equal(summary.isComplete, false);
  assert.equal(record.awardedScore, null);
});

test('buildStudentSummary excludes scoringApplied false even when a model score is present', () => {
  const project = createProject();
  const submission = project.studentSubmissions[0];
  assert.ok(submission);
  const records = project.questions.map((question, index) => createRecord({
    id: `record-${index}`,
    runId: 'run-1',
    submissionId: submission.id,
    questionId: question.id,
    questionNumber: question.number,
    awardedScore: 10,
    scoringApplied: index !== 0,
    createdAt: `2026-07-04T00:00:0${index}Z`,
    updatedAt: `2026-07-04T00:00:0${index}Z`,
  }));

  const summary = buildStudentSummary(project, submission, records, {
    submissionId: submission.id,
    provisionalScore: 50,
    acceptedScore: 50,
    finalScore: null,
    maxScore: 60,
    isComplete: false,
    expectedRecordCount: 6,
    acceptedRecordCount: 5,
    provisionalRecordCount: 0,
    reviewRequiredCount: 1,
  });
  assert.equal(summary.totalScore, 50);
  assert.equal(summary.scoredCount, 5);
  assert.equal(summary.unscoredCount, 1);
  assert.equal(summary.isComplete, false);
});

test('getStudentSummaryBadges marks review-needed students clearly', () => {
  assert.deepEqual(
    getStudentSummaryBadges({
      hasRecords: true,
      needsReview: true,
      warningCount: 2,
      approvedCount: 0,
      pendingCount: 1,
    }),
    ['İnceleme gerekli', 'Uyarı var', 'Onay bekliyor'],
  );
});

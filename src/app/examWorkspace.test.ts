import test from 'node:test';
import assert from 'node:assert/strict';
import {
  deriveExamStepStatuses,
  getCanonicalWorkspaceStepPath,
  getExamStepDefinitions,
  resolveNextExamStep,
} from './examWorkspace.ts';
import type { AssessmentActivity, PerformanceStatus, WorkflowSnapshot } from '../api/types';

const baseActivity: AssessmentActivity = {
  id: 'activity_001',
  academicYearId: '2025-2026',
  courseId: 'course_turkce',
  courseName: 'Türk Dili ve Edebiyatı',
  title: '11. Sınıf 2. Dönem 1. Ortak Yazılı',
  gradeLevel: 11,
  term: 2,
  assessmentType: 'written',
  workflowFamily: 'written',
  sequenceNumber: 1,
  status: 'active',
  commonDocumentIds: [],
  classApplications: [
    {
      id: 'app_11a',
      activityId: 'activity_001',
      schoolClassId: 'class_11a',
      status: 'active',
      documentIds: [],
      studentScopeIds: ['std_1', 'std_2'],
      speakingAttempts: [],
      createdAt: '2026-01-01T00:00:00Z',
      updatedAt: '2026-01-01T00:00:00Z',
    },
    {
      id: 'app_11b',
      activityId: 'activity_001',
      schoolClassId: 'class_11b',
      status: 'active',
      documentIds: [],
      studentScopeIds: ['std_3', 'std_4'],
      speakingAttempts: [],
      createdAt: '2026-01-01T00:00:00Z',
      updatedAt: '2026-01-01T00:00:00Z',
    },
  ],
  createdAt: '2026-01-01T00:00:00Z',
  updatedAt: '2026-01-01T00:00:00Z',
};

const defaultWorkflowSnapshot: WorkflowSnapshot = {
  currentStage: 'question_text_missing',
  currentStageLabel: 'Soru Metni Eksik',
  blockingReasons: ['QUESTION_TEXT_MISSING'],
  nextActions: [],
  summary: {
    steps: [],
    readiness: {
      examPackageFreeze: false,
      studentIntake: false,
      scoring: false,
    },
  },
};

test('returns 5 canonical steps for written, listening, and speaking exam types', () => {
  const writtenSteps = getExamStepDefinitions('written');
  assert.equal(writtenSteps.length, 5);
  assert.deepEqual(
    writtenSteps.map((s) => s.id),
    ['prep', 'students', 'ocr', 'scoring', 'results'],
  );

  const listeningSteps = getExamStepDefinitions('listening');
  assert.equal(listeningSteps.length, 5);
  assert.deepEqual(
    listeningSteps.map((s) => s.id),
    ['listening_content', 'questions', 'students', 'ocr_scoring', 'results'],
  );

  const speakingSteps = getExamStepDefinitions('speaking');
  assert.equal(speakingSteps.length, 5);
  assert.deepEqual(
    speakingSteps.map((s) => s.id),
    ['settings', 'students', 'transcript', 'evaluation', 'results'],
  );
});

test('written step status marks student intake blocked when prep is incomplete', () => {
  const states = deriveExamStepStatuses(baseActivity, defaultWorkflowSnapshot);
  const prepState = states.find((s) => s.definition.id === 'prep');
  const studentsState = states.find((s) => s.definition.id === 'students');

  assert.equal(prepState?.status, 'ready');
  assert.equal(studentsState?.status, 'blocked');
  assert.match(studentsState?.blockerMessage ?? '', /dondurulmadan/);
});

test('written step status marks student intake ready when exam package is frozen', () => {
  const frozenSnapshot: WorkflowSnapshot = {
    ...defaultWorkflowSnapshot,
    currentStage: 'qep_frozen',
    summary: {
      steps: [],
      readiness: {
        examPackageFreeze: true,
        studentIntake: false,
        scoring: false,
      },
    },
  };

  const states = deriveExamStepStatuses(baseActivity, frozenSnapshot);
  const prepState = states.find((s) => s.definition.id === 'prep');
  const studentsState = states.find((s) => s.definition.id === 'students');

  assert.equal(prepState?.status, 'completed');
  assert.equal(studentsState?.status, 'ready');
});

function listeningActivity(): AssessmentActivity {
  return {
    ...baseActivity,
    id: 'activity_listen',
    assessmentType: 'listening',
    workflowFamily: 'listening',
    listeningDetails: {
      audioDocumentId: 'audio_1',
      playCount: 2,
      durationSeconds: 300,
      instruction: 'İki kez dinletin.',
    },
  };
}

test('listening questions step consumes backend examPackageFreeze readiness', () => {
  const notFrozenSnapshot: WorkflowSnapshot = {
    ...defaultWorkflowSnapshot,
    currentStage: 'question_text_missing',
    summary: {
      steps: [],
      readiness: {
        examPackageFreeze: false,
        studentIntake: false,
        scoring: false,
      },
    },
  };
  const blockedStates = deriveExamStepStatuses(listeningActivity(), notFrozenSnapshot);
  assert.equal(
    blockedStates.find((s) => s.definition.id === 'questions')?.status,
    'ready',
    'unfrozen backend readiness must keep the questions step open',
  );

  const frozenSnapshot: WorkflowSnapshot = {
    ...defaultWorkflowSnapshot,
    currentStage: 'qep_frozen',
    summary: {
      steps: [],
      readiness: {
        examPackageFreeze: true,
        studentIntake: false,
        scoring: false,
      },
    },
  };
  const frozenStates = deriveExamStepStatuses(listeningActivity(), frozenSnapshot);
  assert.equal(
    frozenStates.find((s) => s.definition.id === 'questions')?.status,
    'completed',
    'questions step must complete only from backend examPackageFreeze readiness',
  );
  const studentsState = frozenStates.find((s) => s.definition.id === 'students');
  assert.equal(studentsState?.status, 'ready');
});

test('listening students step consumes backend studentIntake readiness', () => {
  const intakeSnapshot: WorkflowSnapshot = {
    ...defaultWorkflowSnapshot,
    currentStage: 'student_answer_ocr_ready_for_scoring',
    summary: {
      steps: [],
      readiness: {
        examPackageFreeze: true,
        studentIntake: true,
        scoring: false,
      },
    },
  };
  const states = deriveExamStepStatuses(listeningActivity(), intakeSnapshot);
  assert.equal(
    states.find((s) => s.definition.id === 'students')?.status,
    'completed',
    'students step must complete only from backend studentIntake readiness',
  );
});

test('speaking step status marks transcript blocked until task text is set', () => {
  const speakingActivity: AssessmentActivity = {
    ...baseActivity,
    assessmentType: 'speaking',
    workflowFamily: 'speaking',
    speakingConfiguration: null,
  };

  const states = deriveExamStepStatuses(speakingActivity);
  const settingsState = states.find((s) => s.definition.id === 'settings');
  const transcriptState = states.find((s) => s.definition.id === 'transcript');

  assert.equal(settingsState?.status, 'ready');
  assert.equal(transcriptState?.status, 'blocked');
  assert.match(transcriptState?.blockerMessage ?? '', /tamamlanmadan/);
});

test('speaking step status updates transcript to in_progress when attempts exist', () => {
  const activeSpeakingActivity: AssessmentActivity = {
    ...baseActivity,
    assessmentType: 'speaking',
    workflowFamily: 'speaking',
    speakingConfiguration: {
      speakingType: 'prepared',
      taskText: 'Serbest konuşma konusu',
      targetDurationSeconds: 180,
      minDurationSeconds: 120,
      maxDurationSeconds: 240,
      rubricVersion: 'v1',
      scoringPolicyVersion: 'v1',
      cleanupPromptVersion: 'v1',
      evaluationPromptVersion: 'v1',
      rubricSnapshot: {},
    },
    classApplications: [
      {
        ...baseActivity.classApplications[0]!,
        speakingAttempts: [
          {
            id: 'att_1',
            activityId: 'activity_001',
            classApplicationId: 'app_11a',
            schoolClassId: 'class_11a',
            studentId: 'std_1',
            speakingType: 'prepared',
            taskText: 'Serbest konuşma konusu',
            targetDurationSeconds: 180,
            minDurationSeconds: 120,
            maxDurationSeconds: 240,
            state: 'approved',
            createdAt: '2026-01-01T00:00:00Z',
            updatedAt: '2026-01-01T00:00:00Z',
          },
        ],
      },
    ],
  };

  const states = deriveExamStepStatuses(activeSpeakingActivity);
  const transcriptState = states.find((s) => s.definition.id === 'transcript');
  const evaluationState = states.find((s) => s.definition.id === 'evaluation');

  assert.equal(transcriptState?.status, 'in_progress');
  assert.equal(evaluationState?.status, 'ready');
});

test('resolveNextExamStep selects the first ready or in-progress step', () => {
  const nextWritten = resolveNextExamStep(baseActivity, defaultWorkflowSnapshot);
  assert.equal(nextWritten.id, 'prep');

  const ocrNeededSnapshot: WorkflowSnapshot = {
    ...defaultWorkflowSnapshot,
    currentStage: 'student_answer_ocr_review_needed',
    summary: {
      steps: [],
      readiness: {
        examPackageFreeze: true,
        studentIntake: true,
        scoring: false,
      },
    },
  };

  const nextOcr = resolveNextExamStep(baseActivity, ocrNeededSnapshot);
  assert.equal(nextOcr.id, 'ocr');
});

test('getCanonicalWorkspaceStepPath strips project path parameters and preserves custom queries', () => {
  const path = getCanonicalWorkspaceStepPath('proj_1', 'act_10', 'students', '?classApplicationId=app_11a&tab=grouping');
  assert.equal(
    path,
    '/project/proj_1/activities/act_10/students?classApplicationId=app_11a&tab=grouping',
  );
});

function performanceActivity(overrides: Partial<AssessmentActivity> = {}): AssessmentActivity {
  return {
    ...baseActivity,
    id: 'activity_perf',
    assessmentType: 'performance',
    workflowFamily: 'performance',
    performanceDetails: null,
    classApplications: [],
    ...overrides,
  };
}

function performanceStatus(overrides: Partial<PerformanceStatus> = {}): PerformanceStatus {
  return {
    hasPublishedRubric: false,
    publishedRubricVersion: null,
    hasDraftRubric: true,
    hasTaskDetails: true,
    totalStudents: 2,
    approvedCount: 0,
    inProgressCount: 0,
    missingCount: 0,
    notPerformedCount: 0,
    allApproved: false,
    ...overrides,
  };
}

test('performance steps fall back to blocked when the status DTO is unavailable', () => {
  const states = deriveExamStepStatuses(performanceActivity(), null, null, null);
  assert.equal(states.length, 3);
  assert.deepEqual(
    states.map((s) => s.status),
    ['blocked', 'blocked', 'blocked'],
  );
  assert.match(states[1]?.blockerMessage ?? '', /rubrik/);
});

test('performance task step renders from DTO publish state', () => {
  const draftStatus = performanceStatus();
  const states = deriveExamStepStatuses(performanceActivity(), null, null, draftStatus);
  const taskState = states.find((s) => s.definition.id === 'task');
  assert.equal(taskState?.status, 'in_progress');

  const publishedStatus = performanceStatus({ hasPublishedRubric: true, publishedRubricVersion: 1, hasDraftRubric: false });
  const publishedStates = deriveExamStepStatuses(performanceActivity(), null, null, publishedStatus);
  assert.equal(
    publishedStates.find((s) => s.definition.id === 'task')?.status,
    'completed',
  );
});

test('performance assessment step blocks without published rubric and completes on allApproved', () => {
  const draftStatus = performanceStatus();
  const draftStates = deriveExamStepStatuses(performanceActivity(), null, null, draftStatus);
  assert.equal(
    draftStates.find((s) => s.definition.id === 'assessment')?.status,
    'blocked',
  );

  const publishedStatus = performanceStatus({ hasPublishedRubric: true, publishedRubricVersion: 1, hasDraftRubric: false });
  const readyStates = deriveExamStepStatuses(performanceActivity(), null, null, publishedStatus);
  assert.equal(
    readyStates.find((s) => s.definition.id === 'assessment')?.status,
    'ready',
  );

  const inProgressStatus = performanceStatus({
    hasPublishedRubric: true,
    publishedRubricVersion: 1,
    hasDraftRubric: false,
    inProgressCount: 1,
  });
  const inProgressStates = deriveExamStepStatuses(performanceActivity(), null, null, inProgressStatus);
  assert.equal(
    inProgressStates.find((s) => s.definition.id === 'assessment')?.status,
    'in_progress',
  );

  const allApprovedStatus = performanceStatus({
    hasPublishedRubric: true,
    publishedRubricVersion: 1,
    hasDraftRubric: false,
    approvedCount: 2,
    allApproved: true,
  });
  const completedStates = deriveExamStepStatuses(performanceActivity(), null, null, allApprovedStatus);
  assert.equal(
    completedStates.find((s) => s.definition.id === 'assessment')?.status,
    'completed',
  );
});

test('performance results step renders from DTO approvedCount', () => {
  const noneApproved = performanceStatus({ hasPublishedRubric: true, publishedRubricVersion: 1, hasDraftRubric: false });
  const blockedStates = deriveExamStepStatuses(performanceActivity(), null, null, noneApproved);
  assert.equal(
    blockedStates.find((s) => s.definition.id === 'results')?.status,
    'blocked',
  );

  const withApproval = performanceStatus({
    hasPublishedRubric: true,
    publishedRubricVersion: 1,
    hasDraftRubric: false,
    approvedCount: 1,
  });
  const readyStates = deriveExamStepStatuses(performanceActivity(), null, null, withApproval);
  assert.equal(
    readyStates.find((s) => s.definition.id === 'results')?.status,
    'ready',
  );
});

test('resolveNextExamStep for performance uses the status DTO', () => {
  const publishedStatus = performanceStatus({ hasPublishedRubric: true, publishedRubricVersion: 1, hasDraftRubric: false });
  assert.equal(
    resolveNextExamStep(performanceActivity(), null, null, publishedStatus).id,
    'assessment',
  );
  const approvedStatus = performanceStatus({
    hasPublishedRubric: true,
    publishedRubricVersion: 1,
    hasDraftRubric: false,
    approvedCount: 2,
    allApproved: true,
  });
  assert.equal(
    resolveNextExamStep(performanceActivity(), null, null, approvedStatus).id,
    'results',
  );
});

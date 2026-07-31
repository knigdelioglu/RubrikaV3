/// <reference types="node" />

import assert from 'node:assert/strict';
import test from 'node:test';
import type { JobSnapshot } from '../api/types.ts';
import {
  getActiveJobs,
  getFailedJobs,
  getJobCenterButtonLabel,
  getJobLabel,
  getJobProgressPercent,
} from './globalJobs.ts';
import {
  getProjectArea,
  getExamPackageActionPath,
  getStudentOperationsActionPath,
  getProjectIdFromPathname,
  projectDocumentsPath,
  projectExamPackagePath,
  projectNavigation,
  projectOverviewPath,
  projectStudentOperationsPath,
  resolveLegacyProjectDestination,
  resolveLegacyProjectPath,
} from './projectRoutes.ts';

const job = (overrides: Partial<JobSnapshot> = {}): JobSnapshot => ({
  id: 'internal-job-id',
  projectId: 'project-1',
  kind: 'student_answer_ocr',
  status: 'running',
  progress: { current: 18, total: 32, message: '18 / 32 öğrenci tamamlandı' },
  createdAt: '2026-07-21T10:00:00Z',
  updatedAt: '2026-07-21T10:01:00Z',
  ...overrides,
});

test('project navigation exposes the 5 canonical teacher workspaces', () => {
  assert.equal(projectNavigation.length, 5);
  assert.deepEqual(projectNavigation.map((item) => item.label), [
    'Ana Sayfa',
    'Sınavlar',
    'Sınıflar ve Öğrenciler',
    'Raporlar',
    'Ayarlar',
  ]);
  assert.equal(projectOverviewPath('project 1'), '/project/project%201/overview');
});

test('nested project routes resolve their active teacher area', () => {
  assert.equal(getProjectArea('/project/p1/exam/rubrics'), 'activities');
  assert.equal(getProjectArea('/project/p1/ocr/review'), 'activities');
  assert.equal(getProjectArea('/project/p1/classes'), 'classes');
  assert.equal(getProjectArea('/project/p1/activities'), 'activities');
  assert.equal(getProjectArea('/project/p1/analysis'), 'analysis');
  assert.equal(getProjectArea('/project/p1/settings/model'), 'settings');
  assert.equal(getProjectArea('/workflow'), 'overview');
});

test('a nested project route opened directly retains its project context', () => {
  const examRoute = '/project/project%201/exam/documents';
  const ocrRoute = '/project/project%201/ocr/review';

  assert.equal(getProjectIdFromPathname(examRoute), 'project 1');
  assert.equal(getProjectIdFromPathname(ocrRoute), 'project 1');
  assert.equal(getProjectArea(examRoute), 'activities');
  assert.equal(getProjectArea(ocrRoute), 'activities');
});

test('legacy project routes resolve to their canonical nested routes', () => {
  assert.equal(resolveLegacyProjectPath('/documents', 'project 1'), '/project/project%201/exam/documents');
  assert.equal(resolveLegacyProjectPath('/pdf-preview', 'project 1'), '/project/project%201/exam/documents');
  assert.equal(resolveLegacyProjectPath('/student-scans', 'project 1'), '/project/project%201/students');
  assert.equal(resolveLegacyProjectPath('/model-status', 'project 1'), '/project/project%201/settings/model');
  assert.equal(resolveLegacyProjectPath('/question-text', 'project 1'), '/project/project%201/exam/package');
  assert.equal(resolveLegacyProjectPath('/rubric-preparation', 'project 1'), '/project/project%201/exam/package');
});

test('question, rubric, and package compatibility routes preserve selection and select the correct tab', () => {
  assert.equal(
    resolveLegacyProjectDestination('/question-text', 'project 1', '?questionId=q-2&criterionId=c-1'),
    '/project/project%201/exam/package?questionId=q-2&criterionId=c-1&tab=question',
  );
  assert.equal(
    resolveLegacyProjectDestination('/rubric-preparation', 'project 1', '?questionId=q-2&review=required'),
    '/project/project%201/exam/package?questionId=q-2&review=required&tab=rubric',
  );
  assert.equal(
    resolveLegacyProjectDestination('/exam-package-review', 'project 1', '?mode=review'),
    '/project/project%201/exam/package?mode=review&tab=freeze',
  );
  assert.equal(
    projectExamPackagePath('project 1', 'rubric', '?questionId=q-2&criterionId=c-1'),
    '/project/project%201/exam/package?questionId=q-2&criterionId=c-1&tab=rubric',
  );
});

test('workflow next actions target the matching canonical workspace tab', () => {
  assert.equal(getExamPackageActionPath('p1', 'open_question_text_page'), '/project/p1/exam/package?tab=question');
  assert.equal(getExamPackageActionPath('p1', 'prepare_rubric'), '/project/p1/exam/package?tab=rubric');
  assert.equal(getExamPackageActionPath('p1', 'confirm_all_rubrics'), '/project/p1/exam/package?tab=freeze');
  assert.equal(getExamPackageActionPath('p1', 'start_scoring_job'), null);
  assert.equal(getStudentOperationsActionPath('p1', 'open_student_grouping_page'), '/project/p1/students?tab=grouping');
  assert.equal(getStudentOperationsActionPath('p1', 'open_student_identity_page'), '/project/p1/students?tab=identity');
  assert.equal(getStudentOperationsActionPath('p1', 'open_student_answer_ocr_page'), '/project/p1/students?tab=ocr');
  assert.equal(getStudentOperationsActionPath('p1', 'open_student_answer_ocr_issue_review_page'), '/project/p1/students?tab=issues');
});

test('student workspace deep links preserve class and batch selection', () => {
  assert.equal(
    projectStudentOperationsPath('project 1', 'identity', '?classId=class-1&batchId=batch-1'),
    '/project/project%201/students?classId=class-1&batchId=batch-1&tab=identity',
  );
  assert.equal(
    resolveLegacyProjectDestination('/student-answer-ocr-issues', 'project 1', '?classId=class-1'),
    '/project/project%201/students?classId=class-1&tab=issues',
  );
});

test('legacy PDF preview deep links preserve document and page selection', () => {
  const legacySearch = '?projectId=project%201&documentId=student-1&documentType=student&page=3';
  assert.equal(
    resolveLegacyProjectDestination('/pdf-preview', 'project 1', legacySearch),
    '/project/project%201/exam/documents?documentId=student-1&documentType=student&page=3',
  );
  assert.equal(
    projectDocumentsPath('project 1', '?documentId=exam-1&page=2'),
    '/project/project%201/exam/documents?documentId=exam-1&page=2',
  );
});

test('sidebar active area follows nested and legacy routes', () => {
  assert.equal(getProjectArea('/project/p1/students/grouping'), 'activities');
  assert.equal(getProjectArea('/student-scans'), 'activities');
  assert.equal(getProjectArea('/project/p1/grading/papers'), 'activities');
  assert.equal(getProjectArea('/graded-exam-review'), 'activities');
});

test('global jobs expose teacher labels and bounded progress without internal ids', () => {
  const running = job();
  const failed = job({ id: 'failed-id', status: 'failed' });
  assert.deepEqual(getActiveJobs([running, failed]), [running]);
  assert.deepEqual(getFailedJobs([running, failed]), [failed]);
  assert.equal(getJobLabel(running.kind), 'Öğrenci cevapları okunuyor');
  assert.equal(getJobProgressPercent(running), 56);
  assert.equal(getJobProgressPercent(job({ progress: { current: 4, total: 0, message: '' } })), 0);
  assert.equal(
    getJobLabel(job({ kind: 'assessment_analysis' }).kind),
    'Sınav analizi hazırlanıyor',
  );
});

test('global job center keeps showing an active job while the project route changes', () => {
  const jobs = [job()];

  assert.equal(getProjectArea('/project/project-1/exam/documents'), 'activities');
  assert.equal(getJobCenterButtonLabel(jobs), 'Öğrenci cevapları okunuyor');
  assert.equal(getProjectArea('/project/project-1/ocr/review'), 'activities');
  assert.equal(getJobCenterButtonLabel(jobs), 'Öğrenci cevapları okunuyor');
});

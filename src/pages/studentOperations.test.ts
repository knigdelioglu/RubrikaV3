/// <reference types="node" />

import assert from 'node:assert/strict';
import test from 'node:test';
import type { SchoolClass, Student, StudentScanBatch, StudentSubmission } from '../api/types.ts';
import {
  filterStudentSubmissions,
  getSubmissionClassName,
  getStudentBatchImportDisabledReason,
  getStudentTeacherLabel,
  hasIdentityClassMismatch,
  normalizeStudentOperationsTab,
  resolveStudentOperationsSelection,
  suggestSchoolClassFromFilename,
} from './studentOperations.ts';

const classes: SchoolClass[] = [
  {
    id: 'class-internal-a',
    name: '11-A',
    normalizedName: '11-A',
    displayOrder: 1,
    status: 'active',
    createdAt: '2026-07-21T00:00:00Z',
    updatedAt: '2026-07-21T00:00:00Z',
  },
  {
    id: 'class-internal-c',
    name: '11-C',
    normalizedName: '11-C',
    displayOrder: 2,
    status: 'active',
    createdAt: '2026-07-21T00:00:00Z',
    updatedAt: '2026-07-21T00:00:00Z',
  },
];

const batches: StudentScanBatch[] = [
  {
    id: 'batch-a',
    classId: classes[0]!.id,
    documentId: 'doc-a',
    originalFileName: '11A.pdf',
    displayName: '11A.pdf',
    createdAt: '2026-07-21T00:00:00Z',
    updatedAt: '2026-07-21T00:00:00Z',
  },
  {
    id: 'batch-c',
    classId: classes[1]!.id,
    documentId: 'doc-c',
    originalFileName: '11-C öğrenci sınavları.pdf',
    displayName: '11-C öğrenci sınavları.pdf',
    createdAt: '2026-07-21T00:00:00Z',
    updatedAt: '2026-07-21T00:00:00Z',
  },
];

const submissions: StudentSubmission[] = batches.map((batch, index) => ({
  id: `submission-${index + 1}`,
  studentId: `student-${index + 1}`,
  documentId: batch.documentId,
  classId: batch.classId,
  scanBatchId: batch.id,
  classMembershipSource: 'inherited_from_batch',
  pageNumbers: [index * 2 + 1, index * 2 + 2],
  status: 'grouped',
  answerSlots: [],
  warnings: [],
  updatedAt: '2026-07-21T00:00:00Z',
}));

const students: Student[] = [
  { id: 'student-1', displayName: 'Öğrenci A', number: '501', className: '11-B', warnings: [] },
  { id: 'student-2', displayName: null, number: null, className: null, warnings: [] },
];

test('student workspace normalizes tabs and rejects a batch from another selected class', () => {
  assert.equal(normalizeStudentOperationsTab('issues'), 'issues');
  assert.equal(normalizeStudentOperationsTab('unknown'), 'grouping');
  assert.deepEqual(
    resolveStudentOperationsSelection(classes, batches, classes[0]!.id, batches[1]!.id),
    { classId: classes[0]!.id, batchId: '' },
  );
  assert.deepEqual(
    resolveStudentOperationsSelection(classes, batches, null, batches[1]!.id),
    { classId: classes[1]!.id, batchId: batches[1]!.id },
  );
});

test('class and batch filters only expose submissions in the selected scope', () => {
  assert.deepEqual(filterStudentSubmissions(submissions, classes[0]!.id, ''), [submissions[0]]);
  assert.deepEqual(filterStudentSubmissions(submissions, '', batches[1]!.id), [submissions[1]]);
});

test('canonical class comes from the PDF batch relationship and mismatch stays a warning', () => {
  const project = { schoolClasses: classes, students };
  assert.equal(getSubmissionClassName(project, submissions[0]!), '11-A');
  assert.equal(hasIdentityClassMismatch(project, submissions[0]!, students[0]!.className), true);
  assert.equal(hasIdentityClassMismatch(project, submissions[0]!, '11 a'), false);
  assert.equal(getStudentTeacherLabel(students[0], '11-A'), 'Öğrenci A · No 501 · 11-A');
  assert.doesNotMatch(getStudentTeacherLabel(students[0], '11-A'), /class-internal|submission-/);
});

test('filename class suggestion is editable metadata, not an automatic assignment', () => {
  assert.equal(suggestSchoolClassFromFilename('11A.pdf', classes)?.id, classes[0]!.id);
  assert.equal(suggestSchoolClassFromFilename('11-C öğrenci sınavları.pdf', classes)?.id, classes[1]!.id);
  assert.equal(suggestSchoolClassFromFilename('karışık-taramalar.pdf', classes), null);

  const teacherSelection = resolveStudentOperationsSelection(classes, batches, classes[1]!.id, null);
  assert.equal(teacherSelection.classId, classes[1]!.id);
});

test('student PDF import stays blocked until both PDF and teacher-confirmed class exist', () => {
  assert.equal(getStudentBatchImportDisabledReason(null, ''), 'Önce PDF seçin.');
  assert.equal(getStudentBatchImportDisabledReason('/tmp/11A.pdf', ''), 'Önce sınıf seçin.');
  assert.equal(getStudentBatchImportDisabledReason('/tmp/11A.pdf', classes[1]!.id), undefined);
});

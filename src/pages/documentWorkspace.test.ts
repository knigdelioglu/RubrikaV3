/// <reference types="node" />

import assert from 'node:assert/strict';
import test from 'node:test';
import type { Document, PdfPreviewStatusSnapshot } from '../api/types.ts';
import {
  buildDocumentWorkspaceItems,
  createDocumentImportController,
  createPreviewStartController,
  getDocumentWorkspaceSummary,
  getWorkspacePreviewStatus,
  importWorkspaceDocument,
  resolveWorkspaceRole,
  startWorkspacePreview,
  type DocumentWorkspaceCommandGateway,
} from './documentWorkspace.ts';

const document = (role: Document['role'], overrides: Partial<Document> = {}): Document => ({
  id: `${role}-1`,
  role,
  fileName: `${role}.pdf`,
  storedPath: `/project/${role}.pdf`,
  pageCount: 4,
  addedAt: '2026-07-21T10:00:00Z',
  ...overrides,
});

function gateway(calls: string[]): DocumentWorkspaceCommandGateway {
  const imported = (role: Document['role']) => Promise.resolve(document(role));
  const started = () => Promise.resolve({ jobId: 'job-1', status: 'queued' as const });
  const status = (): Promise<PdfPreviewStatusSnapshot> => Promise.resolve({
    documentId: 'doc-1', status: 'ready', pageCount: 4, previewCount: 4, message: 'Hazır',
  });
  return {
    importExamSourcePdf: () => { calls.push('import_exam_source_pdf'); return imported('exam_source'); },
    importAnswerKeyPdf: () => { calls.push('import_answer_key_pdf'); return imported('answer_key'); },
    importStudentScanPdf: () => { calls.push('import_student_scan_pdf'); return imported('student_scan'); },
    startPdfPreviewRender: () => { calls.push('start_pdf_preview_render'); return started(); },
    startStudentScanPreviewRender: () => { calls.push('start_student_scan_preview_render'); return started(); },
    getPdfPreviewStatus: () => { calls.push('get_pdf_preview_status'); return status(); },
    getStudentScanPreviewStatus: () => { calls.push('get_student_scan_preview_status'); return status(); },
  };
}

test('workspace always exposes the three teacher document roles', () => {
  const items = buildDocumentWorkspaceItems([]);
  assert.deepEqual(items.map((item) => item.label), ['Sınav PDF’i', 'Cevap Anahtarı', 'Öğrenci Cevapları']);
  assert.ok(items.every((item) => item.uploadState === 'missing'));
});

test('workspace normalizes all backend preview states into teacher labels', () => {
  const items = buildDocumentWorkspaceItems([
    document('exam_source', { preview: { status: 'queued' } }),
    document('answer_key', { preview: { status: 'failed' } }),
    document('student_scan', { preview: { status: 'ready' } }),
  ]);
  assert.deepEqual(items.map((item) => item.previewLabel), [
    'Önizleme hazırlanıyor',
    'Önizleme oluşturulamadı',
    'İncelemeye hazır',
  ]);
  assert.deepEqual(getDocumentWorkspaceSummary(items, 1), {
    uploadedCount: 3,
    readyPreviewCount: 1,
    activePreviewCount: 1,
    failedPreviewCount: 1,
  });
});

test('deep links preserve document role selection including legacy document types', () => {
  assert.equal(resolveWorkspaceRole('exam', null), 'exam_source');
  assert.equal(resolveWorkspaceRole('student', null), 'student_scan');
  assert.equal(resolveWorkspaceRole(null, document('answer_key')), 'answer_key');
});

test('each role calls its existing upload command', async () => {
  const calls: string[] = [];
  const commands = gateway(calls);
  await importWorkspaceDocument(commands, 'exam_source', { projectId: 'p1', sourcePath: '/exam.pdf' });
  await importWorkspaceDocument(commands, 'answer_key', { projectId: 'p1', sourcePath: '/answer.pdf' });
  await importWorkspaceDocument(commands, 'student_scan', { projectId: 'p1', sourcePath: '/student.pdf' });
  assert.deepEqual(calls, ['import_exam_source_pdf', 'import_answer_key_pdf', 'import_student_scan_pdf']);
});

test('student preview keeps its dedicated command while exam and answer key use the generic command', async () => {
  const calls: string[] = [];
  const commands = gateway(calls);
  await startWorkspacePreview(commands, 'exam_source', { projectId: 'p1', documentId: 'exam' });
  await startWorkspacePreview(commands, 'answer_key', { projectId: 'p1', documentId: 'answer' });
  await startWorkspacePreview(commands, 'student_scan', { projectId: 'p1', documentId: 'student' });
  await getWorkspacePreviewStatus(commands, 'answer_key', { projectId: 'p1', documentId: 'answer' });
  await getWorkspacePreviewStatus(commands, 'student_scan', { projectId: 'p1', documentId: 'student' });
  assert.deepEqual(calls, [
    'start_pdf_preview_render',
    'start_pdf_preview_render',
    'start_student_scan_preview_render',
    'get_pdf_preview_status',
    'get_student_scan_preview_status',
  ]);
});

test('cancelled native selection does not import and pending selection cannot duplicate import', async () => {
  let selectCount = 0;
  let importCount = 0;
  const cancelled = createDocumentImportController(async () => null, async () => {
    importCount += 1;
    return document('exam_source');
  });
  assert.equal(await cancelled.run('exam_source'), null);
  assert.equal(importCount, 0);

  let resolveSelection: ((path: string | null) => void) | undefined;
  const controller = createDocumentImportController(
    () => {
      selectCount += 1;
      return new Promise<string | null>((resolve) => { resolveSelection = resolve; });
    },
    async (role) => {
      importCount += 1;
      return document(role);
    },
  );
  const first = controller.run('student_scan');
  const second = controller.run('student_scan');
  resolveSelection?.('/student.pdf');
  await Promise.all([first, second]);
  assert.equal(selectCount, 1);
  assert.equal(importCount, 1);
});

test('failed replacement leaves the existing document presentation intact', async () => {
  const existing = document('exam_source', { fileName: 'mevcut.pdf' });
  const controller = createDocumentImportController(
    async () => '/yeni.pdf',
    async () => { throw new Error('Yüklenemedi'); },
  );
  await assert.rejects(controller.run('exam_source'));
  assert.equal(buildDocumentWorkspaceItems([existing])[0]?.documentName, 'mevcut.pdf');
});

test('duplicate preview requests share one backend start', async () => {
  let calls = 0;
  let finish: ((value: { jobId: string; status: 'queued' }) => void) | undefined;
  const controller = createPreviewStartController(() => {
    calls += 1;
    return new Promise((resolve) => { finish = resolve; });
  });
  const first = controller.run('exam_source', 'doc-1');
  const second = controller.run('exam_source', 'doc-1');
  finish?.({ jobId: 'job-1', status: 'queued' });
  await Promise.all([first, second]);
  assert.equal(calls, 1);
});

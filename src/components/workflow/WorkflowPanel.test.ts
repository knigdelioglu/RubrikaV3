/// <reference types="node" />

import assert from 'node:assert/strict';
import test from 'node:test';
import type { WorkflowSnapshot } from '../../api/types';
import { getStudentAnswerOcrStartDisabledReason } from '../../pages/studentAnswerOcrUi.ts';
import { getWorkflowSummaryText } from './workflowSummary.ts';
import { describeStudentScanPreview, getPrimaryWorkflowAction } from './workflowUi.ts';
import { summarizeWorkflowAreas } from './projectOverview.ts';

test('getWorkflowSummaryText returns only the text field', () => {
  const workflow = {
    currentStage: 'documents_missing',
    currentStageLabel: 'Belgeler Eksik',
    blockingReasons: [],
    nextActions: [],
    summary: {
      text: 'Hazır',
      steps: [],
      readiness: {
        examPackageFreeze: false,
        studentIntake: false,
        scoring: false,
      },
    },
  } as WorkflowSnapshot;

  assert.equal(getWorkflowSummaryText(workflow), 'Hazır');
});

test('getWorkflowSummaryText hides empty or missing summary text', () => {
  assert.equal(
    getWorkflowSummaryText({
      currentStage: 'documents_missing',
      currentStageLabel: 'Belgeler Eksik',
      blockingReasons: [],
      nextActions: [],
      summary: {
        text: '   ',
        steps: [],
        readiness: {
          examPackageFreeze: false,
          studentIntake: false,
          scoring: false,
        },
      },
    } as WorkflowSnapshot),
    '',
  );
});

test('getPrimaryWorkflowAction prefers the first enabled action', () => {
  const action = getPrimaryWorkflowAction([
    { code: 'start_exam_package_build', label: 'Sınav Paketi Oluştur', enabled: false },
    { code: 'open_student_scans_page', label: 'Öğrenci PDF’leri sayfasını aç', enabled: true },
  ]);

  assert.equal(action?.code, 'open_student_scans_page');
  assert.equal(action?.label, 'Öğrenci PDF’leri sayfasını aç');
});

test('describeStudentScanPreview explains missing previews clearly', () => {
  assert.equal(
    describeStudentScanPreview({
      pageCount: 0,
      preview: { status: 'missing' },
    }),
    'Öğrenci PDF yüklendi, sayfa sayısı henüz bilinmiyor. Önizleme oluşturulduğunda sayfa sayısı hesaplanacak.',
  );
  assert.equal(
    describeStudentScanPreview({
      pageCount: 12,
      preview: { status: 'missing' },
    }),
    'Öğrenci PDF yüklendi, sayfa önizlemesi arka planda hazırlanıyor.',
  );
});

test('student answer OCR start helper blocks only non-ready stages', () => {
  assert.equal(
    getStudentAnswerOcrStartDisabledReason('question_text_missing'),
    'OCR başlatmak için workflow OCR hazır olmalı.',
  );
  assert.equal(
    getStudentAnswerOcrStartDisabledReason('ocr_ready'),
    undefined,
  );
});

test('overview area summaries use backend step statuses without project readiness rules', () => {
  const areas = summarizeWorkflowAreas([
    { code: 'pdf_preview_render', label: 'PDF', status: 'succeeded', message: 'Hazır' },
    { code: 'question_text_extraction', label: 'Sorular', status: 'partial', message: 'Kontrol bekliyor' },
    { code: 'rubric_pdf_import', label: 'Rubrik', status: 'succeeded', message: 'Hazır' },
    { code: 'student_answer_ocr', label: 'OCR', status: 'running', message: 'Çalışıyor', current: 5, total: 10 },
  ]);

  assert.equal(areas.find((area) => area.area === 'exam')?.status, 'partial');
  assert.equal(areas.find((area) => area.area === 'ocr')?.status, 'running');
  assert.equal(areas.find((area) => area.area === 'ocr')?.current, 5);
  assert.equal(areas.find((area) => area.area === 'grading')?.status, 'pending');
});

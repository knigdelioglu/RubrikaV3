/// <reference types="node" />

import assert from 'node:assert/strict';
import test from 'node:test';
import type { Question, StudentAnswerCropTemplateItem, StudentAnswerOcrRecord } from '../api/types';
import {
  getMissingStudentAnswerCropQuestionNumbers,
  getStudentAnswerCropTemplateSummary,
  applyStudentAnswerOcrSuggestedCorrection,
  getStudentAnswerOcrDraftText,
  getStudentAnswerOcrActionableIssueEntries,
  getStudentAnswerOcrActionableIssueEntriesForQuestion,
  getStudentAnswerOcrPreviewMessage,
  getStudentAnswerOcrPreviewMode,
  getStudentAnswerOcrRawOutput,
  getStudentAnswerOcrIssueCount,
  getStudentAnswerOcrIssueHighlightBoxes,
  getStudentAnswerOcrIssueKinds,
  getStudentAnswerOcrIssueSummary,
  getStudentAnswerOcrReviewedCount,
  getStudentAnswerOcrRerunVisible,
  getStudentAnswerOcrRerunConfirmMessage,
  getStudentAnswerOcrStartDisabledReasonWithHistory,
  getOcrPreprocessModeLabel,
  getStudentAnswerOcrPreprocessSummary,
  getStudentAnswerOcrPreprocessVariantRef,
  getStudentAnswerOcrIssueReviewModelInputRef,
  getStudentAnswerOcrTextHighlights,
  getStudentAnswerOcrTextHighlightsForQuestion,
  getStudentAnswerOcrUncertaintySummary,
  hasApprovedStudentAnswerOcrRecords,
} from './studentAnswerOcrUi.ts';

function baseRecord(overrides: Partial<StudentAnswerOcrRecord> = {}): StudentAnswerOcrRecord {
  return {
    id: 'ocr-1',
    submissionId: 'submission-1',
    questionId: 'q1',
    questionNumber: 1,
    sourcePageNumbers: [1],
    sourceImageRefs: [],
    cropRefs: [],
    originalCropRefs: [],
    preprocessedCropRefs: [],
    fullPagePreviewRefs: [],
    modelInputCropRef: null,
    answerText: 'Cevap',
    structuredAnswer: null,
    confidence: 0.9,
    uncertainSpans: [],
    suggestedCorrections: [],
    criticalTermWarnings: [],
    ocrSemanticWarnings: [],
    criticalKeywordUncertain: false,
    status: 'succeeded',
    needsReview: false,
    reviewReasons: [],
    warnings: [],
    modelName: 'gemma',
    promptVersion: 'student_answer_ocr_v1',
    preprocessVersion: 'ocr_image_preprocess_v2',
    createdAt: 'now',
    updatedAt: 'now',
    teacherCorrectedText: null,
    teacherReviewedAt: null,
    parseDiagnostics: null,
    renderDiagnostics: null,
    availablePreprocessVariants: ['original', 'clean_grayscale', 'handwriting_enhanced', 'high_contrast', 'high_contrast_bw'],
    ...overrides,
  };
}

function question(number: number): Question {
  return {
    id: `q${number}`,
    number,
    maxScore: 10,
    answerType: 'general_text',
    questionText: {
      value: `Soru ${number}`,
      source: 'manual',
      status: 'confirmed',
      warnings: [],
    },
    rubric: {
      status: 'confirmed',
      source: 'manual',
      maxScore: 10,
      expectedAnswer: null,
      criteria: [],
      partialCreditHints: [],
      zeroScoreConditions: [],
      commonMistakes: [],
      warnings: [],
    },
    cropTemplate: undefined,
  };
}

function questionWithExpectedAnswer(number: number, expectedAnswer: string): Question {
  const item = question(number);
  item.rubric.expectedAnswer = expectedAnswer;
  return item;
}

function templateItem(questionNumber: number): StudentAnswerCropTemplateItem {
  return {
    questionId: `q${questionNumber}`,
    questionNumber,
    pageIndexWithinSubmission: 0,
    bbox: { x: 0.1, y: 0.2, width: 0.3, height: 0.4, pageIndex: 0 },
    label: null,
    note: null,
  };
}

test('raw output helper exposes parse-failed model text', () => {
  const record = baseRecord({
    answerText: 'salvaged text',
    parseDiagnostics: {
      rawModelOutput: '```json\n{broken\n```',
      parseError: 'Öğrenci OCR JSON çıktısı çözülemedi.',
      parsedJson: null,
      salvagedAnswerText: 'salvaged text',
      parseStrategy: 'raw_text_salvage',
      modelRequestMetadata: null,
    },
    status: 'parse_failed',
    needsReview: true,
  });

  assert.equal(getStudentAnswerOcrRawOutput(record), '```json\n{broken\n```');
  assert.equal(getStudentAnswerOcrDraftText(record), 'salvaged text');
});

test('crop missing preview helper returns explicit diagnostics', () => {
  const record = baseRecord({
    status: 'crop_missing',
    needsReview: true,
    renderDiagnostics: {
      cropRefs: [],
      fullPagePreviewRefs: [],
      cropBBox: null,
      cropWidth: null,
      cropHeight: null,
      sourcePageCount: 1,
      answerRegionSource: 'crop_missing',
      questionRegionStart: 1,
      questionRegionEnd: 1,
      nextQuestionAnchor: 'q2',
      cropWasClamped: false,
      cropMarginApplied: false,
      renderedCropExists: false,
      renderedPagePreviewExists: false,
      cropMissing: true,
      pagePreviewMissing: true,
      partialAnswerSuspected: false,
      printedTextMixed: false,
      printedQuestionLeakDetected: false,
    },
  });

  assert.equal(getStudentAnswerOcrPreviewMode(record), 'missing');
  assert.match(getStudentAnswerOcrPreviewMessage(record), /crop_missing=true/);
});

test('fallback review helper warns about full page fallback', () => {
  const record = baseRecord({
    renderDiagnostics: {
      cropRefs: [],
      fullPagePreviewRefs: ['page-1.png', 'page-2.png'],
      cropBBox: null,
      cropWidth: null,
      cropHeight: null,
      sourcePageCount: 2,
      answerRegionSource: 'full_page_fallback_review_required',
      questionRegionStart: 1,
      questionRegionEnd: 2,
      nextQuestionAnchor: 'q2',
      cropWasClamped: false,
      cropMarginApplied: false,
      renderedCropExists: false,
      renderedPagePreviewExists: true,
      cropMissing: false,
      pagePreviewMissing: false,
      partialAnswerSuspected: true,
      printedTextMixed: false,
      printedQuestionLeakDetected: false,
    },
  });

  assert.match(getStudentAnswerOcrPreviewMessage(record), /Tam sayfa fallback/);
});

test('draft helper prefers teacher edits over salvaged text', () => {
  const record = baseRecord({
    answerText: 'model text',
    parseDiagnostics: {
      rawModelOutput: 'raw',
      parseError: null,
      parsedJson: null,
      salvagedAnswerText: 'salvaged text',
      parseStrategy: 'raw_text_salvage',
      modelRequestMetadata: null,
    },
    teacherCorrectedText: 'edited text',
  });

  assert.equal(getStudentAnswerOcrDraftText(record), 'edited text');
});

test('reviewed count only includes approved records', () => {
  assert.equal(
    getStudentAnswerOcrReviewedCount([
      baseRecord({ status: 'teacher_approved' }),
      baseRecord({ status: 'teacher_corrected' }),
      baseRecord({ status: 'succeeded' }),
    ]),
    1,
  );
});

test('rerun button only appears when review-needed records exist and no job is active', () => {
  assert.equal(getStudentAnswerOcrRerunVisible([{ code: 'rerun_student_answer_ocr', enabled: true }], false), true);
  assert.equal(getStudentAnswerOcrRerunVisible([{ code: 'rerun_student_answer_ocr', enabled: true }], true), false);
  assert.equal(getStudentAnswerOcrRerunVisible([{ code: 'open_student_answer_ocr_page', enabled: true }], false), false);
});

test('approved OCR records keep the start action available for rerun', () => {
  assert.equal(getStudentAnswerOcrStartDisabledReasonWithHistory('review_required', 0), 'OCR başlatmak için workflow OCR hazır olmalı.');
  assert.equal(getStudentAnswerOcrStartDisabledReasonWithHistory('review_required', 1), undefined);
  assert.equal(hasApprovedStudentAnswerOcrRecords([baseRecord({ status: 'teacher_approved' })]), true);
  assert.equal(hasApprovedStudentAnswerOcrRecords([baseRecord({ status: 'succeeded' })]), false);
  assert.match(getStudentAnswerOcrRerunConfirmMessage(true), /onaylı/i);
  assert.match(getStudentAnswerOcrRerunConfirmMessage(false), /mevcut sonuç korunacak/i);
});

test('crop template summary counts project questions only', () => {
  assert.equal(
    getStudentAnswerCropTemplateSummary([question(1), question(2), question(3)], [templateItem(1), templateItem(3)]),
    '2/3 soru için crop var',
  );
});

test('crop template helper lists missing question numbers', () => {
  assert.deepEqual(
    getMissingStudentAnswerCropQuestionNumbers([question(1), question(2), question(3)], [templateItem(2)]),
    [1, 3],
  );
});

test('issue review helper prefers the direct model input crop ref', () => {
  const record = baseRecord({
    modelInputCropRef: 'model-input.png',
    originalCropRefs: ['original.png'],
    cropRefs: ['crop.png'],
    preprocessedCropRefs: ['preprocessed.png'],
    fullPagePreviewRefs: ['page.png'],
  });

  assert.equal(getStudentAnswerOcrIssueReviewModelInputRef(record), 'model-input.png');
});

test('issue helpers capture only actionable OCR signals and highlights', () => {
  const record = baseRecord({
    needsReview: true,
    criticalKeywordUncertain: true,
    uncertainSpans: [
      {
        text: 'çelişen',
        start: 0,
        end: 8,
        alternatives: ['gelişen'],
        confidence: 0.41,
        reason: 'handwriting_ambiguity',
        highlightRegion: { x: 0.1, y: 0.2, width: 0.3, height: 0.1, pageIndex: 0 },
      },
    ],
    suggestedCorrections: [
      {
        originalText: 'çelişen',
        suggestedText: 'gelişen',
        reason: 'near_match',
        confidence: 0.41,
        applied: false,
        highlightRegion: { x: 0.2, y: 0.3, width: 0.2, height: 0.1, pageIndex: 0 },
      },
    ],
    criticalTermWarnings: [
      {
        observedText: 'çelişen sözcük kullanımı',
        expectedOrRelatedTerm: 'gelişen sözcük kullanımı',
        reason: 'semantic_confusion',
        warningCode: 'critical_keyword_ocr_uncertain',
        highlightRegion: { x: 0.15, y: 0.35, width: 0.4, height: 0.12, pageIndex: 0 },
      },
    ],
    preprocessWarnings: ['preprocess_failed'],
    warnings: ['ocr_parse_failed'],
    parseDiagnostics: {
      rawModelOutput: '{broken',
      parseError: 'json parse failed',
      parsedJson: null,
      salvagedAnswerText: 'çelişen sözcük kullanımı',
      parseStrategy: 'raw_text_salvage',
      modelRequestMetadata: null,
    },
    renderDiagnostics: {
      cropRefs: [],
      fullPagePreviewRefs: [],
      cropBBox: null,
      cropWidth: null,
      cropHeight: null,
      sourcePageCount: 1,
      answerRegionSource: 'crop_missing',
      questionRegionStart: 1,
      questionRegionEnd: 1,
      nextQuestionAnchor: 'q2',
      cropWasClamped: false,
      cropMarginApplied: false,
      renderedCropExists: false,
      renderedPagePreviewExists: false,
      cropMissing: true,
      pagePreviewMissing: true,
      partialAnswerSuspected: true,
      printedTextMixed: false,
      printedQuestionLeakDetected: true,
    },
  });

  assert.deepEqual(getStudentAnswerOcrIssueKinds(record), [
    'uncertain_span',
    'critical_term_uncertain',
    'ocr_low_confidence',
    'suggested_correction',
    'critical_term_warning',
    'parse_warning',
  ]);
  assert.equal(getStudentAnswerOcrIssueCount(record), 4);
  assert.match(getStudentAnswerOcrIssueSummary(record), /Şüpheli ifade/);
  assert.match(getStudentAnswerOcrIssueSummary(record), /Öneri/);
  assert.equal(getStudentAnswerOcrIssueHighlightBoxes(record).length, 3);
  assert.equal(getStudentAnswerOcrActionableIssueEntries(record).some((entry) => entry.kind === 'answer_crop_may_be_truncated'), false);
  assert.equal(getStudentAnswerOcrTextHighlights(record, 'çelişen sözcük kullanımı').length > 0, true);
});

test('partial answer warning does not create an actionable issue', () => {
  const record = baseRecord({
    needsReview: true,
    renderDiagnostics: {
      cropRefs: [],
      fullPagePreviewRefs: [],
      cropBBox: null,
      cropWidth: null,
      cropHeight: null,
      sourcePageCount: 1,
      answerRegionSource: 'crop_missing',
      questionRegionStart: 1,
      questionRegionEnd: 1,
      nextQuestionAnchor: 'q2',
      cropWasClamped: false,
      cropMarginApplied: false,
      renderedCropExists: false,
      renderedPagePreviewExists: false,
      cropMissing: true,
      pagePreviewMissing: true,
      partialAnswerSuspected: true,
      printedTextMixed: false,
      printedQuestionLeakDetected: false,
    },
  });

  assert.equal(getStudentAnswerOcrActionableIssueEntries(record).length, 0);
});

test('critical keyword uncertainty with no structured fields still creates a concrete issue card', () => {
  const record = baseRecord({
    answerText: '2. Anlatım Bozukluğunun Nedeni: gelşeqiz sözcük kullanımı',
    criticalKeywordUncertain: true,
    needsReview: true,
    ocrSemanticWarnings: ['critical_keyword_ocr_uncertain'],
  });
  const questionItem = questionWithExpectedAnswer(5, 'çelişen sözcük kullanımı');

  const entries = getStudentAnswerOcrActionableIssueEntriesForQuestion(record, questionItem);

  assert.equal(entries.length, 1);
  assert.equal(entries[0]?.kind, 'critical_keyword_uncertain');
  assert.equal(entries[0]?.originalText, 'gelşeqiz');
  assert.equal(entries[0]?.suggestionText, 'çelişen');
  assert.equal(getStudentAnswerOcrTextHighlightsForQuestion(record, record.answerText, questionItem).length, 1);
});

test('critical keyword uncertainty derives a concrete issue from answer text and expected answer', () => {
  const record = baseRecord({
    answerText: 'gelşeqiz sözcük kullanımı',
    criticalKeywordUncertain: true,
    needsReview: true,
    ocrSemanticWarnings: ['Öğrencinin yazdığı "gelşeqiz" kelimesi OCR’da belirsiz okunmuş olabilir.'],
  });
  const questionItem = questionWithExpectedAnswer(5, 'çelişen sözcük kullanımı');

  const entries = getStudentAnswerOcrActionableIssueEntriesForQuestion(record, questionItem);

  assert.equal(entries.length, 1);
  assert.equal(entries[0]?.kind, 'critical_keyword_uncertain');
  assert.equal(entries[0]?.originalText, 'gelşeqiz');
  assert.equal(entries[0]?.suggestionText, 'çelişen');
  assert.equal(getStudentAnswerOcrTextHighlightsForQuestion(record, record.answerText, questionItem).length > 0, true);
});

test('warning-only crop notes do not become actionable issues', () => {
  const record = baseRecord({
    warnings: ['answer_crop_may_be_incomplete'],
  });

  assert.equal(getStudentAnswerOcrActionableIssueEntries(record).length, 0);
  assert.deepEqual(getStudentAnswerOcrIssueKinds(record), []);
});

test('semantic warning does not create an actionable issue without critical uncertainty', () => {
  const record = baseRecord({
    answerText: 'gelşeqiz sözcük kullanımı',
    ocrSemanticWarnings: ["Öğrencinin yazdığı 'gelşeqiz' kelimesi anlamsızdır ve düzeltme gerektirir."],
  });

  assert.equal(getStudentAnswerOcrActionableIssueEntries(record).length, 0);
});

test('preprocess and truncation warnings do not create actionable issue cards alone', () => {
  const record = baseRecord({
    preprocessWarnings: ['preprocess_failed'],
    renderDiagnostics: {
      cropRefs: [],
      fullPagePreviewRefs: [],
      cropBBox: null,
      cropWidth: null,
      cropHeight: null,
      sourcePageCount: 1,
      answerRegionSource: 'crop_missing',
      questionRegionStart: 1,
      questionRegionEnd: 1,
      nextQuestionAnchor: 'q2',
      cropWasClamped: false,
      cropMarginApplied: false,
      renderedCropExists: false,
      renderedPagePreviewExists: false,
      cropMissing: true,
      pagePreviewMissing: true,
      partialAnswerSuspected: true,
      printedTextMixed: false,
      printedQuestionLeakDetected: false,
    },
  });

  assert.equal(getStudentAnswerOcrActionableIssueEntries(record).length, 0);
});

test('suggestion helper replaces only the matching phrase', () => {
  const result = applyStudentAnswerOcrSuggestedCorrection('gelşeqiz sözcük kullanımı', {
    originalText: 'gelşeqiz',
    suggestedText: 'çelişen',
  });

  assert.equal(result.applied, true);
  assert.equal(result.text, 'çelişen sözcük kullanımı');
});

test('fuzzy text highlight avoids false positives', () => {
  const record = baseRecord({
    uncertainSpans: [
      {
        text: 'gelşeqiz',
        alternatives: ['çelişen'],
        confidence: 0.41,
        reason: 'handwriting_ambiguity',
      },
    ],
  });

  assert.equal(getStudentAnswerOcrTextHighlights(record, 'metinde aranan ifade yok').length, 0);
});

test('preview helper does not surface generic truncation wording for partial answer flags alone', () => {
  const record = baseRecord({
    renderDiagnostics: {
      cropRefs: [],
      fullPagePreviewRefs: [],
      cropBBox: null,
      cropWidth: null,
      cropHeight: null,
      sourcePageCount: 1,
      answerRegionSource: 'crop_missing',
      questionRegionStart: 1,
      questionRegionEnd: 1,
      nextQuestionAnchor: 'q2',
      cropWasClamped: false,
      cropMarginApplied: false,
      renderedCropExists: false,
      renderedPagePreviewExists: false,
      cropMissing: false,
      pagePreviewMissing: false,
      partialAnswerSuspected: true,
      printedTextMixed: false,
      printedQuestionLeakDetected: false,
    },
  });

  assert.doesNotMatch(getStudentAnswerOcrPreviewMessage(record), /kırpım/i);
});

test('preprocess summary reflects the selected mode and warnings', () => {
  const record = baseRecord({
    preprocessMode: 'handwriting_enhanced',
    preprocessApplied: true,
    preprocessWarnings: ['preprocess_failed'],
    originalCropRefs: ['original.png'],
    preprocessedCropRefs: ['clean.png'],
    preprocessDiagnostics: [
      {
        mode: 'original',
        preprocessVersion: 'ocr_image_preprocess_v2',
        sourceImagePath: 'original.png',
        outputImagePath: 'original.png',
        sourceWidth: 1,
        sourceHeight: 1,
        outputWidth: 1,
        outputHeight: 1,
        sourceBytes: 1,
        outputBytes: 1,
        cacheHit: false,
        applied: false,
        warnings: [],
        errorMessage: null,
        technicalDetails: null,
      },
      {
        mode: 'handwriting_enhanced',
        preprocessVersion: 'ocr_image_preprocess_v2',
        sourceImagePath: 'original.png',
        outputImagePath: 'handwriting.png',
        sourceWidth: 1,
        sourceHeight: 1,
        outputWidth: 1,
        outputHeight: 1,
        sourceBytes: 1,
        outputBytes: 1,
        cacheHit: false,
        applied: true,
        warnings: [],
        errorMessage: null,
        technicalDetails: null,
      },
    ],
  });

  assert.equal(getOcrPreprocessModeLabel('high_contrast_bw_optional'), 'Siyah-beyaz alternatif');
  assert.match(getStudentAnswerOcrPreprocessSummary(record), /El yazısı güçlendirildi/);
  assert.match(getStudentAnswerOcrPreprocessSummary(record), /model için kullanıldı/);
  assert.match(getStudentAnswerOcrPreprocessSummary(record), /preprocess_failed/);
});

test('preprocess variant ref helper resolves variant paths', () => {
  const record = baseRecord({
    originalCropRefs: ['original.png'],
    preprocessDiagnostics: [
      {
        mode: 'clean_grayscale',
        preprocessVersion: 'ocr_image_preprocess_v2',
        sourceImagePath: 'original.png',
        outputImagePath: 'clean.png',
        sourceWidth: 1,
        sourceHeight: 1,
        outputWidth: 1,
        outputHeight: 1,
        sourceBytes: 1,
        outputBytes: 1,
        cacheHit: false,
        applied: true,
        warnings: [],
        errorMessage: null,
        technicalDetails: null,
      },
    ],
  });

  assert.equal(getStudentAnswerOcrPreprocessVariantRef(record, 'clean_grayscale'), 'clean.png');
  assert.equal(getStudentAnswerOcrPreprocessVariantRef(record, 'original'), 'original.png');
});

test('uncertainty summary reflects critical term metadata', () => {
  const record = baseRecord({
    criticalKeywordUncertain: true,
    uncertainSpans: [
      {
        text: 'çelişen',
        start: 0,
        end: 8,
        alternatives: ['gelişen'],
        confidence: 0.41,
        reason: 'handwriting_ambiguity',
      },
    ],
    suggestedCorrections: [
      {
        originalText: 'çelişen',
        suggestedText: 'gelişen',
        reason: 'near_match',
        confidence: 0.41,
        applied: false,
      },
    ],
    criticalTermWarnings: [
      {
        observedText: 'çelişen sözcük kullanımı',
        expectedOrRelatedTerm: 'gelişen sözcük kullanımı',
        reason: 'semantic_confusion',
        warningCode: 'ocr_critical_keyword_uncertain',
      },
    ],
    ocrSemanticWarnings: ['ocr_critical_keyword_uncertain'],
  });

  assert.match(getStudentAnswerOcrUncertaintySummary(record), /Kritik terim belirsizliği/);
  assert.match(getStudentAnswerOcrUncertaintySummary(record), /1 belirsiz alan/);
  assert.match(getStudentAnswerOcrUncertaintySummary(record), /1 önerili düzeltme/);
});

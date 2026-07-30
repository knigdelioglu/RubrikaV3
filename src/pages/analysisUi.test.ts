/// <reference types="node" />

import assert from 'node:assert/strict';
import test from 'node:test';

import type { AssessmentAnalysis } from '../api/types.ts';
import {
  analysisStatusLabel,
  clampAnalysisPercentage,
  latestAnalysisId,
  percentageLabel,
} from './analysisUi.ts';

function analysis(id: string, kind: 'written' | 'speaking'): AssessmentAnalysis {
  return {
    id,
    projectId: 'project-1',
    kind,
    title: 'Sınav',
    status: 'ready',
    studentCount: 0,
    criteria: [],
    students: [],
    scoreBands: [],
    createdAt: '2026-07-28T00:00:00Z',
  };
}

test('analysis percentages are bounded before rendering', () => {
  assert.equal(clampAnalysisPercentage(-5), 0);
  assert.equal(clampAnalysisPercentage(108), 100);
  assert.equal(clampAnalysisPercentage(Number.NaN), 0);
  assert.equal(percentageLabel(84.6), '%85');
});

test('latest analysis selection never crosses written and speaking modes', () => {
  const analyses = [analysis('speaking-new', 'speaking'), analysis('written-new', 'written')];

  assert.equal(latestAnalysisId(analyses, 'written'), 'written-new');
  assert.equal(latestAnalysisId(analyses, 'speaking'), 'speaking-new');
});

test('analysis states use teacher-facing labels', () => {
  assert.equal(analysisStatusLabel('generating'), 'Gemma raporu hazırlanıyor');
  assert.equal(analysisStatusLabel('partial'), 'grafikler hazır, rapor kısmi');
  assert.equal(analysisStatusLabel('failed'), 'analiz tamamlanamadı');
});

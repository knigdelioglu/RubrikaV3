import assert from 'node:assert/strict';
import test from 'node:test';
import { buildPerformanceCsv } from './performanceReportUi.ts';
import type { PerformanceReport } from '../api/types';

function reportWithStudentName(studentName: string): PerformanceReport {
  return {
    taskTitle: '1. Performans Görevi',
    courseName: 'Türk Dili ve Edebiyatı',
    gradeLevel: 9,
    term: 1,
    sequenceNumber: 1,
    className: '9A',
    rubricId: 'rubric-1',
    rubricName: 'Yazılı Ürün Rubriği',
    rubricVersion: 1,
    criteria: [
      {
        id: 'c1',
        name: 'Metne uygunluk',
        description: 'Metne uygunluk açıklaması',
        levelDescriptions: [],
      },
    ],
    levels: [{ id: 'l1', name: 'Çok iyi', points: 5, description: 'Çok iyi tanımı' }],
    maxPoints: 5,
    generatedAt: '2026-01-01T00:00:00Z',
    summary: {
      studentCount: 1,
      assessedCount: 1,
      approvedCount: 1,
      missingCount: 0,
      notPerformedCount: 0,
      unratedCount: 0,
    },
    rows: [
      {
        studentId: 'student-1',
        studentName,
        studentNumber: '1',
        status: 'approved',
        criterionScores: [
          {
            criterionId: 'c1',
            criterionName: 'Metne uygunluk',
            levelId: 'l1',
            levelName: 'Çok iyi',
            points: 5,
          },
        ],
        total: 5,
        feedback: null,
      },
    ],
  };
}

function studentNameCell(report: PerformanceReport): string {
  const csv = buildPerformanceCsv(report).replace(/^\uFEFF/, '');
  const lines = csv.split('\r\n').filter((line) => line.length > 0);
  const row = lines[1];
  return row.split(';')[1];
}

test('CSV output prevents formula injection through user-controlled student names', () => {
  const payloads = ['=HYPERLINK("http://evil.example","tık")', '+SUM(1,1)', '-1+2', '@cmd'];
  for (const payload of payloads) {
    const cell = studentNameCell(reportWithStudentName(payload));
    assert.ok(
      cell.startsWith("'"),
      `student name "${payload}" must be escaped with a leading apostrophe, got "${cell}"`,
    );
  }
});

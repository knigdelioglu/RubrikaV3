import test from 'node:test';
import assert from 'node:assert/strict';

import { teacherFacingRubricWarnings } from './rubricWarnings.ts';

test('teacherFacingRubricWarnings hides technical alias and empty-content warnings', () => {
  assert.deepEqual(
    teacherFacingRubricWarnings([
      'maxPoints_alias:max_points',
      'rubric_empty_content',
      'Rubrik boş geldi.',
      'Beklenen cevap çıkarılamadı.',
    ]),
    ['Rubrik boş geldi.', 'Beklenen cevap çıkarılamadı.'],
  );
});

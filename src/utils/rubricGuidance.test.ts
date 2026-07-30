import test from 'node:test';
import assert from 'node:assert/strict';

import { optionalGuidanceEmptyText, optionalGuidanceText } from './rubricGuidance.ts';

test('optional guidance fields are described as later enrichment, not errors', () => {
  assert.match(optionalGuidanceText, /opsiyoneldir/);
  assert.match(optionalGuidanceEmptyText('partialCreditHints'), /öneri olarak üretilebilir/);
  assert.match(optionalGuidanceEmptyText('zeroScoreConditions'), /öneri olarak üretilebilir/);
  assert.match(optionalGuidanceEmptyText('commonMistakes'), /sonra çıkarılabilir/);
});

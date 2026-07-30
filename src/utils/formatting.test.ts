/// <reference types="node" />

import assert from 'node:assert/strict';
import test from 'node:test';

import { formatPageRange } from './formatting.ts';

test('formatPageRange collapses consecutive pages into ranges', () => {
  assert.equal(formatPageRange([1, 2, 3, 5, 6, 8]), '1-3, 5-6, 8');
});

test('formatPageRange keeps a single page as-is', () => {
  assert.equal(formatPageRange([4]), '4');
});

test('formatPageRange returns a dash for empty input', () => {
  assert.equal(formatPageRange([]), '-');
});

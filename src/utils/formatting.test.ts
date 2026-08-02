/// <reference types="node" />

import assert from 'node:assert/strict';
import test from 'node:test';

import { formatDate, formatDateTime, formatPageRange } from './formatting.ts';

test('formatDateTime accepts Rust RFC3339 timestamps with microseconds', () => {
  const formatted = formatDateTime('2026-08-02T20:34:29.902921+00:00');

  assert.ok(formatted);
  assert.notEqual(formatted, null);
});

test('invalid timestamps do not throw during project-shell rendering', () => {
  assert.equal(formatDateTime('not-a-date'), null);
  assert.equal(formatDate('not-a-date'), 'Tarih bilinmiyor');
});

test('formatPageRange collapses consecutive pages into ranges', () => {
  assert.equal(formatPageRange([1, 2, 3, 5, 6, 8]), '1-3, 5-6, 8');
});

test('formatPageRange keeps a single page as-is', () => {
  assert.equal(formatPageRange([4]), '4');
});

test('formatPageRange returns a dash for empty input', () => {
  assert.equal(formatPageRange([]), '-');
});

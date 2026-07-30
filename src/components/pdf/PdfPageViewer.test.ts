import test from 'node:test';
import assert from 'node:assert/strict';
import { resolveImageSrc } from './resolveImageSrc.ts';

test('resolveImageSrc keeps already-resolved urls intact', () => {
  assert.equal(resolveImageSrc('asset://localhost/tmp/page.png'), 'asset://localhost/tmp/page.png');
  assert.equal(resolveImageSrc('https://example.com/page.png'), 'https://example.com/page.png');
});

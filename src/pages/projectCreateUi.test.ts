import assert from 'node:assert/strict';
import test from 'node:test';
import {
  DEFAULT_COURSE_ID,
  DEFAULT_COURSE_NAME,
  getDefaultAcademicYear,
  getDefaultProjectPathQueryConfig,
} from './projectCreateUi.ts';

test('default academic year changes on July 1', () => {
  assert.equal(getDefaultAcademicYear(new Date(2026, 5, 30)), '2025-2026');
  assert.equal(getDefaultAcademicYear(new Date(2026, 6, 1)), '2026-2027');
  assert.equal(getDefaultAcademicYear(new Date(2026, 7, 2)), '2026-2027');
});

test('new project form defaults to the Turkish literature course', () => {
  assert.equal(DEFAULT_COURSE_ID, 'tde');
  assert.equal(DEFAULT_COURSE_NAME, 'Türk Dili ve Edebiyatı');
});

test('default project path query is scoped by academic year', () => {
  const config = getDefaultProjectPathQueryConfig('11. edebiyat 1. Yazılı', '2026-2027');

  assert.deepEqual(config.queryKey, ['default-project-path', '11. edebiyat 1. Yazılı', '2026-2027']);
  assert.equal(config.enabled, true);
});

test('default project path query waits for both project name and academic year', () => {
  assert.equal(getDefaultProjectPathQueryConfig('11. edebiyat 1. Yazılı', '').enabled, false);
  assert.equal(getDefaultProjectPathQueryConfig('', '2026-2027').enabled, false);
});

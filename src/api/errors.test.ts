/// <reference types="node" />

import assert from 'node:assert/strict';
import test from 'node:test';
import {
  isAppError,
  isProjectConflictError,
  isProjectMigrationRequiredError,
  normalizeAppError,
  type AppError,
} from './errors.ts';

test('project conflicts are safe to expose as refresh actions', () => {
  assert.equal(isProjectConflictError({ code: 'PROJECT_REVISION_CONFLICT' }), true);
  assert.equal(isProjectConflictError({ code: 'PROJECT_EXTERNALLY_MODIFIED' }), true);
  assert.equal(isProjectConflictError({ code: 'PROJECT_SAVE_FAILED' }), false);
  assert.equal(isProjectConflictError(undefined), false);
});

test('legacy project opens expose an explicit migration action', () => {
  assert.equal(isProjectMigrationRequiredError({ code: 'PROJECT_MIGRATION_REQUIRED' }), true);
  assert.equal(isProjectMigrationRequiredError({ code: 'PROJECT_LOAD_FAILED' }), false);
  assert.equal(isProjectMigrationRequiredError(undefined), false);
});

test('isAppError rejects fabricated error values', () => {
  assert.equal(isAppError({ code: 123, safeMessage: 'sayı kod' }), false);
  assert.equal(isAppError({ code: 'X' }), false);
  assert.equal(isAppError(null), false);
  assert.equal(isAppError('bir hata metni'), false);
  assert.equal(isAppError(undefined), false);
  assert.equal(isAppError(42), false);
  assert.equal(isAppError({ safeMessage: 'msg yok code' }), false);
  assert.equal(isAppError({ code: 'X', safeMessage: 123 }), false);
  assert.equal(isAppError({ code: 'X', safeMessage: 'ok', recoveryAction: 7 }), false);
});

test('isAppError accepts a real AppError shape', () => {
  assert.equal(isAppError({ code: 'UNKNOWN_ERROR', safeMessage: 'Genel hata' }), true);
  assert.equal(
    isAppError({ code: 'PROJECT_NOT_FOUND', safeMessage: 'Bulunamadı', recoveryAction: 'Tekrar dene' }),
    true,
  );
  assert.equal(
    isAppError({
      code: 'OCR_FAILED',
      safeMessage: 'OCR başarısız',
      recoveryAction: 'Yeniden çalıştır',
      correlationId: 'c-1',
      retryable: true,
      detailsAvailable: false,
    }),
    true,
  );
});

test('normalizeAppError drops non-conforming values to UNKNOWN_ERROR', () => {
  const normalized = normalizeAppError({ code: 123, safeMessage: 'x' });
  assert.equal(normalized.code, 'UNKNOWN_ERROR');
  assert.equal(typeof normalized.safeMessage, 'string');
  assert.equal(normalized.safeMessage.length > 0, true);
  assert.equal(typeof normalized.correlationId, 'string');
  assert.equal(normalized.retryable, false);

  const fromNull = normalizeAppError(null);
  assert.equal(fromNull.code, 'UNKNOWN_ERROR');
});

test('normalizeAppError passes through a valid AppError unchanged', () => {
  const valid: AppError = {
    code: 'PROJECT_NOT_FOUND',
    safeMessage: 'Proje bulunamadı',
    recoveryAction: 'Projeleri yeniden açın',
    correlationId: 'c-1',
    retryable: false,
    detailsAvailable: false,
  };
  assert.equal(normalizeAppError(valid), valid);
});

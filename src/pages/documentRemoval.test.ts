/// <reference types="node" />

import assert from 'node:assert/strict';
import test from 'node:test';
import { createDocumentRemovalController } from './documentRemoval.ts';

test('cancelling document deletion does not call the backend', async () => {
  const removedIds: string[] = [];
  const controller = createDocumentRemovalController(async (documentId) => {
    removedIds.push(documentId);
  });

  controller.selectDocument('document-1');
  controller.cancelSelection();

  assert.equal(await controller.confirmSelection(), false);
  assert.deepEqual(removedIds, []);
});

test('confirming document deletion calls the backend only once', async () => {
  const removedIds: string[] = [];
  let finishRemoval: (() => void) | undefined;
  const controller = createDocumentRemovalController((documentId) => {
    removedIds.push(documentId);
    return new Promise<void>((resolve) => {
      finishRemoval = resolve;
    });
  });

  controller.selectDocument('document-1');
  const firstConfirmation = controller.confirmSelection();
  const secondConfirmation = controller.confirmSelection();

  assert.deepEqual(removedIds, ['document-1']);
  finishRemoval?.();
  assert.equal(await firstConfirmation, true);
  assert.equal(await secondConfirmation, true);
  assert.equal(controller.getSelectedDocumentId(), null);
});

test('failed document deletion keeps the selected document available for retry', async () => {
  const visibleDocumentIds = ['document-1', 'document-2'];
  const removalError = new Error('Belge silinemedi');
  const controller = createDocumentRemovalController(async () => {
    throw removalError;
  });

  controller.selectDocument('document-1');

  await assert.rejects(controller.confirmSelection(), removalError);
  assert.equal(controller.getSelectedDocumentId(), 'document-1');
  assert.deepEqual(visibleDocumentIds, ['document-1', 'document-2']);
});

test('successful document deletion leaves its workspace role in the empty state', async () => {
  const visibleDocumentIds = ['document-1'];
  const controller = createDocumentRemovalController(async (documentId) => {
    const index = visibleDocumentIds.indexOf(documentId);
    if (index >= 0) visibleDocumentIds.splice(index, 1);
  });

  controller.selectDocument('document-1');
  assert.equal(await controller.confirmSelection(), true);
  assert.deepEqual(visibleDocumentIds, []);
  assert.equal(controller.getSelectedDocumentId(), null);
});

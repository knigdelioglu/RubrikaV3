export type RemoveDocumentCommand = (documentId: string) => Promise<void>;

export type DocumentRemovalController = {
  selectDocument: (documentId: string) => void;
  cancelSelection: () => void;
  getSelectedDocumentId: () => string | null;
  confirmSelection: () => Promise<boolean>;
};

export function createDocumentRemovalController(
  removeDocument: RemoveDocumentCommand,
): DocumentRemovalController {
  let selectedDocumentId: string | null = null;
  let pendingRemoval: Promise<boolean> | undefined;

  return {
    selectDocument(documentId) {
      selectedDocumentId = documentId;
    },
    cancelSelection() {
      if (!pendingRemoval) {
        selectedDocumentId = null;
      }
    },
    getSelectedDocumentId() {
      return selectedDocumentId;
    },
    confirmSelection() {
      if (pendingRemoval) {
        return pendingRemoval;
      }

      const documentId = selectedDocumentId;
      if (!documentId) {
        return Promise.resolve(false);
      }

      const removal = removeDocument(documentId)
        .then(() => {
          if (selectedDocumentId === documentId) {
            selectedDocumentId = null;
          }
          return true;
        })
        .finally(() => {
          pendingRemoval = undefined;
        });

      pendingRemoval = removal;
      return removal;
    },
  };
}

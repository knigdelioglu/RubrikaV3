import type {
  Document,
  DocumentRole,
  PdfPreviewStatus,
  PdfPreviewStatusSnapshot,
  StartPdfPreviewRenderOutput,
} from '../api/types';

export const workspaceDocumentRoles = [
  'exam_source',
  'answer_key',
  'student_scan',
] as const;

export type WorkspaceDocumentRole = (typeof workspaceDocumentRoles)[number];
export type WorkspacePreviewState = 'not_started' | 'queued' | 'running' | 'ready' | 'failed';

export type DocumentWorkspaceItem = {
  role: WorkspaceDocumentRole;
  label: string;
  description: string;
  purpose: string;
  dialogLabel: string;
  deleteImpact: string;
  document: Document | null;
  documentName?: string;
  pageCount?: number;
  uploadState: 'missing' | 'ready';
  previewState: WorkspacePreviewState;
  previewLabel: string;
  canUpload: boolean;
  canDelete: boolean;
};

export type DocumentWorkspaceSummary = {
  uploadedCount: number;
  readyPreviewCount: number;
  activePreviewCount: number;
  failedPreviewCount: number;
};

export type DocumentWorkspaceCommandGateway = {
  importExamSourcePdf: (input: { projectId: string; sourcePath: string }) => Promise<Document>;
  importAnswerKeyPdf: (input: { projectId: string; sourcePath: string }) => Promise<Document>;
  importStudentScanPdf: (input: { projectId: string; sourcePath: string }) => Promise<Document>;
  startPdfPreviewRender: (input: { projectId: string; documentId: string }) => Promise<StartPdfPreviewRenderOutput>;
  startStudentScanPreviewRender: (input: { projectId: string; documentId: string }) => Promise<StartPdfPreviewRenderOutput>;
  getPdfPreviewStatus: (input: { projectId: string; documentId: string }) => Promise<PdfPreviewStatusSnapshot>;
  getStudentScanPreviewStatus: (input: { projectId: string; documentId: string }) => Promise<PdfPreviewStatusSnapshot>;
};

const roleDetails: Record<WorkspaceDocumentRole, Omit<DocumentWorkspaceItem,
  | 'document'
  | 'documentName'
  | 'pageCount'
  | 'uploadState'
  | 'previewState'
  | 'previewLabel'
  | 'canUpload'
  | 'canDelete'
>> = {
  exam_source: {
    role: 'exam_source',
    label: 'Sınav PDF’i',
    description: 'Soruların bulunduğu boş sınav belgesi',
    purpose: 'Soru metinlerini ve sayfa düzenini kontrol etmek için sınavın özgün PDF dosyasını yükleyin.',
    dialogLabel: 'Sınav PDF',
    deleteImpact: 'Bu belgeye bağlı önizlemeler ve soru hazırlıkları geçersiz hâle gelebilir.',
  },
  answer_key: {
    role: 'answer_key',
    label: 'Cevap Anahtarı',
    description: 'Çözüm ve puanlama ölçütlerini içeren belge',
    purpose: 'Beklenen cevapları ve puanlama ölçütlerini incelemek için cevap anahtarı veya rubrik PDF’ini yükleyin.',
    dialogLabel: 'Cevap Anahtarı PDF',
    deleteImpact: 'Bu belgeye bağlı rubrik hazırlıkları ve önizlemeler geçersiz hâle gelebilir.',
  },
  student_scan: {
    role: 'student_scan',
    label: 'Öğrenci Cevapları',
    description: 'Öğrencilerin doldurduğu toplu tarama',
    purpose: 'Öğrenci sayfalarını gruplamak ve OCR hazırlığı yapmak için cevap PDF’ini yükleyin.',
    dialogLabel: 'Öğrenci Cevap PDF',
    deleteImpact: 'Bu belgeye bağlı sayfa grupları, önizlemeler ve OCR hazırlıkları geçersiz hâle gelebilir.',
  },
};

const previewLabels: Record<WorkspacePreviewState, string> = {
  not_started: 'Önizleme hazırlanmadı',
  queued: 'Önizleme hazırlanıyor',
  running: 'Önizleme hazırlanıyor',
  ready: 'İncelemeye hazır',
  failed: 'Önizleme oluşturulamadı',
};

function assertNever(value: never): never {
  throw new Error(`Desteklenmeyen belge rolü: ${String(value)}`);
}

export function isWorkspaceDocumentRole(role: DocumentRole): role is WorkspaceDocumentRole {
  return role === 'exam_source' || role === 'answer_key' || role === 'student_scan';
}

export function getWorkspaceRoleDetails(role: WorkspaceDocumentRole) {
  return roleDetails[role];
}

export function toWorkspacePreviewState(status?: PdfPreviewStatus | null): WorkspacePreviewState {
  if (!status || status === 'missing') return 'not_started';
  return status;
}

export function getWorkspacePreviewLabel(state: WorkspacePreviewState): string {
  return previewLabels[state];
}

export function shouldShowSelectedDocumentPanel(
  role: WorkspaceDocumentRole,
  hasSelectedDocument: boolean,
): boolean {
  return role !== 'student_scan' || hasSelectedDocument;
}

export function buildDocumentWorkspaceItems(
  documents: Document[],
  preferredDocumentId?: string | null,
): DocumentWorkspaceItem[] {
  const preferredDocument = preferredDocumentId
    ? documents.find((document) => document.id === preferredDocumentId && isWorkspaceDocumentRole(document.role))
    : undefined;

  return workspaceDocumentRoles.map((role) => {
    const roleDocuments = documents.filter((document) => document.role === role);
    const document = preferredDocument?.role === role
      ? preferredDocument
      : roleDocuments[roleDocuments.length - 1] ?? null;
    const previewState = toWorkspacePreviewState(document?.preview?.status);

    return {
      ...roleDetails[role],
      document,
      documentName: document?.fileName,
      pageCount: document?.pageCount,
      uploadState: document ? 'ready' : 'missing',
      previewState,
      previewLabel: previewLabels[previewState],
      canUpload: true,
      canDelete: document !== null,
    };
  });
}

export function getDocumentWorkspaceSummary(
  items: DocumentWorkspaceItem[],
  activePreviewCount: number,
): DocumentWorkspaceSummary {
  return {
    uploadedCount: items.filter((item) => item.uploadState === 'ready').length,
    readyPreviewCount: items.filter((item) => item.previewState === 'ready').length,
    activePreviewCount,
    failedPreviewCount: items.filter((item) => item.previewState === 'failed').length,
  };
}

export function getAutomaticPreviewTargets(documents: Document[]): Array<{
  role: WorkspaceDocumentRole;
  documentId: string;
}> {
  return documents.flatMap((document) => (
    isWorkspaceDocumentRole(document.role)
      && ['not_started', 'failed'].includes(toWorkspacePreviewState(document.preview?.status))
      ? [{ role: document.role, documentId: document.id }]
      : []
  ));
}

export async function runAutomaticPreviewQueue(
  targets: Array<{ role: WorkspaceDocumentRole; documentId: string }>,
  startPreview: (target: { role: WorkspaceDocumentRole; documentId: string }) => Promise<unknown>,
): Promise<void> {
  for (const target of targets) {
    try {
      await startPreview(target);
    } catch {
      // One failed document must not prevent the remaining documents from being processed.
    }
  }
}

export function resolveWorkspaceRole(
  roleParam: string | null,
  document: Document | null | undefined,
): WorkspaceDocumentRole {
  if (document && isWorkspaceDocumentRole(document.role)) return document.role;
  switch (roleParam) {
    case 'exam':
    case 'exam_source':
      return 'exam_source';
    case 'answer':
    case 'answer_key':
      return 'answer_key';
    case 'student':
    case 'student_scan':
      return 'student_scan';
    default:
      return 'exam_source';
  }
}

export function documentTypeParam(role: WorkspaceDocumentRole): string {
  switch (role) {
    case 'exam_source':
      return 'exam';
    case 'answer_key':
      return 'answer_key';
    case 'student_scan':
      return 'student';
    default:
      return assertNever(role);
  }
}

export function importWorkspaceDocument(
  gateway: DocumentWorkspaceCommandGateway,
  role: WorkspaceDocumentRole,
  input: { projectId: string; sourcePath: string },
): Promise<Document> {
  switch (role) {
    case 'exam_source':
      return gateway.importExamSourcePdf(input);
    case 'answer_key':
      return gateway.importAnswerKeyPdf(input);
    case 'student_scan':
      return gateway.importStudentScanPdf(input);
    default:
      return assertNever(role);
  }
}

export function startWorkspacePreview(
  gateway: DocumentWorkspaceCommandGateway,
  role: WorkspaceDocumentRole,
  input: { projectId: string; documentId: string },
): Promise<StartPdfPreviewRenderOutput> {
  switch (role) {
    case 'exam_source':
    case 'answer_key':
      return gateway.startPdfPreviewRender(input);
    case 'student_scan':
      return gateway.startStudentScanPreviewRender(input);
    default:
      return assertNever(role);
  }
}

export function getWorkspacePreviewStatus(
  gateway: DocumentWorkspaceCommandGateway,
  role: WorkspaceDocumentRole,
  input: { projectId: string; documentId: string },
): Promise<PdfPreviewStatusSnapshot> {
  switch (role) {
    case 'exam_source':
    case 'answer_key':
      return gateway.getPdfPreviewStatus(input);
    case 'student_scan':
      return gateway.getStudentScanPreviewStatus(input);
    default:
      return assertNever(role);
  }
}

export function createDocumentImportController(
  selectPdf: (role: WorkspaceDocumentRole) => Promise<string | null>,
  importPdf: (role: WorkspaceDocumentRole, sourcePath: string) => Promise<Document>,
) {
  let pending: Promise<Document | null> | undefined;
  return {
    run(role: WorkspaceDocumentRole): Promise<Document | null> {
      if (pending) return pending;
      const operation = selectPdf(role)
        .then((sourcePath) => sourcePath ? importPdf(role, sourcePath) : null)
        .finally(() => {
          pending = undefined;
        });
      pending = operation;
      return operation;
    },
  };
}

export function createPreviewStartController(
  startPreview: (role: WorkspaceDocumentRole, documentId: string) => Promise<StartPdfPreviewRenderOutput>,
) {
  const pendingByDocument = new Map<string, Promise<StartPdfPreviewRenderOutput>>();
  return {
    run(role: WorkspaceDocumentRole, documentId: string): Promise<StartPdfPreviewRenderOutput> {
      const existing = pendingByDocument.get(documentId);
      if (existing) return existing;
      const operation = startPreview(role, documentId).finally(() => {
        pendingByDocument.delete(documentId);
      });
      pendingByDocument.set(documentId, operation);
      return operation;
    },
  };
}

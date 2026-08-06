export type DocumentRole = 'student_scan' | 'exam_source' | 'answer_key' | 'rubric' | 'export';

export type Document = {
  id: string;
  role: DocumentRole;
  fileName: string;
  storedPath: string;
  pageCount: number;
  addedAt: string;
  checksum?: string;
  preview?: PdfPreviewState | null;
};

export type PdfPreviewStatus = 'missing' | 'queued' | 'running' | 'ready' | 'failed';

export type PdfPreviewState = {
  status: PdfPreviewStatus;
  renderedAt?: string | null;
  pageCount?: number | null;
  jobId?: string | null;
  errorMessage?: string | null;
  activeGenerationId?: string | null;
  pendingGenerationId?: string | null;
  sourceFingerprint?: string | null;
};

export type PdfPagePreview = {
  documentId: string;
  pageNumber: number;
  imagePath: string;
  width: number;
  height: number;
  renderedAt: string;
};

export type PdfPreviewStatusSnapshot = {
  documentId: string;
  status: PdfPreviewStatus;
  pageCount: number;
  renderedAt?: string | null;
  jobId?: string | null;
  previewCount: number;
  message: string;
  errorMessage?: string | null;
};

export type StartPdfPreviewRenderOutput = {
  jobId: string;
  status: 'queued' | 'running';
};

export type PdfRendererStatus = {
  available: boolean;
  backend: 'poppler' | 'macos_fallback' | 'none';
  pdfinfoPath?: string;
  pdftoppmPath?: string;
  searchedPaths: string[];
  pathEnv?: string;
  installHint?: string;
  warnings: string[];
};

export type ImportDocumentInput = {
  projectId: string;
  sourcePath: string;
};

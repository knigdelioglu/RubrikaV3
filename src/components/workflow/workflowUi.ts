import type { Document, WorkflowAction } from '../../api/types';

export function getPrimaryWorkflowAction(actions: WorkflowAction[]): WorkflowAction | null {
  return actions.find((action) => action.enabled) ?? actions[0] ?? null;
}

export function describeStudentScanPreview(document: Pick<Document, 'pageCount' | 'preview'>): string {
  const previewStatus = document.preview?.status ?? 'missing';

  if (previewStatus === 'missing') {
    if (document.pageCount > 0) {
      return 'Öğrenci PDF yüklendi, sayfa önizlemesi arka planda hazırlanıyor.';
    }
    return 'Öğrenci PDF yüklendi, sayfa sayısı henüz bilinmiyor. Önizleme oluşturulduğunda sayfa sayısı hesaplanacak.';
  }

  if (previewStatus === 'queued' || previewStatus === 'running') {
    return 'Öğrenci PDF önizlemesi oluşturuluyor.';
  }

  if (previewStatus === 'ready') {
    const pageCount = document.preview?.pageCount ?? document.pageCount;
    return pageCount > 0
      ? `Öğrenci PDF önizlemesi hazır. Sayfa sayısı: ${pageCount}.`
      : 'Öğrenci PDF önizlemesi hazır.';
  }

    return 'Öğrenci PDF önizlemesi hazırlanamadı; hata işlem merkezinde görülebilir.';
}

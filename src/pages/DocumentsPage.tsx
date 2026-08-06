import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { open } from '@tauri-apps/plugin-dialog';
import { AlertTriangle, CheckCircle2, FileText, Loader2, Upload } from 'lucide-react';
import { commands } from '../api/commands';
import type { AppError } from '../api/errors';
import type { Document, JobSnapshot, StudentScanBatch } from '../api/types';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { ProjectContextState } from '../components/common/ProjectContextState';
import { LoadingButton } from '../components/common/LoadingButton';
import { ConfirmationDialog } from '../components/common/ConfirmationDialog';
import { DocumentPreviewViewer } from '../components/pdf/DocumentPreviewViewer';
import { tauriClient } from '../api/tauriClient';
import { formatPageCount } from '../utils/formatting';
import { useProjectContext } from '../state/useProjectContext';
import { projectStudentOperationsPath } from '../app/projectRoutes';
import { getStudentBatchImportDisabledReason, suggestSchoolClassFromFilename } from './studentOperations';
import { createDocumentRemovalController } from './documentRemoval';
import {
  buildDocumentWorkspaceItems,
  createDocumentImportController,
  documentTypeParam,
  getAutomaticPreviewTargets,
  getDocumentWorkspaceSummary,
  getWorkspacePreviewLabel,
  getWorkspacePreviewStatus,
  getWorkspaceRoleDetails,
  importWorkspaceDocument,
  resolveWorkspaceRole,
  runAutomaticPreviewQueue,
  shouldShowSelectedDocumentPanel,
  startWorkspacePreview,
  toWorkspacePreviewState,
  type WorkspaceDocumentRole,
} from './documentWorkspace';

function isAppError(value: unknown): value is AppError {
  if (typeof value !== 'object' || value === null) return false;
  const candidate = value as Record<string, unknown>;
  return typeof candidate.code === 'string'
    && typeof candidate.safeMessage === 'string'
    && typeof candidate.retryable === 'boolean';
}

function operationError(value: unknown, message: string, suggestedAction: string): AppError {
  return {
    code: isAppError(value) ? value.code : 'UNKNOWN_ERROR',
    safeMessage: message,
    retryable: true,
    recoveryAction: suggestedAction,
    correlationId: isAppError(value) ? value.correlationId : crypto.randomUUID?.() || 'unknown',
    detailsAvailable: false,
  };
}

function initialPageFromSearch(searchParams: URLSearchParams): number {
  const page = Number(searchParams.get('page'));
  return Number.isInteger(page) && page > 0 ? page : 1;
}

function isActiveJob(job: JobSnapshot): boolean {
  return job.status === 'queued' || job.status === 'running';
}

function fileNameFromPath(path: string): string {
  return path.split(/[\\/]/).pop() || 'Öğrenci cevapları.pdf';
}

export function DocumentsPage({ hideHeader = false }: { hideHeader?: boolean } = {}) {
  const { projectId, projectPath, isResolving } = useProjectContext();
  const [searchParams, setSearchParams] = useSearchParams();
  const queryClient = useQueryClient();
  const deepLinkedDocumentId = searchParams.get('documentId');
  const [selectedRole, setSelectedRole] = useState<WorkspaceDocumentRole>(() =>
    resolveWorkspaceRole(searchParams.get('documentType'), null));
  const [preferredDocumentId, setPreferredDocumentId] = useState<string | null>(deepLinkedDocumentId);
  const [currentPage, setCurrentPage] = useState(() => initialPageFromSearch(searchParams));
  const [error, setError] = useState<AppError | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const [documentToRemove, setDocumentToRemove] = useState<Document | null>(null);
  const [pendingStudentScanPath, setPendingStudentScanPath] = useState<string | null>(null);
  const [studentBatchClassId, setStudentBatchClassId] = useState(() => searchParams.get('classId') || '');
  const [newClassName, setNewClassName] = useState('');
  const [batchToRemove, setBatchToRemove] = useState<StudentScanBatch | null>(null);
  const automaticPreviewStartedRef = useRef(new Set<string>());

  const documentsQuery = useQuery({
    queryKey: ['documents', projectId],
    queryFn: () => commands.listDocuments(projectId),
    enabled: !!projectId,
  });
  const documents = useMemo(() => documentsQuery.data ?? [], [documentsQuery.data]);
  const projectQuery = useQuery({
    queryKey: ['project-snapshot', projectId],
    queryFn: () => commands.getProjectSnapshot(projectId),
    enabled: !!projectId,
  });
  const classesQuery = useQuery({
    queryKey: ['school-classes', projectId, 'active'],
    queryFn: () => commands.listSchoolClasses({ projectId, includeArchived: false }),
    enabled: !!projectId,
  });
  const batchesQuery = useQuery({
    queryKey: ['student-scan-batches', projectId],
    queryFn: () => commands.listStudentScanBatches({ projectId }),
    enabled: !!projectId,
  });
  const schoolClasses = classesQuery.data ?? projectQuery.data?.schoolClasses ?? [];
  const studentScanBatches = batchesQuery.data ?? projectQuery.data?.studentScanBatches ?? [];
  const deepLinkedDocument = deepLinkedDocumentId
    ? documents.find((document) => document.id === deepLinkedDocumentId)
    : null;

  useEffect(() => {
    if (!deepLinkedDocument) return;
    const role = resolveWorkspaceRole(searchParams.get('documentType'), deepLinkedDocument);
    setSelectedRole(role);
    setPreferredDocumentId(deepLinkedDocument.id);
  }, [deepLinkedDocument, searchParams]);

  const workspaceItems = useMemo(
    () => buildDocumentWorkspaceItems(documents, preferredDocumentId),
    [documents, preferredDocumentId],
  );
  const selectedItem = workspaceItems.find((item) => item.role === selectedRole) ?? workspaceItems[0];
  const selectedDocument = selectedItem?.document ?? null;

  const jobsQuery = useQuery({
    queryKey: ['jobs', projectId],
    queryFn: () => commands.listJobs(projectId),
    enabled: !!projectId,
  });
  const activePreviewJobs = (jobsQuery.data ?? []).filter(
    (job) => job.kind === 'pdf_preview_render' && isActiveJob(job),
  );

  const previewStatusQuery = useQuery({
    queryKey: ['document-preview-status', projectId, selectedRole, selectedDocument?.id],
    queryFn: () => getWorkspacePreviewStatus(commands, selectedRole, {
      projectId,
      documentId: selectedDocument?.id ?? '',
    }),
    enabled: !!projectId && !!selectedDocument,
    refetchInterval: (query) => {
      const status = query.state.data?.status;
      return status === 'queued' || status === 'running' ? 1000 : false;
    },
  });
  const effectivePreviewState = toWorkspacePreviewState(
    previewStatusQuery.data?.status ?? selectedDocument?.preview?.status,
  );
  const hasUsablePreview = Boolean(
    selectedDocument?.preview?.activeGenerationId
      || selectedDocument?.preview?.renderedAt,
  );

  const pagePreviewsQuery = useQuery({
    queryKey: ['pdf-page-previews', projectId, selectedDocument?.id],
    queryFn: () => commands.listPdfPagePreviews({
      projectId,
      documentId: selectedDocument?.id ?? '',
    }),
    enabled: !!projectId && !!selectedDocument && (effectivePreviewState === 'ready' || hasUsablePreview),
  });
  const pagePreviews = pagePreviewsQuery.data ?? [];
  const selectedJobId = previewStatusQuery.data?.jobId ?? selectedDocument?.preview?.jobId;
  const selectedPreviewJob = activePreviewJobs.find((job) => job.id === selectedJobId);
  const summary = getDocumentWorkspaceSummary(workspaceItems, activePreviewJobs.length);

  const invalidateWorkspace = useCallback((documentId?: string) => {
    queryClient.invalidateQueries({ queryKey: ['documents', projectId] });
    queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
    queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
    queryClient.invalidateQueries({ queryKey: ['jobs', projectId] });
    if (documentId) {
      queryClient.invalidateQueries({ queryKey: ['document-preview-status', projectId] });
      queryClient.invalidateQueries({ queryKey: ['pdf-page-previews', projectId, documentId] });
    }
  }, [projectId, queryClient]);

  useEffect(() => {
    if (!projectId || documentsQuery.isLoading) return;
    const targets = getAutomaticPreviewTargets(documents).filter((target) => {
      if (automaticPreviewStartedRef.current.has(target.documentId)) return false;
      automaticPreviewStartedRef.current.add(target.documentId);
      return true;
    });
    if (targets.length === 0) return;
    void runAutomaticPreviewQueue(targets, async (target) => {
      try {
        await startWorkspacePreview(commands, target.role, {
          projectId,
          documentId: target.documentId,
        });
      } finally {
        invalidateWorkspace(target.documentId);
      }
    });
  }, [documents, documentsQuery.isLoading, invalidateWorkspace, projectId]);

  function updateDeepLink(role: WorkspaceDocumentRole, documentId: string | null, page = 1) {
    const next = new URLSearchParams(searchParams);
    next.set('documentType', documentTypeParam(role));
    if (documentId) next.set('documentId', documentId);
    else next.delete('documentId');
    if (page > 1) next.set('page', String(page));
    else next.delete('page');
    setSearchParams(next, { replace: true });
  }

  function selectRole(role: WorkspaceDocumentRole) {
    const item = workspaceItems.find((candidate) => candidate.role === role);
    const documentId = item?.document?.id ?? null;
    setSelectedRole(role);
    setPreferredDocumentId(documentId);
    setCurrentPage(1);
    setError(null);
    setSuccessMessage(null);
    updateDeepLink(role, documentId);
  }

  const importMutation = useMutation({
    mutationFn: ({ role, sourcePath }: { role: WorkspaceDocumentRole; sourcePath: string }) =>
      importWorkspaceDocument(commands, role, { projectId, sourcePath }),
    onMutate: () => {
      setError(null);
      setSuccessMessage(null);
    },
    onSuccess: (document) => {
      invalidateWorkspace();
      setSelectedRole(resolveWorkspaceRole(null, document));
      setPreferredDocumentId(document.id);
      setCurrentPage(1);
      updateDeepLink(resolveWorkspaceRole(null, document), document.id);
      setSuccessMessage(`${document.fileName} başarıyla yüklendi; önizleme arka planda hazırlanıyor.`);
    },
    onError: (mutationError) => {
      setError(operationError(
        mutationError,
        'PDF yüklenemedi. Mevcut belge korunuyor.',
        'Dosyayı ve erişim izinlerini kontrol edip yeniden deneyin.',
      ));
    },
  });

  const batchImportMutation = useMutation({
    mutationFn: ({ sourcePath, classId }: { sourcePath: string; classId: string }) =>
      commands.importStudentScanBatch({
        projectId,
        classId,
        sourcePath,
        displayName: fileNameFromPath(sourcePath),
      }),
    onMutate: () => {
      setError(null);
      setSuccessMessage(null);
    },
    onSuccess: (output) => {
      invalidateWorkspace(output.document.id);
      queryClient.invalidateQueries({ queryKey: ['school-classes', projectId] });
      queryClient.invalidateQueries({ queryKey: ['student-scan-batches', projectId] });
      setPendingStudentScanPath(null);
      setPreferredDocumentId(output.document.id);
      updateDeepLink('student_scan', output.document.id);
      setSuccessMessage(`${output.document.fileName} sınıfa eklendi; önizleme arka planda hazırlanıyor.`);
    },
    onError: (mutationError) => setError(operationError(
      mutationError,
      'Öğrenci PDF paketi yüklenemedi. Mevcut paketler korunuyor.',
      'Sınıf seçimini, PDF dosyasını ve erişim izinlerini kontrol edin.',
    )),
  });

  const createClassMutation = useMutation({
    mutationFn: (name: string) => commands.createSchoolClass({ projectId, name }),
    onSuccess: (schoolClass) => {
      queryClient.invalidateQueries({ queryKey: ['school-classes', projectId] });
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
      setStudentBatchClassId(schoolClass.id);
      setNewClassName('');
      setSuccessMessage(`${schoolClass.name} sınıfı oluşturuldu. PDF paketini yüklemeden önce seçimi doğrulayın.`);
    },
    onError: (mutationError) => setError(operationError(
      mutationError,
      'Sınıf oluşturulamadı.',
      'Sınıf adını kontrol edip yeniden deneyin.',
    )),
  });

  const batchRemoveMutation = useMutation({
    mutationFn: (batchId: string) => commands.removeStudentScanBatch({ projectId, batchId }),
    onSuccess: (removedBatch) => {
      invalidateWorkspace(removedBatch.documentId);
      queryClient.invalidateQueries({ queryKey: ['student-scan-batches', projectId] });
      setBatchToRemove(null);
      setSuccessMessage('Öğrenci PDF paketi silindi.');
    },
    onError: (mutationError) => {
      setBatchToRemove(null);
      setError(operationError(
        mutationError,
        'Bu PDF paketi silinemedi; bağlı öğrenci, OCR veya notlandırma verileri korunuyor.',
        'Önce bağlı kayıtları gözden geçirin veya paketi başka bir sınıfa taşıyın.',
      ));
    },
  });
  const importMutationRef = useRef(importMutation.mutateAsync);
  importMutationRef.current = importMutation.mutateAsync;
  const importController = useMemo(
    () => createDocumentImportController(
      async (role) => {
        const details = getWorkspaceRoleDetails(role);
        const selected = await open({
          multiple: false,
          filters: [{ name: details.dialogLabel, extensions: ['pdf'] }],
        });
        return typeof selected === 'string' ? selected : null;
      },
      (role, sourcePath) => importMutationRef.current({ role, sourcePath }),
    ),
    [],
  );

  async function handleImport(role: WorkspaceDocumentRole) {
    if (role === 'student_scan') {
      await handleChooseStudentBatchPdf();
      return;
    }
    try {
      await importController.run(role);
    } catch (importError) {
      setError(operationError(
        importError,
        'PDF seçimi veya yükleme işlemi tamamlanamadı. Mevcut belge korunuyor.',
        'PDF dosyasını yeniden seçin.',
      ));
    }
  }

  async function handleChooseStudentBatchPdf() {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: 'Öğrenci cevap PDF paketi', extensions: ['pdf'] }],
      });
      if (typeof selected !== 'string') return;
      setPendingStudentScanPath(selected);
      const suggestedClass = suggestSchoolClassFromFilename(fileNameFromPath(selected), schoolClasses);
      if (suggestedClass) setStudentBatchClassId(suggestedClass.id);
      setError(null);
      setSuccessMessage(suggestedClass
        ? `Dosya adına göre ${suggestedClass.name} önerildi. Yüklemeden önce sınıfı doğrulayın.`
        : 'PDF seçildi. Yüklemeden önce sınıfı seçin.');
    } catch (selectionError) {
      setError(operationError(
        selectionError,
        'PDF seçimi tamamlanamadı.',
        'Dosya seçiciyi yeniden açın.',
      ));
    }
  }

  const documentRemoval = useMemo(
    () => createDocumentRemovalController(
      (documentId) => commands.removeDocument({ projectId, documentId }),
    ),
    [projectId],
  );
  const removeMutation = useMutation({
    mutationFn: () => documentRemoval.confirmSelection(),
    onMutate: () => {
      setError(null);
      setSuccessMessage(null);
    },
    onSuccess: (removed) => {
      if (!removed) return;
      const removedDocument = documentToRemove;
      invalidateWorkspace(removedDocument?.id);
      setDocumentToRemove(null);
      setPreferredDocumentId(null);
      setCurrentPage(1);
      updateDeepLink(selectedRole, null);
      setSuccessMessage(`${removedDocument?.fileName ?? 'Belge'} silindi.`);
    },
    onError: (mutationError) => {
      setDocumentToRemove(null);
      setError(operationError(
        mutationError,
        'Belge silinemedi. Belge ve mevcut önizleme korunuyor.',
        'Biraz sonra yeniden deneyin.',
      ));
    },
  });

  useEffect(() => {
    if (!projectId) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void tauriClient.listenToJobEvents(() => {
      if (cancelled) return;
      invalidateWorkspace(selectedDocument?.id);
    }).then((cleanup) => {
      unlisten = cleanup;
      if (cancelled) cleanup();
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [invalidateWorkspace, projectId, selectedDocument?.id]);

  if (isResolving) {
    return <ProjectContextState pageLabel="Belgeler" loading projectPath={projectPath} />;
  }
  if (!projectId) {
    return <ProjectContextState pageLabel="Belgeler" projectPath={projectPath} />;
  }

  const queryError = documentsQuery.error
    ?? projectQuery.error
    ?? classesQuery.error
    ?? batchesQuery.error
    ?? previewStatusQuery.error
    ?? pagePreviewsQuery.error;
  const pageCount = previewStatusQuery.data?.pageCount || selectedDocument?.pageCount || 0;
  const previewRunning = effectivePreviewState === 'queued' || effectivePreviewState === 'running';
  const actionBusy = importMutation.isPending
    || batchImportMutation.isPending
    || batchRemoveMutation.isPending
    || removeMutation.isPending;

  return (
    <div className="documents-workspace">
      {!hideHeader && (
        <>
          <div className="documents-workspace__breadcrumb">
            <Link to={`/project/${encodeURIComponent(projectId)}/overview`}>← İş akışına dön</Link>
          </div>

          <header className="documents-workspace__header">
            <div>
              <h2>Belgeler</h2>
              <p>Sınav belgelerini yükleyin ve sayfaların doğru göründüğünü kontrol edin.</p>
            </div>
            <dl className="documents-summary" aria-label="Belge özeti">
              <div><dt>Yüklü belge</dt><dd>{summary.uploadedCount} / 3</dd></div>
              <div><dt>Önizleme hazır</dt><dd>{summary.readyPreviewCount} / 3</dd></div>
              <div><dt>Devam eden iş</dt><dd>{summary.activePreviewCount}</dd></div>
              <div className={summary.failedPreviewCount ? 'has-error' : ''}><dt>Önizleme hatası</dt><dd>{summary.failedPreviewCount}</dd></div>
            </dl>
          </header>
        </>
      )}

      {error && <ErrorBanner error={error} />}
      {queryError && <ErrorBanner error={operationError(
        queryError,
        'Belge veya önizleme bilgileri alınamadı.',
        'Sayfayı yenileyip yeniden deneyin.',
      )} />}
      {successMessage && (
        <div className="documents-notice documents-notice--success" role="status">
          <CheckCircle2 size={18} aria-hidden="true" /> {successMessage}
        </div>
      )}

      <section className="document-selector" aria-label="Belge seçimi">
        {workspaceItems.map((item) => {
          const active = item.role === selectedRole;
          return (
            <button
              key={item.role}
              type="button"
              data-project-write="false"
              className={`document-selector__item ${active ? 'is-active' : ''}`}
              onClick={() => selectRole(item.role)}
              aria-pressed={active}
              aria-controls="selected-document-workspace"
            >
              <span className="document-selector__topline">
                <strong>{item.label}</strong>
                <span>{active ? 'Seçili belge' : item.uploadState === 'ready' ? 'Belge yüklendi' : 'Belge yüklenmedi'}</span>
              </span>
              <span className="document-selector__description">{item.description}</span>
              <span className="document-selector__file" title={item.documentName}>
                {item.documentName ?? 'Henüz PDF seçilmedi'}
              </span>
              <span className={`document-selector__status is-${item.previewState}`}>{item.previewLabel}</span>
            </button>
          );
        })}
      </section>

      {selectedRole === 'student_scan' && (
        <section className="student-batch-intake" aria-labelledby="student-batch-title">
          <div className="student-batch-intake__heading">
            <div>
              <span className="selected-document__eyebrow">Çoklu sınıf öğrenci alımı</span>
              <h3 id="student-batch-title">Öğrenci PDF paketleri</h3>
              <p>Her PDF’yi yüklemeden önce ait olduğu sınıfı doğrulayın. Bu sınıf, paketten oluşan bütün öğrencilere uygulanır.</p>
            </div>
            <button type="button" data-project-write="false" className="button button--primary" onClick={() => void handleChooseStudentBatchPdf()} disabled={actionBusy}>
              <Upload size={17} aria-hidden="true" /> PDF Paketi Seç
            </button>
          </div>

          <div className="student-batch-intake__controls">
            <label htmlFor="student-batch-class">
              <span>Sınıf</span>
              <select id="student-batch-class" value={studentBatchClassId} onChange={(event) => setStudentBatchClassId(event.target.value)}>
                <option value="">Sınıf seçin</option>
                {schoolClasses.map((schoolClass) => <option key={schoolClass.id} value={schoolClass.id}>{schoolClass.name}</option>)}
              </select>
            </label>
            <div className="student-batch-intake__new-class">
              <label htmlFor="student-batch-new-class">Yeni sınıf</label>
              <div>
                <input id="student-batch-new-class" value={newClassName} onChange={(event) => setNewClassName(event.target.value)} placeholder="Örn. 11-C" />
                <LoadingButton
                  type="button"
                  className="button button--secondary"
                  onClick={() => createClassMutation.mutate(newClassName.trim())}
                  loading={createClassMutation.isPending}
                  disabledReason={newClassName.trim() ? undefined : 'Sınıf adı girin.'}
                >
                  Sınıf Oluştur
                </LoadingButton>
              </div>
            </div>
          </div>

          {pendingStudentScanPath && (
            <div className="student-batch-intake__pending" role="status">
              <FileText size={20} aria-hidden="true" />
              <div><strong>{fileNameFromPath(pendingStudentScanPath)}</strong><span>Seçilen sınıfı doğruladıktan sonra yükleyin.</span></div>
              <button type="button" data-project-write="false" className="button button--secondary" onClick={() => setPendingStudentScanPath(null)} disabled={batchImportMutation.isPending}>İptal</button>
              <LoadingButton
                type="button"
                projectWrite={false}
                className="button button--primary"
                onClick={() => batchImportMutation.mutate({ sourcePath: pendingStudentScanPath, classId: studentBatchClassId })}
                loading={batchImportMutation.isPending}
                disabledReason={getStudentBatchImportDisabledReason(pendingStudentScanPath, studentBatchClassId)}
              >
                Sınıfa Yükle
              </LoadingButton>
            </div>
          )}

          {studentScanBatches.length === 0 ? (
            <div className="student-batch-intake__empty">
              Henüz öğrenci PDF paketi yok. Önce sınıfı oluşturun veya seçin, ardından PDF paketini yükleyin.
            </div>
          ) : (
            <div className="student-batch-grid">
              {studentScanBatches.map((batch) => {
                const schoolClass = schoolClasses.find((item) => item.id === batch.classId)
                  ?? projectQuery.data?.schoolClasses.find((item) => item.id === batch.classId);
                const document = documents.find((item) => item.id === batch.documentId);
                const submissionCount = projectQuery.data?.studentSubmissions.filter((item) => item.scanBatchId === batch.id).length ?? 0;
                const previewState = toWorkspacePreviewState(document?.preview?.status);
                return (
                  <article key={batch.id} className="student-batch-card">
                    <div className="student-batch-card__title"><strong>{schoolClass?.name ?? 'Sınıfı belirlenmemiş'}</strong><span>{batch.displayName || batch.originalFileName}</span></div>
                    <dl>
                      <div><dt>Sayfa</dt><dd>{document?.pageCount ? formatPageCount(document.pageCount) : 'Hazırlanıyor'}</dd></div>
                      <div><dt>Önizleme</dt><dd>{getWorkspacePreviewLabel(previewState)}</dd></div>
                      <div><dt>Gruplama</dt><dd>{batch.groupingCompletedAt ? 'Tamamlandı' : 'Bekliyor'}</dd></div>
                      <div><dt>Öğrenci</dt><dd>{submissionCount}</dd></div>
                    </dl>
                    <div className="student-batch-card__actions">
                      <button type="button" data-project-write="false" className="button button--secondary" onClick={() => {
                        setPreferredDocumentId(batch.documentId);
                        setCurrentPage(1);
                        updateDeepLink('student_scan', batch.documentId);
                      }}>Belgeyi Aç</button>
                      <Link className="button button--secondary" to={projectStudentOperationsPath(projectId, 'grouping', `classId=${encodeURIComponent(batch.classId)}&batchId=${encodeURIComponent(batch.id)}`)}>Gruplamayı Aç</Link>
                      <button type="button" data-project-write="false" className="button button--danger-outline" onClick={() => setBatchToRemove(batch)} disabled={actionBusy}>Sil</button>
                    </div>
                  </article>
                );
              })}
            </div>
          )}
          <Link className="student-batch-intake__classes-link" to={`/project/${encodeURIComponent(projectId)}/classes`}>Sınıfları ve paket taşımalarını yönet →</Link>
        </section>
      )}

      {shouldShowSelectedDocumentPanel(selectedRole, Boolean(selectedDocument)) && (
      <section id="selected-document-workspace" className="selected-document" aria-labelledby="selected-document-title">
        <div className="selected-document__heading">
          <div>
            <span className="selected-document__eyebrow">Seçili belge</span>
            <h3 id="selected-document-title">{selectedItem?.label}</h3>
            <p>{selectedItem?.purpose}</p>
          </div>
          {selectedDocument && selectedRole !== 'student_scan' && (
            <div className="selected-document__actions">
              <LoadingButton
                type="button"
                projectWrite={false}
                className="button button--secondary"
                onClick={() => void handleImport(selectedRole)}
                loading={importMutation.isPending && importMutation.variables?.role === selectedRole}
                disabled={removeMutation.isPending}
              >
                Belgeyi Değiştir
              </LoadingButton>
              <button
                type="button"
                data-project-write="false"
                className="button button--danger-outline"
                disabled={actionBusy}
                onClick={() => {
                  documentRemoval.selectDocument(selectedDocument.id);
                  setDocumentToRemove(selectedDocument);
                }}
              >
                Sil
              </button>
            </div>
          )}
        </div>

        {!selectedDocument ? (
          <div className="document-empty-state">
            <div className="document-empty-state__icon"><Upload size={28} aria-hidden="true" /></div>
            <h4>{selectedItem?.label} yüklenmedi</h4>
            <p>{selectedItem?.purpose}</p>
            <LoadingButton
              type="button"
              projectWrite={false}
              className="button button--primary document-empty-state__button"
              onClick={() => void handleImport(selectedRole)}
              loading={importMutation.isPending && importMutation.variables?.role === selectedRole}
            >
              PDF Seç
            </LoadingButton>
            <small>Desteklenen dosya türü: PDF</small>
          </div>
        ) : (
          <>
            <div className="document-details">
              <div className="document-details__file">
                <FileText size={22} aria-hidden="true" />
                <div><strong title={selectedDocument.fileName}>{selectedDocument.fileName}</strong><span>{pageCount > 0 ? formatPageCount(pageCount) : 'Sayfa sayısı önizleme sonrası belirlenecek'}</span></div>
              </div>
              <div className={`document-preview-state is-${effectivePreviewState}`}>
                <strong>{getWorkspacePreviewLabel(effectivePreviewState)}</strong>
                <span>{previewStatusQuery.data?.message ?? (previewRunning ? 'Sayfa görüntüleri hazırlanıyor.' : 'Belge yüklendi.')}</span>
              </div>
            </div>

            {previewRunning && (
              <div className="document-local-job" role="status" aria-live="polite">
                <Loader2 size={18} className="animate-spin" aria-hidden="true" />
                <div>
                  <strong>Önizleme hazırlanıyor</strong>
                  <span>{selectedPreviewJob?.progress.message || previewStatusQuery.data?.message || 'İşlem devam ediyor.'}</span>
                  {selectedPreviewJob && selectedPreviewJob.progress.total > 0 && (
                    <progress value={selectedPreviewJob.progress.current} max={selectedPreviewJob.progress.total}>
                      {selectedPreviewJob.progress.current} / {selectedPreviewJob.progress.total}
                    </progress>
                  )}
                </div>
              </div>
            )}

            {previewRunning && hasUsablePreview && (
              <div className="document-preview-callout">
                <FileText size={22} aria-hidden="true" />
                <div>
                  <strong>Önizleme yenileniyor</strong>
                  <p>Mevcut önizleme, yeni sürüm hazır olana kadar kullanılabilir.</p>
                </div>
              </div>
            )}

            {(effectivePreviewState === 'not_started' || effectivePreviewState === 'failed') && (
              <div className={`document-preview-callout ${effectivePreviewState === 'failed' ? 'is-error' : ''}`}>
                {effectivePreviewState === 'failed' ? <AlertTriangle size={22} aria-hidden="true" /> : <FileText size={22} aria-hidden="true" />}
                <div>
                  <strong>{effectivePreviewState === 'failed' ? 'Önizleme oluşturulamadı' : 'Önizleme arka planda hazırlanıyor'}</strong>
                  <p>{effectivePreviewState === 'failed' ? 'Belge korunuyor. Hata ayrıntılarını işlem merkezinden kontrol edebilirsiniz.' : 'Sayfa görüntüleri hazır olduğunda bu alanda görünecek.'}</p>
                </div>
              </div>
            )}

            {hasUsablePreview && pagePreviewsQuery.isLoading && (
              <div className="document-viewer-state" role="status"><Loader2 className="animate-spin" aria-hidden="true" /> Sayfa görüntüleri yükleniyor…</div>
            )}
            {hasUsablePreview && !pagePreviewsQuery.isLoading && pagePreviews.length === 0 && (
              <div className="document-preview-callout is-error">
                <AlertTriangle size={22} aria-hidden="true" />
                <div><strong>Sayfa görüntüsü henüz hazır değil</strong><p>Belge korunuyor. Arka plandaki önizleme işi tamamlandığında görüntüler burada görünecek.</p></div>
              </div>
            )}
            {hasUsablePreview && pagePreviews.length > 0 && (
              <DocumentPreviewViewer
                key={selectedDocument.id}
                documentName={selectedDocument.fileName}
                previews={pagePreviews}
                initialPage={currentPage}
                projectId={projectId}
                onPageChange={(page) => {
                  setCurrentPage(page);
                  updateDeepLink(selectedRole, selectedDocument.id, page);
                }}
              />
            )}
          </>
        )}
      </section>
      )}

      <ConfirmationDialog
        open={documentToRemove !== null}
        title={`${selectedItem?.label ?? 'PDF belgesi'} silinsin mi?`}
        description={`${documentToRemove?.fileName ?? 'Bu belge'} silinecek. ${selectedItem?.deleteImpact ?? 'Belgeye bağlı hazırlıklar geçersiz hâle gelebilir.'}`}
        confirmLabel="PDF’i Sil"
        destructive
        busy={removeMutation.isPending}
        onCancel={() => {
          documentRemoval.cancelSelection();
          setDocumentToRemove(null);
        }}
        onConfirm={() => removeMutation.mutate()}
      />
      <ConfirmationDialog
        open={batchToRemove !== null}
        title="Öğrenci PDF paketi silinsin mi?"
        description={`${batchToRemove?.displayName ?? 'Bu paket'} yalnız bağlı öğrenci, OCR veya notlandırma kaydı yoksa silinir. Bağlı veriler varsa işlem güvenli biçimde durdurulur.`}
        confirmLabel="Paketi Sil"
        destructive
        busy={batchRemoveMutation.isPending}
        onCancel={() => setBatchToRemove(null)}
        onConfirm={() => {
          if (batchToRemove) batchRemoveMutation.mutate(batchToRemove.id);
        }}
      />
    </div>
  );
}

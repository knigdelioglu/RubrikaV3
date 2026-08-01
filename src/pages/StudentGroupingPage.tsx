import { useEffect, useMemo, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { commands } from '../api/commands';
import type { AppError } from '../api/errors';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { ProjectContextState } from '../components/common/ProjectContextState';
import { LoadingButton } from '../components/common/LoadingButton';

import { tauriClient } from '../api/tauriClient';

import { useProjectContext } from '../state/useProjectContext';

import { FileStack, User, AlertCircle, CheckCircle2, ChevronRight, FileDigit } from 'lucide-react';
import { formatPageRange } from '../utils/formatting';
import { projectStudentOperationsPath } from '../app/projectRoutes';
import { filterStudentSubmissions, getSubmissionClassName } from './studentOperations';
export function StudentGroupingPage() {
  const [searchParams] = useSearchParams();
  const { projectId, projectPath, isResolving } = useProjectContext();
  const documentId = searchParams.get('documentId') || '';
  const classId = searchParams.get('classId') || '';
  const batchId = searchParams.get('batchId') || '';
  const queryClient = useQueryClient();
  const [error, setError] = useState<AppError | null>(null);
  const [pagesPerStudent, setPagesPerStudent] = useState('2');
  const [statusMessage, setStatusMessage] = useState<string | null>(null);

  const { data: project, isLoading: isProjectLoading } = useQuery({
    queryKey: ['project-snapshot', projectId],
    queryFn: () => commands.getProjectSnapshot(projectId),
    enabled: !!projectId,
  });

  const { data: submissions = [] } = useQuery({
    queryKey: ['student-submissions', projectId],
    queryFn: () => commands.listStudentSubmissions(projectId),
    enabled: !!projectId,
  });
  const visibleSubmissions = filterStudentSubmissions(submissions, classId, batchId);

  const { data: readiness } = useQuery({
    queryKey: ['ocr-readiness', projectId, batchId],
    queryFn: () => commands.getOcrReadiness(projectId, batchId || undefined),
    enabled: !!projectId,
  });

  const activeBatch = project?.studentScanBatches?.find((batch) => batch.id === batchId) ?? null;
  const hasCanonicalBatches = (project?.studentScanBatches?.length ?? 0) > 0;
  const activeDocument = useMemo(() => {
    if (!project) return null;
    if (activeBatch) {
      return project.documents.find((document) => document.id === activeBatch.documentId) ?? null;
    }
    if (hasCanonicalBatches && !documentId) return null;
    if (documentId) {
      return project.documents.find((document) => document.id === documentId && document.role === 'student_scan') ?? null;
    }
    if (project.studentScanDocumentId) {
      return project.documents.find((document) => document.id === project.studentScanDocumentId) ?? null;
    }
    return project.documents.find((document) => document.role === 'student_scan') ?? null;
  }, [activeBatch, documentId, hasCanonicalBatches, project]);

  const { data: previewStatus } = useQuery({
    queryKey: ['student-scan-preview-status', projectId, activeDocument?.id],
    queryFn: () => commands.getStudentScanPreviewStatus({ projectId, documentId: activeDocument!.id }),
    enabled: !!projectId && !!activeDocument?.id,
  });

  const refreshGroupingState = async () => {
    if (!projectId) return;
    const [projectSnapshot, readinessSnapshot] = await Promise.all([
      commands.getProjectSnapshot(projectId),
      commands.getOcrReadiness(projectId, batchId || undefined),
    ]);
    queryClient.setQueryData(['project-snapshot', projectId], projectSnapshot);
    queryClient.setQueryData(['workflow-snapshot', projectId], projectSnapshot.workflow);
    queryClient.setQueryData(['student-submissions', projectId], projectSnapshot.studentSubmissions);
    queryClient.setQueryData(['ocr-readiness', projectId, batchId], readinessSnapshot);
  };

  const parsedPagesPerStudent = Number(pagesPerStudent);
  const totalPages = activeDocument?.pageCount ?? 0;
  const pagesPerStudentIsValid =
    Number.isInteger(parsedPagesPerStudent) &&
    parsedPagesPerStudent >= 1 &&
    parsedPagesPerStudent <= 20 &&
    totalPages > 0 &&
    parsedPagesPerStudent <= totalPages;

  const createMutation = useMutation({
    mutationFn: async () => {
      if (!activeDocument) {
        throw {
          code: 'STUDENT_SCAN_NOT_FOUND',
          safeMessage: 'Öğrenci cevap PDF’i bulunamadı.',
          recoveryAction: 'Önce öğrenci cevap PDF’ini içe aktarın.',
          retryable: true,
          correlationId: 'unknown',
          detailsAvailable: false,
        } as AppError;
      }
      return commands.createStudentPageGroups({
        projectId,
        documentId: activeDocument.id,
        pagesPerStudent: parsedPagesPerStudent,
        batchId: activeBatch?.id,
      });
    },
    onMutate: () => {
      setError(null);
      setStatusMessage(null);
    },
    onSuccess: (result) => {
      setError(null);
      setStatusMessage(
        result.needsReview
          ? `${result.totalPages} sayfa, her öğrenci ${result.pagesPerStudent} sayfa olarak bölündüğünde ${result.remainderPages} sayfa artıyor. Lütfen son sayfayı manuel kontrol edin.`
          : `Öğrenciler sayfalara göre gruplandı: ${result.groupsCreated} grup.`,
      );
      void refreshGroupingState();
      queryClient.invalidateQueries({ queryKey: ['student-scan-preview-status', projectId] });
    },
    onError: (err: AppError) => {
      setStatusMessage(null);
      setError(err);
    },
  });

  const completeMutation = useMutation({
    mutationFn: () => commands.markStudentGroupingComplete({ projectId, batchId: activeBatch?.id }),
    onMutate: () => {
      setError(null);
      setStatusMessage(null);
    },
    onSuccess: () => {
      setError(null);
      setStatusMessage('Öğrenci grupları onaylandı.');
      void refreshGroupingState();
      queryClient.invalidateQueries({ queryKey: ['student-scan-preview-status', projectId] });
    },
    onError: (err: AppError) => {
      setStatusMessage(null);
      setError(err);
    },
  });

  useEffect(() => {
    if (!projectId) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void tauriClient.listenToJobEvents(() => {
      if (cancelled) return;
      queryClient.invalidateQueries({ queryKey: ['student-submissions', projectId] });
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['ocr-readiness', projectId] });
      queryClient.invalidateQueries({ queryKey: ['student-scan-preview-status', projectId] });
    }).then((cleanup) => {
      unlisten = cleanup;
      if (cancelled) {
        cleanup();
      }
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [projectId, queryClient]);

  useEffect(() => {
    if (activeBatch?.pagesPerStudent) {
      setPagesPerStudent(String(activeBatch.pagesPerStudent));
    }
  }, [activeBatch?.id, activeBatch?.pagesPerStudent]);

  if (isResolving || isProjectLoading) {
    return <ProjectContextState pageLabel="Öğrenci gruplama" loading projectPath={projectPath} />;
  }

  if (!projectId) {
    return <ProjectContextState pageLabel="Öğrenci gruplama" projectPath={projectPath} />;
  }

  if (!project) {
    return <ProjectContextState pageLabel="Öğrenci gruplama" projectPath={projectPath} />;
  }


  const isGroupingComplete = activeBatch
    ? !!activeBatch.groupingCompletedAt
    : !!project?.studentGroupingCompleteAt;
  
  const isPdfReady = !!activeDocument && previewStatus && previewStatus.previewCount === previewStatus.pageCount;

  return (
    <div style={{ maxWidth: '56rem', margin: '0 auto', padding: '2rem', display: 'flex', flexDirection: 'column', gap: '1.5rem', fontFamily: 'system-ui, -apple-system, sans-serif' }}>
      
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div>
          <h2 style={{ fontSize: '1.5rem', fontWeight: 700, color: '#0f172a', margin: 0, letterSpacing: '-0.025em' }}>Öğrenci Gruplama</h2>
          <p style={{ fontSize: '0.875rem', color: '#64748b', margin: '0.25rem 0 0 0' }}>Öğrenci PDF sayfaları submission (grup) haline getirildi.</p>
        </div>
        <div style={{ display: 'flex', gap: '1rem', alignItems: 'center' }}>
          <Link to={`/project/${encodeURIComponent(projectId)}/exam/documents?documentType=student`} style={{ color: '#475569', fontSize: '0.875rem', fontWeight: 500, textDecoration: 'none' }}>
            ← PDF Önizlemeler
          </Link>
          <Link to={`/project/${encodeURIComponent(projectId)}/overview`} style={{ color: '#4f46e5', fontSize: '0.875rem', fontWeight: 600, textDecoration: 'none', display: 'flex', alignItems: 'center', gap: '0.25rem' }}>
            İş Akışı <ChevronRight size={16} />
          </Link>
        </div>
      </div>

      {error && <ErrorBanner error={error} />}

      {/* Diagnostics / Uyarılar */}
      {!isGroupingComplete && (
        <div style={{ padding: '1rem', background: '#fef2f2', border: '1px solid #fecaca', borderRadius: '1rem', color: '#991b1b', fontSize: '0.875rem', display: 'flex', gap: '0.5rem', alignItems: 'flex-start' }}>
          <AlertCircle size={18} style={{ flexShrink: 0, marginTop: '0.125rem' }} />
          <div>
            <strong style={{ display: 'block', marginBottom: '0.25rem' }}>Gruplama Eksik veya Onaylanmadı</strong>
            {!activeDocument ? (
              <span>{hasCanonicalBatches ? 'Gruplamak için üstte bir öğrenci PDF paketi seçin.' : 'Öğrenci PDF’i henüz yüklenmedi. Lütfen önce belgeler ekranından bir öğrenci cevap PDF’i yükleyin.'}</span>
            ) : !isPdfReady ? (
              <span>Öğrenci PDF sayfaları şu anda işleniyor ({previewStatus?.previewCount ?? 0}/{previewStatus?.pageCount ?? activeDocument.pageCount}). Lütfen işlemin tamamlanmasını bekleyin.</span>
            ) : (
              <span>Sistem PDF sayfalarını gruplara ayırmaya hazır. Lütfen sayfa sayısını belirleyip gruplamayı başlatın ve onaylayın.</span>
            )}
          </div>
        </div>
      )}

      {statusMessage && (
        <div style={{ padding: '1rem', background: '#f0fdf4', border: '1px solid #bbf7d0', borderRadius: '1rem', color: '#166534', fontSize: '0.875rem', display: 'flex', gap: '0.5rem', alignItems: 'center' }}>
          <CheckCircle2 size={18} />
          {statusMessage}
        </div>
      )}

      {/* Kontrol Paneli */}
      <div style={{ background: 'white', border: '1px solid #e2e8f0', borderRadius: '1rem', padding: '1.5rem', display: 'grid', gap: '1rem', boxShadow: '0 1px 2px 0 rgba(0,0,0,0.05)' }}>
        <div style={{ display: 'flex', gap: '2rem', flexWrap: 'wrap', alignItems: 'flex-end' }}>
          <label style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem', flex: 1, minWidth: '200px' }}>
            <span style={{ fontSize: '0.875rem', fontWeight: 600, color: '#334155' }}>Bir öğrencinin sınavı kaç sayfa?</span>
            <input
              type="number"
              min={1} max={20} step={1}
              value={pagesPerStudent}
              onChange={(e) => setPagesPerStudent(e.target.value)}
              placeholder="Örn: 2"
              style={{ padding: '0.5rem 0.75rem', borderRadius: '0.5rem', border: '1px solid #cbd5e1', fontSize: '1rem' }}
            />
            <span style={{ fontSize: '0.75rem', color: '#64748b' }}>
              {totalPages > 0 ? `Seçili PDF toplam ${totalPages} sayfa.` : 'Gruplanacak sayfa sayısı.'} Örneğin 2 girerseniz ikişerli gruplara ayırır.
            </span>
          </label>
          <div style={{ display: 'flex', gap: '0.75rem' }}>
            <LoadingButton
              onClick={() => createMutation.mutate()}
              loading={createMutation.isPending}
              disabledReason={
                !activeDocument ? 'PDF seçilmedi' : !pagesPerStudentIsValid ? 'Geçersiz sayfa sayısı' : undefined
              }
              style={{ background: '#f8fafc', color: '#0f172a', border: '1px solid #cbd5e1', padding: '0.5rem 1rem', borderRadius: '0.5rem', fontWeight: 500 }}
            >
              Grupla
            </LoadingButton>
            <LoadingButton
              onClick={() => completeMutation.mutate()}
              loading={completeMutation.isPending}
              disabledReason={visibleSubmissions.length === 0 ? 'Önce seçili paketi gruplayın' : undefined}
              style={{ background: '#4f46e5', color: 'white', border: '1px solid #4338ca', padding: '0.5rem 1rem', borderRadius: '0.5rem', fontWeight: 500 }}
            >
              Grupları Onayla
            </LoadingButton>
          </div>
        </div>
      </div>

      {/* Teslimler Listesi */}
      <div style={{ background: 'white', border: '1px solid #e2e8f0', borderRadius: '1rem', overflow: 'hidden', boxShadow: '0 1px 2px 0 rgba(0,0,0,0.05)' }}>
        <div style={{ padding: '1rem', borderBottom: '1px solid #f1f5f9', background: '#f8fafc', display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
            <FileStack size={20} color="#64748b" />
            <h3 style={{ margin: 0, fontWeight: 600, color: '#334155', fontSize: '1rem' }}>Teslimler (Submissions)</h3>
          </div>
          <span style={{ background: '#e0e7ff', color: '#3730a3', fontSize: '0.75rem', fontWeight: 700, padding: '0.25rem 0.5rem', borderRadius: '0.25rem' }}>
            Toplam: {visibleSubmissions.length} Öğrenci
          </span>
        </div>

        {visibleSubmissions.length === 0 ? (
          <div style={{ padding: '3rem', textAlign: 'center', color: '#64748b', fontSize: '0.875rem' }}>
            Henüz öğrenci grubu oluşturulmadı. Lütfen yukarıdan "Grupla" butonunu kullanın.
          </div>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column' }}>
            {visibleSubmissions.map((sub) => {
              const student = project.students.find(s => s.id === sub.studentId);
              const pageRange = formatPageRange(sub.pageNumbers);
              const isVerified = student?.displayName || student?.number;
              const isReadyForOcr = readiness?.ready && sub.status === 'ready_for_ocr';

              return (
                <div key={sub.id} style={{ padding: '1rem', borderBottom: '1px solid #f1f5f9', display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '1rem', flexWrap: 'wrap' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '1rem' }}>
                    <div style={{ width: '2.5rem', height: '2.5rem', borderRadius: '9999px', background: '#f1f5f9', display: 'flex', alignItems: 'center', justifyContent: 'center', color: '#94a3b8' }}>
                      <User size={20} />
                    </div>
                    <div>
                      <h4 style={{ margin: 0, fontWeight: 500, color: '#0f172a', fontSize: '0.875rem' }}>{student?.displayName || student?.number ? `${student?.displayName || `Öğrenci ${student?.number}`} · ${getSubmissionClassName(project, sub)}` : `Kimliği doğrulanmamış öğrenci · ${getSubmissionClassName(project, sub)}`}</h4>
                      <p style={{ margin: '0.125rem 0 0 0', fontSize: '0.75rem', color: '#64748b', display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
                        <span>Sayfa {pageRange}</span>
                        <span>•</span>
                        <span>Toplam {sub.pageNumbers.length} sayfa</span>
                      </p>
                    </div>
                  </div>
                  
                  <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
                    {!isVerified && (
                      <Link to={projectStudentOperationsPath(projectId, 'identity', searchParams.toString())} style={{ textDecoration: 'none', display: 'flex', alignItems: 'center', gap: '0.375rem', color: '#d97706', background: '#fffbeb', padding: '0.25rem 0.5rem', borderRadius: '0.5rem', border: '1px solid #fde68a', fontSize: '0.75rem', fontWeight: 500 }}>
                        <AlertCircle size={14} /> Kimlik Doğrulanmadı
                      </Link>
                    )}
                    {!isReadyForOcr && (
                      <span style={{ display: 'flex', alignItems: 'center', gap: '0.375rem', color: '#64748b', background: '#f1f5f9', padding: '0.25rem 0.5rem', borderRadius: '0.5rem', border: '1px solid #e2e8f0', fontSize: '0.75rem', fontWeight: 500 }}>
                        <FileDigit size={14} /> OCR Bekliyor
                      </span>
                    )}
                    <Link
                      to={`/project/${encodeURIComponent(projectId)}/exam/documents?documentId=${encodeURIComponent(sub.documentId)}&documentType=student`}
                      style={{ color: '#4f46e5', fontWeight: 500, fontSize: '0.875rem', border: '1px solid #c7d2fe', background: '#eef2ff', padding: '0.375rem 0.75rem', borderRadius: '0.5rem', textDecoration: 'none' }}
                    >
                      Sayfaları İncele
                    </Link>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>

      <div style={{ padding: '1rem', background: '#eef2ff', border: '1px solid #e0e7ff', borderRadius: '0.75rem', color: '#3730a3', fontSize: '0.875rem' }}>
        Sistem öğrenci sayfalarını sayfa düzenine göre otomatik gruplandırmıştır. 
        Gerekirse manuel müdahaleler için "Sayfaları İncele" bölümünü kullanabilirsiniz.
      </div>
      
    </div>
  );
}

import { useEffect, useMemo, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { commands } from '../api/commands';
import type { AppError } from '../api/errors';
import type { OcrGeneration, OcrImagePreprocessMode, OcrReviewPolicyDto, StudentAnswerOcrJobMode } from '../api/types';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { ProjectContextState } from '../components/common/ProjectContextState';
import { PdfPageViewer } from '../components/pdf/PdfPageViewer';
import { ocrIssueTypeLabels, ocrPreprocessModeLabels, ocrWarningLabels, stageLabels, studentAnswerOcrStatusLabels } from '../utils/labels';
import { useProjectContext } from '../state/useProjectContext';
import { tauriClient } from '../api/tauriClient';
import { PlayCircle, Clock, AlertTriangle, Check, AlertCircle, RefreshCcw, FileImage } from 'lucide-react';
import {
  getStudentAnswerOcrDraftText,
  getMissingStudentAnswerCropQuestionNumbers,
  getStudentAnswerCropTemplateSummary,
  getStudentAnswerOcrPreviewMessage,
  getStudentAnswerOcrReviewedCount,
  getStudentAnswerOcrRerunVisible,
  getStudentAnswerOcrPreprocessSummary,
  getStudentAnswerOcrUncertaintySummary,
  getStudentAnswerOcrRerunConfirmMessage,
  getStudentAnswerOcrStartDisabledReasonWithHistory,
  hasApprovedStudentAnswerOcrRecords,
  getStudentAnswerOcrPreprocessVariantRef,
} from './studentAnswerOcrUi';
import { projectStudentOperationsPath } from '../app/projectRoutes';
import {
  filterStudentSubmissions,
  getSubmissionClassName,
  getStudentTeacherLabel,
} from './studentOperations';

function recordKey(submissionId: string, questionId: string) {
  return `${submissionId}:${questionId}`;
}

const friendlyWarning = (code: string, policy?: OcrReviewPolicyDto | null) => {
  const backendLabel = policy?.reasonLabels?.[code];
  if (backendLabel) return backendLabel;
  switch (code) {
    case 'answer_crop_may_be_incomplete': return 'Kırpma alanı cevabın tamamını içermeyebilir.';
    case 'answer_crop_may_be_truncated': return 'Crop sınırı kontrol edilmeli.';
    case 'full_page_fallback_review_required': return 'Tam sayfa fallback kullanıldı; soru kökü karışabilir.';
    case 'experimental_full_page_review_only': return 'Deneysel tam sayfa OCR yalnızca öğretmen incelemesi içindir; notlandırmaya onaylanamaz.';
    case 'structured_answer_invalid': return 'OCR yapısal cevabı soru tipiyle doğrulanamadı; öğretmen kontrolü gerekli.';
    case 'printed_question_leak_detected': return 'Soru kökü cevaba karışmış olabilir.';
    case 'printed_text_mixed': return 'Basılı metin karışmış olabilir.';
    case 'critical_keyword_uncertain': return 'Kritik terim belirsiz olabilir.';
    case 'ocr_commentary_detected': return 'OCR çıktısına görselde olmayan yorum karışmış olabilir.';
    case 'ocr_schema_incomplete': return 'OCR yanıtı gerekli kontrol alanlarını eksik döndürdü.';
    case 'ocr_answer_empty': return 'Cevap alanı boş veya okunamadı; öğretmen kontrolü gerekli.';
    case 'ocr_unreadable_span': return 'Cevabın bir bölümü okunamadı.';
    case 'ocr_scoring_fields_present': return 'OCR modeli okuma yerine puanlama bilgisi üretmeye çalıştı.';
    case 'json_parse_failed': return 'Model çıktısı otomatik çözülemedi.';
    case 'preprocess_failed': return 'Görüntü ön hazırlığı başarısız oldu; orijinal crop kullanıldı.';
    case 'preprocess_fallback_used': return 'El yazısı güçlendirme yerine yedek preprocess kullanıldı.';
    case 'critical_keyword_ocr_uncertain': return ocrWarningLabels.critical_keyword_ocr_uncertain;
    case 'ocr_critical_keyword_uncertain': return ocrWarningLabels.ocr_critical_keyword_uncertain;
    case 'ocr_parse_failed': return ocrWarningLabels.ocr_parse_failed;
    default: return ocrWarningLabels[code] ?? ocrIssueTypeLabels[code] ?? 'İnceleme gerekli';
  }
};

function OcrGenerationReviewPanel({
  generations,
  activeRecords,
  onAccept,
  onReject,
  disabled,
}: {
  generations: OcrGeneration[];
  activeRecords: import('../api/types').StudentAnswerOcrRecord[];
  onAccept: (generationId: string) => void;
  onReject: (generationId: string) => void;
  disabled: boolean;
}) {
  return (
    <section style={{ marginBottom: '1rem', padding: '1rem', border: '1px solid #c7d2fe', borderRadius: '0.75rem', background: '#eef2ff' }}>
      <div style={{ fontWeight: 700, color: '#3730a3', marginBottom: '0.65rem' }}>Yeni OCR önerileri</div>
      <div style={{ display: 'grid', gap: '0.65rem' }}>
        {generations.map((generation) => {
          const candidate = generation.result[0];
          const active = activeRecords.find((record) =>
            record.submissionId === generation.submissionId && record.questionId === candidate?.questionId,
          );
          const generationNonApprovable = generation.result.some(
            (record) => record.ocrProvenance?.approvableForScoring === false,
          );
          return (
            <div key={generation.generationId} style={{ padding: '0.75rem', borderRadius: '0.6rem', background: 'white', border: '1px solid #c7d2fe' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', gap: '0.75rem', alignItems: 'center', flexWrap: 'wrap' }}>
                <strong style={{ color: '#334155' }}>Mevcut OCR / Yeni OCR önerisi</strong>
                <span style={{ fontSize: '0.75rem', color: '#64748b' }}>{generation.status === 'candidate' ? 'Hazırlanıyor' : 'Karşılaştırmaya hazır'}</span>
              </div>
              {candidate && (
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '0.75rem', marginTop: '0.65rem', fontSize: '0.8125rem' }}>
                  <div><div style={{ color: '#64748b', fontWeight: 700 }}>Mevcut OCR</div><div>{active?.teacherCorrectedText ?? active?.answerText ?? 'Mevcut sonuç yok'}</div></div>
                  <div><div style={{ color: '#64748b', fontWeight: 700 }}>Yeni OCR önerisi</div><div>{candidate.answerText || 'Yeni sonuç ek kontrol gerektiriyor.'}</div></div>
                </div>
              )}
              {generation.status === 'ready_for_review' && (
                <div style={{ display: 'flex', gap: '0.5rem', marginTop: '0.7rem' }}>
                  <button type="button" data-project-write="true" disabled={disabled || generationNonApprovable} title={generationNonApprovable ? 'Deneysel OCR sonucu notlandırmaya onaylanamaz.' : undefined} onClick={() => onAccept(generation.generationId)} style={{ padding: '0.45rem 0.7rem', border: '1px solid #86efac', borderRadius: '0.45rem', background: '#dcfce7', color: '#166534', fontWeight: 700, opacity: generationNonApprovable ? 0.55 : 1 }}>{generationNonApprovable ? 'Yalnızca inceleme' : 'Yeni sonucu kabul et'}</button>
                  <button type="button" data-project-write="true" disabled={disabled} onClick={() => onReject(generation.generationId)} style={{ padding: '0.45rem 0.7rem', border: '1px solid #fecaca', borderRadius: '0.45rem', background: '#fef2f2', color: '#991b1b', fontWeight: 700 }}>Yeni sonucu reddet</button>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
}

export function StudentAnswerOcrPage() {
  const [searchParams] = useSearchParams();
  const { projectId, projectPath, isResolving } = useProjectContext();
  const classId = searchParams.get('classId') || '';
  const batchId = searchParams.get('batchId') || '';
  const queryClient = useQueryClient();
  const [error, setError] = useState<AppError | null>(null);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [selectedSubmissionIndex, setSelectedSubmissionIndex] = useState(0);
  const [comparisonModes, setComparisonModes] = useState<Record<string, OcrImagePreprocessMode | 'original'>>({});

  const { data: project, error: projectError } = useQuery({
    queryKey: ['project-snapshot', projectId],
    queryFn: () => commands.getProjectSnapshot(projectId!),
    enabled: !!projectId,
  });

  const { data: ocrReadiness } = useQuery({
    queryKey: ['ocr-readiness', projectId, batchId],
    queryFn: () => commands.getOcrReadiness(projectId!, batchId || undefined),
    enabled: !!projectId,
  });

  const { data: jobs = [] } = useQuery({
    queryKey: ['jobs', projectId],
    queryFn: () => commands.listJobs(projectId!),
    enabled: !!projectId,
  });

  const { data: modelStatus } = useQuery({
    queryKey: ['model-status'],
    queryFn: () => commands.getModelStatus(),
  });

  const startMutation = useMutation({
    mutationFn: () => commands.startStudentAnswerOcr({ projectId: projectId! }),
    onMutate: () => setError(null),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['jobs', projectId] });
    },
    onError: (err: AppError) => setError(err),
  });

  const experimentalMutation = useMutation({
    mutationFn: () => commands.startStudentAnswerOcr({
      projectId: projectId!,
      mode: 'experimental_full_page_review_only' as StudentAnswerOcrJobMode,
    }),
    onMutate: () => setError(null),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['jobs', projectId] });
    },
    onError: (err: AppError) => setError(err),
  });

  const rerunMutation = useMutation({
    mutationFn: () => commands.startStudentAnswerOcr({ projectId: projectId!, forceRerun: true }),
    onMutate: () => setError(null),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['jobs', projectId] });
    },
    onError: (err: AppError) => setError(err),
  });

  const saveMutation = useMutation({
    mutationFn: (input: { submissionId: string; questionId: string; text: string }) =>
      commands.updateStudentAnswerOcrText({ projectId: projectId!, ...input }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
    },
    onError: (err: AppError) => setError(err),
  });

  const approveMutation = useMutation({
    mutationFn: (input: { submissionId: string; questionId: string }) =>
      commands.markStudentAnswerOcrReviewed({ projectId: projectId!, ...input }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
    },
    onError: (err: AppError) => setError(err),
  });

  const approveAllMutation = useMutation({
    mutationFn: () => commands.markAllStudentAnswerOcrReviewed({ projectId: projectId! }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
    },
    onError: (err: AppError) => setError(err),
  });

  const acceptGenerationMutation = useMutation({
    mutationFn: (generationId: string) => commands.acceptStudentAnswerOcrGeneration({ projectId: projectId!, generationId }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
    },
    onError: (err: AppError) => setError(err),
  });

  const rejectGenerationMutation = useMutation({
    mutationFn: (generationId: string) => commands.rejectStudentAnswerOcrGeneration({ projectId: projectId!, generationId }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] }),
    onError: (err: AppError) => setError(err),
  });

  useEffect(() => {
    if (!project) return;
    setDrafts((current) => {
      const next = { ...current };
      for (const record of project.studentAnswerOcrRecords) {
        const key = recordKey(record.submissionId, record.questionId);
        if (next[key] === undefined) {
          next[key] = record.teacherCorrectedText ?? record.answerText;
        }
      }
      return next;
    });
  }, [project]);

  useEffect(() => {
    if (!projectId) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void tauriClient.listenToJobEvents(() => {
      if (cancelled) return;
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['jobs', projectId] });
    }).then((cleanup) => {
      unlisten = cleanup;
      if (cancelled) cleanup();
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [projectId, queryClient]);

  const studentById = useMemo(() => {
    return new Map(project?.students.map((student) => [student.id, student]) ?? []);
  }, [project]);

  const visibleSubmissions = filterStudentSubmissions(project?.studentSubmissions ?? [], classId, batchId);
  const visibleSubmissionIds = useMemo(
    () => new Set(visibleSubmissions.map((submission) => submission.id)),
    [visibleSubmissions],
  );
  const records = (project?.studentAnswerOcrRecords ?? []).filter((record) => visibleSubmissionIds.has(record.submissionId));
  const templateItems = project?.studentAnswerCropTemplate.templates ?? [];
  const missingTemplateQuestions = getMissingStudentAnswerCropQuestionNumbers(project?.questions ?? [], templateItems);



  const recordsBySubmission = useMemo(() => {
    const grouped = new Map<string, typeof records>();
    for (const record of records) {
      const list = grouped.get(record.submissionId) ?? [];
      list.push(record);
      grouped.set(record.submissionId, list);
    }
    return grouped;
  }, [records]);

  useEffect(() => {
    setSelectedSubmissionIndex(0);
  }, [classId, batchId]);

  if (isResolving) return <ProjectContextState pageLabel="Öğrenci Cevap OCR" loading projectPath={projectPath} />;
  if (!projectId || !project) return <ProjectContextState pageLabel="Öğrenci Cevap OCR" projectPath={projectPath} />;

  const queryError = (projectError as AppError | null) || error;

  const workflowStage = project?.workflow.currentStage ?? 'ocr_ready';
  const workflowLabel = stageLabels[workflowStage] ?? 'İş akışı kontrolü gerekli';
  const nextActions = project?.workflow.nextActions ?? [];
  const activeJob = jobs.find((job) => job.kind === 'student_answer_ocr' && (job.status === 'queued' || job.status === 'running'));
  
  const isOcrRunning = !!activeJob;
  const modelNotice = modelStatus && !modelStatus.healthOk && !isOcrRunning
    ? 'Model sunucusu hazır değil; OCR başlatılırken uygulama modeli otomatik başlatacak.'
    : null;
  const visibleStudentIds = new Set(visibleSubmissions.map((submission) => submission.studentId));
  const identityMissing = (project?.students ?? []).some((student) => {
    if (!visibleStudentIds.has(student.id)) return false;
    const hasName = !!student.displayName?.trim();
    const hasNumber = !!student.number?.trim();
    return !hasName && !hasNumber;
  });

  const totalRecords = records.length;
  const reviewedRecords = getStudentAnswerOcrReviewedCount(records);
  const hasNonApprovableRecords = records.some(
    (record) => record.ocrProvenance?.approvableForScoring === false,
  );
  const bulkApprovalDisabledReason = hasNonApprovableRecords
    ? 'Deneysel tam sayfa OCR kayıtları toplu olarak onaylanamaz.'
    : undefined;
  const hasExistingOcrRecords = (project?.studentAnswerOcrRecords.length ?? 0) > 0;
  const hasApprovedOcrRecords = hasApprovedStudentAnswerOcrRecords(project?.studentAnswerOcrRecords ?? []);
  const canRerun = getStudentAnswerOcrRerunVisible(nextActions, isOcrRunning);
  const productionAction = nextActions.find((action) => action.code === 'start_student_answer_ocr');
  const productionDisabledReason = productionAction?.disabledReason
    ?? getStudentAnswerOcrStartDisabledReasonWithHistory(workflowStage, totalRecords)
    ?? undefined;
  
  const submissions = visibleSubmissions;
  const currentSubmission = submissions[selectedSubmissionIndex];
  const currentRecords = currentSubmission ? recordsBySubmission.get(currentSubmission.id) ?? [] : [];
  const pendingGenerations = (project?.studentAnswerOcrGenerations ?? []).filter((generation) =>
    generation.status === 'ready_for_review' || generation.status === 'candidate',
  );

  const handleStartOCR = () => {
    if (isOcrRunning) return;
    if (hasExistingOcrRecords) {
      if (!window.confirm(getStudentAnswerOcrRerunConfirmMessage(hasApprovedOcrRecords))) {
        return;
      }
      rerunMutation.mutate();
      return;
    }
    startMutation.mutate();
  };

  const handleRerunOCR = () => {
    if (isOcrRunning) return;
    if (window.confirm('DİKKAT: Mevcut OCR sonuçları tamamen silinip yeniden üretilecek. Onayladığınız düzeltmeler de kaybolacaktır. Emin misiniz?')) {
      rerunMutation.mutate();
    }
  };

  return (
    <div style={{ padding: '2rem', display: 'flex', flexDirection: 'column', height: 'calc(100vh - 4rem)', maxWidth: '1440px', margin: '0 auto', fontFamily: 'system-ui, -apple-system, sans-serif' }}>
      {/* Breadcrumb & Header */}
      <div style={{ flexShrink: 0, marginBottom: '1.5rem' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', marginBottom: '1.5rem', fontSize: '0.875rem' }}>
          <Link to={`/project/${encodeURIComponent(projectId)}/overview`} style={{ color: '#64748b', textDecoration: 'none' }}>İş Akışı</Link>
          <span style={{ color: '#cbd5e1' }}>/</span>
          <span style={{ color: '#0f172a', fontWeight: 500 }}>Öğrenci Cevap OCR</span>
        </div>

        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', flexWrap: 'wrap', gap: '1rem' }}>
          <div>
            <h2 style={{ fontSize: '1.5rem', fontWeight: 700, color: '#0f172a', margin: 0, letterSpacing: '-0.025em' }}>Öğrenci Cevap OCR'ı</h2>
            <p style={{ fontSize: '0.875rem', color: '#64748b', margin: '0.25rem 0 0 0' }}>Öğrenci kağıtlarındaki el yazısı cevapları modele okutun ve doğrulayın.</p>
          </div>
          
          <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem', flexWrap: 'wrap' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', fontSize: '0.875rem', fontWeight: 500, color: '#475569', background: 'white', border: '1px solid #e2e8f0', padding: '0.375rem 0.75rem', borderRadius: '0.5rem' }}>
              <span style={{ width: '0.5rem', height: '0.5rem', borderRadius: '9999px', background: modelStatus?.healthOk ? '#10b981' : '#f59e0b' }}></span>
              Model Sunucusu {modelStatus?.healthOk ? 'Aktif' : 'Hazır Değil'}
            </div>
            {ocrReadiness?.ocrReviewPolicy && (
              <div title={ocrReadiness.ocrReviewPolicy.fingerprint} style={{ fontSize: '0.75rem', color: '#475569', background: '#f8fafc', border: '1px solid #cbd5e1', padding: '0.375rem 0.625rem', borderRadius: '0.5rem' }}>
                OCR politikası: {ocrReadiness.ocrReviewPolicy.version}
              </div>
            )}
            
            {canRerun && (
              <button
                data-project-write="true"
                onClick={handleRerunOCR}
                disabled={isOcrRunning || rerunMutation.isPending}
                style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', padding: '0.5rem 1rem', background: 'white', color: '#475569', border: '1px solid #cbd5e1', borderRadius: '0.5rem', fontSize: '0.875rem', fontWeight: 500, cursor: (isOcrRunning || rerunMutation.isPending) ? 'not-allowed' : 'pointer' }}
              >
                {rerunMutation.isPending ? <Clock size={16} className="animate-spin" /> : <RefreshCcw size={16} />} 
                Yeniden Çalıştır
              </button>
            )}
            
            <button
              data-project-write="true"
              onClick={handleStartOCR}
              disabled={isOcrRunning || startMutation.isPending || rerunMutation.isPending || !!productionDisabledReason}
              title={productionDisabledReason}
              style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', padding: '0.5rem 1.25rem', background: '#4f46e5', color: 'white', border: 'none', borderRadius: '0.5rem', fontSize: '0.875rem', fontWeight: 500, cursor: (isOcrRunning || startMutation.isPending || rerunMutation.isPending || !!productionDisabledReason) ? 'not-allowed' : 'pointer', opacity: (isOcrRunning || startMutation.isPending || rerunMutation.isPending || !!productionDisabledReason) ? 0.6 : 1, boxShadow: '0 1px 2px 0 rgba(0,0,0,0.05)' }}
            >
              {(isOcrRunning || startMutation.isPending || rerunMutation.isPending) ? (
                <><Clock size={16} className="animate-spin" /> Çalışıyor...</>
              ) : (
                <><PlayCircle size={16} /> OCR Başlat</>
              )}
            </button>
            {productionDisabledReason && (
              <span style={{ width: '100%', color: '#b91c1c', fontSize: '0.75rem' }}>
                Üretim OCR devre dışı: {productionDisabledReason}
              </span>
            )}
            {missingTemplateQuestions.length > 0 && !isOcrRunning && totalRecords === 0 && (
              <button
                type="button"
                data-project-write="true"
                onClick={() => experimentalMutation.mutate()}
                disabled={experimentalMutation.isPending}
                title="Sonuçlar notlandırmaya onaylanamaz"
                style={{ padding: '0.5rem 0.75rem', background: '#fff7ed', color: '#9a3412', border: '1px solid #fdba74', borderRadius: '0.5rem', fontSize: '0.75rem', fontWeight: 600 }}
              >
                {experimentalMutation.isPending ? 'Deneysel OCR çalışıyor...' : 'Deneysel tam sayfa inceleme'}
              </button>
            )}
            <Link
              to={projectStudentOperationsPath(projectId, 'issues', searchParams.toString())}
              style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', padding: '0.5rem 1rem', background: '#fff7ed', color: '#9a3412', border: '1px solid #fdba74', borderRadius: '0.5rem', fontSize: '0.875rem', fontWeight: 600, textDecoration: 'none' }}
            >
              <AlertTriangle size={16} /> Sorun İnceleme
            </Link>
          </div>
        </div>
      </div>

      {queryError && <div style={{ marginBottom: '1rem', flexShrink: 0 }}><ErrorBanner error={queryError} /></div>}

      {isOcrRunning && (
        <div style={{ marginBottom: '1rem', padding: '0.8rem 1rem', border: '1px solid #fde68a', borderRadius: '0.6rem', background: '#fffbeb', color: '#92400e', fontSize: '0.875rem' }}>
          OCR yeniden çalıştırılıyor. Mevcut onaylı sonuç korunuyor; yeni sonuç hazır olduğunda karşılaştırabilirsiniz.
        </div>
      )}

      {pendingGenerations.length > 0 && (
        <OcrGenerationReviewPanel
          generations={pendingGenerations}
          activeRecords={project?.studentAnswerOcrRecords ?? []}
          onAccept={(generationId) => acceptGenerationMutation.mutate(generationId)}
          onReject={(generationId) => rejectGenerationMutation.mutate(generationId)}
          disabled={acceptGenerationMutation.isPending || rejectGenerationMutation.isPending || isOcrRunning}
        />
      )}
      
      {modelNotice && (
        <div style={{ marginBottom: '1rem', padding: '0.75rem 1rem', border: '1px solid #fde68a', borderRadius: '0.5rem', background: '#fffbeb', color: '#92400e', fontSize: '0.875rem', display: 'flex', alignItems: 'center', gap: '0.5rem', flexShrink: 0 }}>
          <AlertTriangle size={16} /> {modelNotice}
        </div>
      )}

      {missingTemplateQuestions.length > 0 && (
        <div style={{ marginBottom: '1rem', padding: '0.75rem 1rem', border: '1px solid #fdba74', borderRadius: '0.5rem', background: '#fff7ed', color: '#9a3412', fontSize: '0.875rem' }}>
          Cevap region’ları eksik olan sorular: {missingTemplateQuestions.join(', ')}. Deneysel tam sayfa çıktıları yalnızca metin düzeltme referansıdır; scoring’e gönderilemez ve onaylanamaz.
        </div>
      )}
      
      {identityMissing && (
        <div style={{ marginBottom: '1rem', padding: '0.75rem 1rem', border: '1px solid #bfdbfe', borderRadius: '0.5rem', background: '#eff6ff', color: '#1e40af', fontSize: '0.875rem', display: 'flex', gap: '0.5rem', flexShrink: 0, alignItems: 'center', justifyContent: 'space-between' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
            <AlertCircle size={16} /> Öğrenci kimliği doğrulanmadı. Notlandırmadan önce öğrenci adı/numarası onaylanmalı.
          </div>
          <Link to={projectStudentOperationsPath(projectId, 'identity', searchParams.toString())} style={{ color: '#2563eb', fontWeight: 600, textDecoration: 'none' }}>Kimlikleri Doğrula</Link>
        </div>
      )}

      {/* Info Bar */}
      <div style={{ display: 'flex', gap: '1rem', flexWrap: 'wrap', marginBottom: '1.5rem', flexShrink: 0 }}>
        <div style={{ flex: 1, padding: '1rem', border: '1px solid #e2e8f0', borderRadius: '0.75rem', background: 'white', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
           <div>
             <span style={{ fontSize: '0.75rem', fontWeight: 600, color: '#64748b', textTransform: 'uppercase' }}>Workflow Durumu</span>
             <div style={{ fontSize: '0.875rem', fontWeight: 600, color: '#0f172a', marginTop: '0.25rem' }}>
               {workflowStage === 'student_answer_ocr_running' ? 'Çalışıyor' : workflowStage === 'student_answer_ocr_review_needed' ? 'Kontrol bekliyor' : workflowStage === 'student_answer_ocr_ready_for_scoring' ? 'Onaylandı' : 'Henüz başlatılmadı'}
               <span style={{ color: '#94a3b8', fontWeight: 400, marginLeft: '0.5rem' }}>({workflowLabel})</span>
             </div>
           </div>
           <div style={{ display: 'flex', gap: '1.5rem' }}>
              <div style={{ textAlign: 'center' }}>
                <div style={{ fontSize: '1.25rem', fontWeight: 700, color: '#d97706' }}>{totalRecords - reviewedRecords}</div>
                <div style={{ fontSize: '0.75rem', color: '#64748b', fontWeight: 500 }}>Kontrol Bekleyen</div>
              </div>
              <div style={{ textAlign: 'center' }}>
                <div style={{ fontSize: '1.25rem', fontWeight: 700, color: '#16a34a' }}>{reviewedRecords}</div>
                <div style={{ fontSize: '0.75rem', color: '#64748b', fontWeight: 500 }}>Onaylanan</div>
              </div>
              <div style={{ textAlign: 'center' }}>
                <div style={{ fontSize: '1.25rem', fontWeight: 700, color: '#dc2626' }}>
                  {records.filter((r) => r.needsReview || r.warnings.length > 0).length}
                </div>
                <div style={{ fontSize: '0.75rem', color: '#64748b', fontWeight: 500 }}>Sorunlu</div>
              </div>
           </div>
        </div>
        
        <div style={{ flex: 1, padding: '1rem', border: '1px solid #e2e8f0', borderRadius: '0.75rem', background: 'white', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
           <div>
             <span style={{ fontSize: '0.75rem', fontWeight: 600, color: '#64748b', textTransform: 'uppercase' }}>Crop Şablonu (Kırpma Alanları)</span>
             <div style={{ fontSize: '0.875rem', fontWeight: 600, color: '#0f172a', marginTop: '0.25rem' }}>
               {getStudentAnswerCropTemplateSummary(project?.questions ?? [], templateItems)}
             </div>
             {missingTemplateQuestions.length > 0 && (
               <div style={{ fontSize: '0.75rem', color: '#b91c1c', marginTop: '0.25rem' }}>
                 Eksik: Soru {missingTemplateQuestions.join(', ')}
               </div>
             )}
           </div>
           <Link 
             to={projectStudentOperationsPath(projectId, 'crops', searchParams.toString())}
             style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', padding: '0.5rem 1rem', background: '#f1f5f9', color: '#334155', textDecoration: 'none', borderRadius: '0.5rem', fontSize: '0.875rem', fontWeight: 500, border: '1px solid #e2e8f0', transition: 'background 0.2s' }}
           >
             <FileImage size={16} /> Şablonu Düzenle
           </Link>
        </div>
      </div>

      {/* Main Content Area */}
      <div style={{ display: 'flex', gap: '1.5rem', flex: 1, minHeight: 0 }}>
        {/* Sidebar - Students List */}
        <div style={{ width: '9rem', background: 'white', border: '1px solid #e2e8f0', borderRadius: '1rem', display: 'flex', flexDirection: 'column', flexShrink: 0, overflow: 'hidden' }}>
           <div style={{ padding: '1rem', borderBottom: '1px solid #f1f5f9', background: '#f8fafc' }}>
             <div style={{ fontSize: '0.875rem', fontWeight: 600, color: '#334155' }}>Öğrenciler ({submissions.length})</div>
           </div>
           <div style={{ overflowY: 'auto', flex: 1 }}>
             {submissions.map((sub, idx) => {
               const student = studentById.get(sub.studentId);
               const subRecords = recordsBySubmission.get(sub.id) ?? [];
               const approved = subRecords.filter(r => r.status === 'teacher_approved').length;
               const total = subRecords.length;
               const isSelected = selectedSubmissionIndex === idx;
               
               return (
                 <button
                   key={sub.id}
                   data-project-write="false"
                   onClick={() => setSelectedSubmissionIndex(idx)}
                   style={{ width: '100%', textAlign: 'left', padding: '1rem', background: isSelected ? '#eef2ff' : 'transparent', border: 'none', borderBottom: '1px solid #f1f5f9', cursor: 'pointer', transition: 'background 0.2s' }}
                 >
                   <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '0.375rem' }}>
                     <span style={{ fontSize: '0.875rem', fontWeight: 600, color: isSelected ? '#4f46e5' : '#0f172a' }}>
                       {getStudentTeacherLabel(student, getSubmissionClassName(project, sub))}
                     </span>
                     <span style={{ fontSize: '0.75rem', fontWeight: 600, color: '#64748b' }}>
                       {approved}/{total}
                     </span>
                   </div>
                   {student?.number && <div style={{ fontSize: '0.75rem', color: '#64748b', marginBottom: '0.5rem' }}>No: {student.number}</div>}
                   <div style={{ width: '100%', background: '#e2e8f0', borderRadius: '9999px', height: '0.375rem' }}>
                      <div style={{ background: '#10b981', height: '0.375rem', borderRadius: '9999px', width: total > 0 ? `${(approved/total)*100}%` : '0%' }}></div>
                   </div>
                 </button>
               )
             })}
           </div>
        </div>

        {/* Main Panel - OCR Results */}
        <div style={{ flex: 1, background: '#f8fafc', borderRadius: '1rem', border: '1px solid #e2e8f0', display: 'flex', flexDirection: 'column', position: 'relative', overflow: 'hidden' }}>
          {isOcrRunning && activeJob && (
            <div style={{ position: 'absolute', right: '1rem', top: '1rem', zIndex: 2, background: '#fffbeb', color: '#92400e', border: '1px solid #fde68a', borderRadius: '0.6rem', padding: '0.65rem 0.8rem', fontSize: '0.75rem' }}>
              {activeJob.progress.message} · {activeJob.progress.current}/{activeJob.progress.total}
            </div>
          )}

          <div style={{ padding: '1rem 1.5rem', borderBottom: '1px solid #e2e8f0', background: 'white', display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexShrink: 0 }}>
            <h3 style={{ fontSize: '1rem', fontWeight: 600, color: '#0f172a', margin: 0 }}>
              OCR Sonuç Kontrolü - {currentSubmission
                ? getStudentTeacherLabel(studentById.get(currentSubmission.studentId), getSubmissionClassName(project, currentSubmission))
                : 'Seçili kapsamda öğrenci yok'}
            </h3>
            {!classId && !batchId && (
              <button
                data-project-write="true"
                onClick={() => approveAllMutation.mutate()}
                disabled={approveAllMutation.isPending || isOcrRunning || totalRecords === 0 || !!bulkApprovalDisabledReason}
                title={bulkApprovalDisabledReason}
                style={{ padding: '0.5rem 1rem', background: '#f1f5f9', color: '#334155', border: '1px solid #cbd5e1', borderRadius: '0.5rem', fontSize: '0.875rem', fontWeight: 500, cursor: (approveAllMutation.isPending || isOcrRunning || totalRecords === 0 || !!bulkApprovalDisabledReason) ? 'not-allowed' : 'pointer' }}
              >
                Tüm Sorunsuzları Onayla
              </button>
            )}
          </div>

          <div style={{ flex: 1, overflowY: 'auto', padding: '1.5rem', display: 'flex', flexDirection: 'column', gap: '1.5rem' }}>
             {currentRecords.length === 0 && !isOcrRunning && (
               <div style={{ textAlign: 'center', padding: '3rem', color: '#64748b' }}>
                  {currentSubmission
                    ? 'Bu öğrenci için gösterilecek OCR kaydı bulunmuyor.'
                    : 'Seçili sınıf veya pakette öğrenci bulunmuyor. Üstteki filtreyi değiştirin ya da önce gruplama yapın.'}
               </div>
             )}
             {currentRecords.map(record => {
                const key = recordKey(record.submissionId, record.questionId);
                const draftValue = drafts[key] ?? getStudentAnswerOcrDraftText(record);
                const isSaving = saveMutation.isPending && saveMutation.variables?.submissionId === record.submissionId && saveMutation.variables?.questionId === record.questionId;
                const isApproving = approveMutation.isPending && approveMutation.variables?.submissionId === record.submissionId && approveMutation.variables?.questionId === record.questionId;
                
                const originalCropImage = record.originalCropRefs?.[0] ?? record.cropRefs[0] ?? null;
                const preprocessedCropImage = record.preprocessedCropRefs?.[0] ?? null;
                const pageImage = record.fullPagePreviewRefs[0] ?? record.sourceImageRefs[0] ?? null;
                const modelInputImage = record.modelInputCropRef ?? preprocessedCropImage ?? originalCropImage ?? pageImage;
                const overlayBox = !originalCropImage && record.renderDiagnostics?.cropBBox ? record.renderDiagnostics.cropBBox : null;
                const preprocessSummary = getStudentAnswerOcrPreprocessSummary(record);
                const availableVariants: OcrImagePreprocessMode[] = record.availablePreprocessVariants?.length
                  ? record.availablePreprocessVariants
                  : ['handwriting_enhanced', 'clean_grayscale', 'high_contrast', 'high_contrast_bw'];
                const selectedComparisonMode = comparisonModes[record.id] ?? record.preprocessMode ?? 'handwriting_enhanced';
                const selectedComparisonImage = getStudentAnswerOcrPreprocessVariantRef(record, selectedComparisonMode) ?? modelInputImage;
                const selectedComparisonLabel = selectedComparisonMode === 'original'
                  ? ocrPreprocessModeLabels.original
                  : ocrPreprocessModeLabels[selectedComparisonMode] ?? selectedComparisonMode;
                
                const isApproved = record.status === 'teacher_approved';
                const isNonApprovable = record.ocrProvenance?.approvableForScoring === false;
                const hasReviewWarnings = record.needsReview && !isApproved;

                return (
                  <div key={key} style={{ background: 'white', border: `1px solid ${isApproved ? '#86efac' : hasReviewWarnings ? '#fde047' : '#e2e8f0'}`, borderRadius: '0.75rem', overflow: 'hidden', boxShadow: hasReviewWarnings ? '0 0 0 1px #fef08a' : 'none', flexShrink: 0 }}>
                    {/* Header */}
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '0.75rem 1.25rem', borderBottom: '1px solid #f1f5f9', background: '#f8fafc' }}>
                       <div style={{ display: 'flex', alignItems: 'center', gap: '1rem' }}>
                         <span style={{ fontSize: '0.875rem', fontWeight: 700, color: '#334155' }}>Soru {record.questionNumber}</span>
                         {isApproved && (
                           <span style={{ fontSize: '0.7rem', fontWeight: 600, color: '#15803d', background: '#dcfce7', padding: '0.125rem 0.5rem', borderRadius: '0.25rem' }}>Onaylandı</span>
                         )}
                         {hasReviewWarnings && (
                           <span style={{ display: 'flex', alignItems: 'center', gap: '0.25rem', fontSize: '0.7rem', fontWeight: 600, color: '#b45309', background: '#fef3c7', padding: '0.125rem 0.5rem', borderRadius: '0.25rem' }}>
                             <AlertTriangle size={12} /> İnceleme Gerekli
                           </span>
                         )}
                         <span style={{ fontSize: '0.7rem', color: '#64748b' }}>
                           Durum: {studentAnswerOcrStatusLabels[record.status] ?? 'İnceleme gerekli'}
                         </span>
                       </div>
                       {!isApproved && !isNonApprovable && (
                         <button
                           data-project-write="true"
                           onClick={async () => {
                             await saveMutation.mutateAsync({ submissionId: record.submissionId, questionId: record.questionId, text: draftValue });
                             await approveMutation.mutateAsync({ submissionId: record.submissionId, questionId: record.questionId });
                           }}
                           disabled={isApproving || isSaving}
                           style={{ display: 'flex', alignItems: 'center', gap: '0.25rem', fontSize: '0.75rem', background: '#10b981', color: 'white', padding: '0.375rem 0.75rem', borderRadius: '0.375rem', border: 'none', fontWeight: 600, cursor: (isApproving || isSaving) ? 'not-allowed' : 'pointer' }}
                         >
                         <Check size={14} /> Onayla
                         </button>
                       )}
                       {isNonApprovable && !isApproved && (
                         <span style={{ fontSize: '0.7rem', color: '#9a3412', background: '#ffedd5', padding: '0.375rem 0.5rem', borderRadius: '0.375rem' }}>
                           Yalnızca inceleme · Onaylanamaz
                         </span>
                       )}
                    </div>
                    
                    {/* Body */}
                       <div style={{ padding: '1.25rem', display: 'grid', gridTemplateColumns: 'minmax(250px, 1fr) minmax(300px, 1.2fr)', gap: '1.5rem', alignItems: 'stretch' }}>
                       {/* Crop Area */}
                       <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
                          <span style={{ fontSize: '0.75rem', fontWeight: 600, color: '#64748b', textTransform: 'uppercase' }}>OCR’a gönderilen görüntü</span>
                          <div style={{ flex: 1, minHeight: '150px', background: '#f1f5f9', borderRadius: '0.5rem', border: '1px solid #cbd5e1', overflow: 'hidden', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                            <PdfPageViewer
                              imagePath={modelInputImage}
                              projectId={projectId}
                              pageNumber={record.questionNumber}
                              zoom={0.8}
                              overlayBox={overlayBox}
                              minimal={true}
                              emptyState={<div style={{ color: '#94a3b8', fontSize: '0.875rem' }}>{getStudentAnswerOcrPreviewMessage(record)}</div>}
                            />
                          </div>
                          <div style={{ fontSize: '0.75rem', color: '#64748b' }}>{preprocessSummary}</div>
                       </div>

                       {/* Text Area */}
                       <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
                          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                             <span style={{ fontSize: '0.75rem', fontWeight: 600, color: '#64748b', textTransform: 'uppercase' }}>OCR Çıktısı / Düzenleme</span>
                             <button
                               data-project-write="true"
                               onClick={() => saveMutation.mutate({ submissionId: record.submissionId, questionId: record.questionId, text: draftValue })}
                               disabled={isSaving}
                               style={{ fontSize: '0.75rem', color: '#2563eb', background: 'transparent', border: 'none', cursor: isSaving ? 'not-allowed' : 'pointer', fontWeight: 500 }}
                             >
                               {isSaving ? 'Kaydediliyor...' : 'Değişikliği Kaydet'}
                             </button>
                          </div>
                          <textarea
                            value={draftValue}
                            onChange={(e) => setDrafts((current) => ({ ...current, [key]: e.target.value }))}
                            style={{ flex: 1, minHeight: '150px', width: '100%', padding: '0.75rem', fontSize: '0.875rem', color: '#334155', border: '1px solid #cbd5e1', borderRadius: '0.5rem', resize: 'vertical', outline: 'none' }}
                          />
                       </div>
                    </div>

                    {/* Warnings & Diagnostics */}
                    <div style={{ padding: '0 1.25rem 1.25rem 1.25rem', display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
                           {(record.reviewReasons.length > 0 || record.warnings.length > 0 || (record.preprocessWarnings?.length ?? 0) > 0) && (
                         <div style={{ background: '#fffbeb', border: '1px solid #fde68a', borderRadius: '0.5rem', padding: '0.75rem', fontSize: '0.875rem', color: '#92400e' }}>
                            <div style={{ fontWeight: 600, marginBottom: '0.25rem', display: 'flex', alignItems: 'center', gap: '0.25rem' }}>
                              <AlertTriangle size={14} /> İnceleme Uyarıları:
                            </div>
                            <ul style={{ margin: 0, paddingLeft: '1.5rem' }}>
                               {[...record.reviewReasons, ...record.warnings, ...(record.preprocessWarnings ?? [])].map(w => (
                                 <li key={w}>{friendlyWarning(w, record.reviewPolicy ?? ocrReadiness?.ocrReviewPolicy)}</li>
                               ))}
                               {record.renderDiagnostics?.printedQuestionLeakDetected && <li>{friendlyWarning('printed_question_leak_detected', record.reviewPolicy ?? ocrReadiness?.ocrReviewPolicy)}</li>}
                             </ul>
                         </div>
                       )}

                       {getStudentAnswerOcrUncertaintySummary(record) && (
                         <div style={{ background: '#fff7ed', border: '1px solid #fdba74', borderRadius: '0.5rem', padding: '0.75rem', fontSize: '0.875rem', color: '#9a3412' }}>
                           <div style={{ fontWeight: 600, marginBottom: '0.25rem', display: 'flex', alignItems: 'center', gap: '0.25rem' }}>
                             <AlertTriangle size={14} /> Kritik Terim Belirsizliği
                           </div>
                           <div style={{ marginBottom: '0.5rem' }}>{getStudentAnswerOcrUncertaintySummary(record)}</div>
                           {record.uncertainSpans.length > 0 && (
                             <ul style={{ margin: 0, paddingLeft: '1.5rem' }}>
                               {record.uncertainSpans.map((span, index) => (
                                 <li key={`${span.text}-${index}`}>
                                   {span.text}
                                   {span.alternatives.length > 0 ? ` → ${span.alternatives.join(', ')}` : ''}
                                   {span.reason ? ` (${friendlyWarning(span.reason, record.reviewPolicy ?? ocrReadiness?.ocrReviewPolicy)})` : ''}
                                 </li>
                               ))}
                             </ul>
                           )}
                           {record.suggestedCorrections.length > 0 && (
                             <div style={{ marginTop: '0.5rem' }}>
                               <strong>Önerili düzeltmeler:</strong>
                               <ul style={{ margin: '0.25rem 0 0 0', paddingLeft: '1.5rem' }}>
                                 {record.suggestedCorrections.map((item, index) => (
                                   <li key={`${item.originalText}-${index}`}>
                                     {item.originalText} → {item.suggestedText}
                                     {item.reason ? ` (${friendlyWarning(item.reason, record.reviewPolicy ?? ocrReadiness?.ocrReviewPolicy)})` : ''}
                                   </li>
                                 ))}
                               </ul>
                             </div>
                           )}
                         </div>
                       )}

                       {record.ocrProvenance && (
                         <div style={{ fontSize: '0.8125rem', color: '#475569' }}>
                           OCR kaynağı: {record.ocrProvenance.sourcePageNumbers.length} sayfa · {record.ocrProvenance.regions.length} cevap bölgesi · {record.ocrProvenance.approvableForScoring ? 'notlandırma akışına uygun' : 'yalnızca inceleme'}
                         </div>
                       )}

                       <details style={{ fontSize: '0.8125rem' }}>
                         <summary style={{ cursor: 'pointer', color: '#64748b', fontWeight: 500 }}>Geliştirici provenance ayrıntıları</summary>
                         <pre style={{ marginTop: '0.5rem', maxHeight: '16rem', overflow: 'auto', background: '#0f172a', color: '#e2e8f0', borderRadius: '0.5rem', padding: '0.75rem', fontSize: '0.7rem' }}>
                           {JSON.stringify(record.ocrProvenance ?? { metadata: 'Bilinmiyor' }, null, 2)}
                         </pre>
                       </details>

                       <details style={{ fontSize: '0.8125rem' }}>
                         <summary style={{ cursor: 'pointer', color: '#64748b', fontWeight: 500 }}>Görüntü karşılaştırması</summary>
                         <div style={{ marginTop: '0.5rem', background: '#f8fafc', padding: '1rem', borderRadius: '0.5rem', border: '1px solid #e2e8f0', color: '#475569', overflowX: 'auto' }}>
                           {(originalCropImage || modelInputImage) && (
                             <div style={{ marginBottom: '1rem', display: 'grid', gap: '0.75rem' }}>
                               <strong>Görüntü Karşılaştırması:</strong>
                               <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
                                 {(['original', ...availableVariants] as const).map((candidate) => (
                                   <button
                                     key={candidate}
                                     type="button"
                                     data-project-write="false"
                                     onClick={() => setComparisonModes((current) => ({ ...current, [record.id]: candidate }))}
                                     style={{
                                       padding: '0.35rem 0.6rem',
                                       borderRadius: '999px',
                                       border: selectedComparisonMode === candidate ? '2px solid #2563eb' : '1px solid #cbd5e1',
                                       background: selectedComparisonMode === candidate ? '#eff6ff' : 'white',
                                       fontSize: '0.75rem',
                                     }}
                                   >
                                     {candidate === 'original'
                                       ? ocrPreprocessModeLabels.original
                                       : ocrPreprocessModeLabels[candidate] ?? candidate}
                                   </button>
                                 ))}
                               </div>
                               <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '0.75rem' }}>
                                 <div style={{ display: 'grid', gap: '0.35rem' }}>
                                   <div style={{ fontSize: '0.75rem', color: '#64748b', fontWeight: 600 }}>Orijinal crop</div>
                                   <PdfPageViewer
                                     imagePath={originalCropImage}
                                     projectId={projectId}
                                     pageNumber={record.questionNumber}
                                     zoom={0.8}
                                     minimal
                                     emptyState={<div style={{ color: '#94a3b8', fontSize: '0.75rem' }}>Orijinal crop yok.</div>}
                                   />
                                 </div>
                                 <div style={{ display: 'grid', gap: '0.35rem' }}>
                                   <div style={{ fontSize: '0.75rem', color: '#64748b', fontWeight: 600 }}>{selectedComparisonLabel}</div>
                                   <PdfPageViewer
                                     imagePath={selectedComparisonImage}
                                     projectId={projectId}
                                     pageNumber={record.questionNumber}
                                     zoom={0.8}
                                     minimal
                                     emptyState={<div style={{ color: '#94a3b8', fontSize: '0.75rem' }}>Preprocess önizlemesi yok.</div>}
                                   />
                                 </div>
                               </div>
                             </div>
                           )}
                         </div>
                       </details>
                    </div>

                  </div>
                )
             })}
          </div>
        </div>
      </div>
    </div>
  );
}

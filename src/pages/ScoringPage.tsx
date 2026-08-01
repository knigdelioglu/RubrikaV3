import { useEffect, useMemo, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AlertCircle, CheckCircle2, Eye, Loader2, RefreshCw, Sparkles } from 'lucide-react';
import { commands } from '../api/commands';
import type { AppError } from '../api/errors';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { ProjectContextState } from '../components/common/ProjectContextState';
import { tauriClient } from '../api/tauriClient';
import { blockingReasonLabels, jobStatusLabels, scoringWarningLabels } from '../utils/labels';
import { useProjectContext } from '../state/useProjectContext';
import type { ScoringRecord } from '../api/types';
import { ClassSelector } from '../components/student/ClassSelector';
import {
  buildStudentSummary,
  dedupeScoringRecords,
  getReviewStatusLabel,
  getSubmissionSortKey,
  resolveActiveScoringRunId,
} from './scoringViewModel';
import {
  filterStudentSubmissions,
  getSubmissionClassName,
  getStudentTeacherLabel,
} from './studentOperations';

function labelOrFallback(code: string, mapping: Record<string, string>): string {
  return mapping[code] ?? 'Notlandırma için bir hazırlık adımı tamamlanmalı.';
}

function scoringIssueLabel(code: string): string {
  return scoringWarningLabels[code] ?? 'Model çıktısı ek öğretmen kontrolü gerektiriyor.';
}

function formatScore(value: number | null | undefined): string {
  if (value === null || value === undefined || Number.isNaN(value)) return '-';
  return value.toFixed(2).replace(/\.00$/, '');
}

function formatConfidence(value: number | null | undefined): string {
  if (value === null || value === undefined || Number.isNaN(value)) return 'Bilinmiyor';
  return `%${Math.round(Math.max(0, Math.min(1, value)) * 100)}`;
}

export function ScoringPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const { projectId, projectPath, isResolving } = useProjectContext();
  const queryClient = useQueryClient();
  const [error, setError] = useState<AppError | null>(null);
  const [draftScores, setDraftScores] = useState<Record<string, { score: string; notes: string }>>({});
  const [expandedSubmissionIds, setExpandedSubmissionIds] = useState<Set<string>>(new Set());

  const { data: activity } = useQuery({
    queryKey: ["assessment-activity", projectId, searchParams.get("assessmentActivityId") || ""],
    queryFn: () => commands.getAssessmentActivity({ projectId: projectId!, activityId: searchParams.get("assessmentActivityId")! }),
    enabled: !!projectId && !!searchParams.get("assessmentActivityId"),
  });

  const { data: project, error: projectError } = useQuery({
    queryKey: ['project-snapshot', projectId],
    queryFn: () => commands.getProjectSnapshot(projectId!),
    enabled: !!projectId,
  });

  const { data: workflow, error: workflowError } = useQuery({
    queryKey: ['workflow-snapshot', projectId],
    queryFn: () => commands.getWorkflowSnapshot(projectId!),
    enabled: !!projectId,
  });

  const { data: jobs = [] } = useQuery({
    queryKey: ['jobs', projectId],
    queryFn: () => commands.listJobs(projectId!),
    enabled: !!projectId,
    refetchInterval: (query) => {
      const active = query.state.data?.some((job) => job.status === 'queued' || job.status === 'running');
      return active ? 1000 : false;
    },
  });

  const { data: modelStatus } = useQuery({
    queryKey: ['model-status'],
    queryFn: () => commands.getModelStatus(),
  });

  const startMutation = useMutation({
    mutationFn: (forceRerun: boolean) => commands.startScoringJob({ projectId: projectId!, forceRerun }),
    onMutate: () => setError(null),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['graded-exam-review', projectId] });
      queryClient.invalidateQueries({ queryKey: ['jobs', projectId] });
    },
    onError: (err: AppError) => setError(err),
  });

  const saveMutation = useMutation({
    mutationFn: (input: { recordId: string; score: string; notes: string; approved: boolean }) =>
      commands.updateScoringRecord({
        projectId: projectId!,
        recordId: input.recordId,
        teacherManualScore: input.score.trim() === '' ? null : Number(input.score),
        teacherNotes: input.notes.trim() === '' ? null : input.notes,
        teacherApproved: input.approved,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['graded-exam-review', projectId] });
    },
    onError: (err: AppError) => setError(err),
  });

  useEffect(() => {
    if (!project) return;
    setDraftScores((current) => {
      const next = { ...current };
      for (const record of project.scoringRecords) {
        if (!next[record.id]) {
          next[record.id] = {
            score: String(record.teacherManualScore ?? record.awardedScore ?? ''),
            notes: record.teacherNotes ?? '',
          };
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

  const requestedClassId = searchParams.get('classId') || '';
  const selectedClassId = requestedClassId && project?.schoolClasses.some((item) => item.id === requestedClassId)
    ? requestedClassId
    : '';

  useEffect(() => {
    if (!requestedClassId || requestedClassId === selectedClassId) return;
    const next = new URLSearchParams(searchParams);
    next.delete('classId');
    setSearchParams(next, { replace: true });
  }, [requestedClassId, searchParams, selectedClassId, setSearchParams]);

  const activeScoringRunId = useMemo(() => resolveActiveScoringRunId(project), [project]);
  const visibleSubmissions = useMemo(
    () => filterStudentSubmissions(project?.studentSubmissions ?? [], selectedClassId, ''),
    [project?.studentSubmissions, selectedClassId],
  );
  const visibleSubmissionIds = useMemo(
    () => new Set(visibleSubmissions.map((submission) => submission.id)),
    [visibleSubmissions],
  );
  const scoringRecords = useMemo(() => {
    const records = project?.scoringRecords ?? [];
    const activeRecords = activeScoringRunId ? records.filter((record) => record.runId === activeScoringRunId) : records;
    return dedupeScoringRecords(activeRecords.filter((record) => visibleSubmissionIds.has(record.submissionId)));
  }, [project, activeScoringRunId, visibleSubmissionIds]);
  const scoringJobActive = !!jobs.find((job) => job.kind === 'scoring' && (job.status === 'queued' || job.status === 'running'));
  const studentRows = useMemo(() => {
    if (!project) return [];
    const grouped = new Map<string, ScoringRecord[]>();
    for (const record of scoringRecords) {
      const list = grouped.get(record.submissionId) ?? [];
      list.push(record);
      grouped.set(record.submissionId, list);
    }
    const submissions = [...visibleSubmissions].sort((left, right) => getSubmissionSortKey(project, left).localeCompare(getSubmissionSortKey(project, right)));
    return submissions.map((submission) => buildStudentSummary(project, submission, grouped.get(submission.id) ?? []));
  }, [project, scoringRecords, visibleSubmissions]);
  const studentGroups = useMemo(() => {
    if (!project) return [];
    const grouped = new Map<string, typeof studentRows>();
    for (const row of studentRows) {
      const className = getSubmissionClassName(project, row.submission);
      const rows = grouped.get(className) ?? [];
      rows.push(row);
      grouped.set(className, rows);
    }
    return [...grouped.entries()]
      .sort(([left], [right]) => left.localeCompare(right, 'tr'))
      .map(([className, rows]) => ({ className, rows }));
  }, [project, studentRows]);

  if (isResolving) {
    return <ProjectContextState pageLabel="Notlandırma" loading projectPath={projectPath} />;
  }

  if (!projectId || !project) {
    return <ProjectContextState pageLabel="Notlandırma" projectPath={projectPath} />;
  }

  const queryError = (projectError as AppError | null) || (workflowError as AppError | null) || error;
  const scoringReady = workflow?.summary.readiness.scoring ?? false;
  const scoringBlockers = workflow?.blockingReasons ?? [];
  const activeJob = jobs.find((job) => job.kind === 'scoring' && (job.status === 'queued' || job.status === 'running'));
  const totalHistoryCount = project.scoringRecords.filter((record) => visibleSubmissionIds.has(record.submissionId)).length;
  const duplicateResultCount = Math.max(0, totalHistoryCount - scoringRecords.length);
  const activeStudentCount = studentRows.filter((row) => row.records.length > 0).length;

  const totalRecordCount = scoringRecords.length;
  const approvedCount = scoringRecords.filter((record) => record.teacherReviewStatus === 'approved' || record.teacherReviewStatus === 'edited').length;
  const needsReviewCount = scoringRecords.filter((record) => record.needsReview).length;
  const activeRunQuestionCount = scoringRecords.length;

  const handleStart = async (forceRerun: boolean) => {
    if (!scoringReady || startMutation.isPending || scoringJobActive) return;
    if (forceRerun && !window.confirm('Mevcut notlandırma sonuçları yeniden üretilecek. Onaylı manuel düzeltmelerin bir kısmı güncellenebilir. Devam edilsin mi?')) {
      return;
    }
    await startMutation.mutateAsync(forceRerun);
  };

  const handleSaveRecord = async (record: ScoringRecord, approved: boolean) => {
    const draft = draftScores[record.id] ?? { score: '', notes: '' };
    const trimmedScore = draft.score.trim();
    if (trimmedScore !== '' && Number.isNaN(Number(trimmedScore))) {
      setError({
        code: 'UNKNOWN_ERROR',
        safeMessage: 'Manuel puan sayısal olmalı.',
        retryable: true,
        correlationId: crypto.randomUUID?.() || 'unknown',
        detailsAvailable: false,
      });
      return;
    }
    await saveMutation.mutateAsync({
      recordId: record.id,
      score: trimmedScore,
      notes: draft.notes,
      approved,
    });
  };



  return (
    <div style={{ padding: '2rem', maxWidth: '1440px', margin: '0 auto', fontFamily: 'system-ui, -apple-system, sans-serif' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', marginBottom: '1.25rem', fontSize: '0.875rem' }}>
        <Link to={`/project/${encodeURIComponent(projectId)}/overview`} style={{ color: '#64748b', textDecoration: 'none' }}>İş Akışı</Link>
        <span style={{ color: '#cbd5e1' }}>/</span>
        <span style={{ color: '#0f172a', fontWeight: 600 }}>Notlandırma</span>
      </div>

      <div style={{ display: 'flex', justifyContent: 'space-between', gap: '1rem', alignItems: 'flex-start', flexWrap: 'wrap', marginBottom: '1.5rem' }}>
        <div>
          <h2 style={{ fontSize: '1.75rem', fontWeight: 800, color: '#0f172a', margin: 0 }}>Notlandırma</h2>
          <p style={{ margin: '0.35rem 0 0 0', color: '#64748b', fontSize: '0.875rem' }}>
            Öğretmen onaylı OCR, doğrulanmış kimlik ve dondurulmuş sınav paketiyle notlandırma işlemi çalıştırılır.
          </p>
          <p style={{ margin: "0.35rem 0 0 0", color: "#64748b", fontSize: "0.875rem" }}>{activity ? `Yazılı · ${activity.classApplications.map((a) => a.schoolClassId).join(", ")}` : `Proje: ${project.name}`}</p>
        </div>

        <div style={{ display: 'flex', gap: '0.75rem', flexWrap: 'wrap', alignItems: 'center' }}>
          <button
            type="button"
            onClick={() => void handleStart(false)}
            disabled={!scoringReady || startMutation.isPending || scoringJobActive}
            title={
              !scoringReady
                ? 'Notlandırma için gereken hazırlıklar henüz tamamlanmadı.'
                : scoringJobActive
                  ? 'Çalışan notlandırma işlemi tamamlanmadan yenisi başlatılamaz.'
                  : undefined
            }
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              gap: '0.5rem',
              padding: '0.75rem 1rem',
              borderRadius: '0.75rem',
              border: '1px solid #0f172a',
              background: (!scoringReady || startMutation.isPending || scoringJobActive) ? '#e2e8f0' : '#0f172a',
              color: (!scoringReady || startMutation.isPending || scoringJobActive) ? '#64748b' : 'white',
              fontWeight: 700,
              cursor: (!scoringReady || startMutation.isPending || scoringJobActive) ? 'not-allowed' : 'pointer',
            }}
          >
            {(startMutation.isPending || scoringJobActive) && !activeJob ? <Loader2 size={16} className="animate-spin" /> : <Sparkles size={16} />}
            Notlandırmayı Başlat
          </button>

          <button
            type="button"
            onClick={() => void handleStart(true)}
            disabled={!scoringReady || startMutation.isPending || scoringJobActive || project.scoringRecords.length === 0}
            title={
              !scoringReady
                ? 'Backend gate scoring için hazır değil.'
                : scoringJobActive
                  ? 'Çalışan notlandırma işlemi tamamlanmadan yeniden çalıştırılamaz.'
                  : project.scoringRecords.length === 0
                    ? 'Yeniden çalıştırmak için mevcut notlandırma sonucu gerekir.'
                    : undefined
            }
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              gap: '0.5rem',
              padding: '0.75rem 1rem',
              borderRadius: '0.75rem',
              border: '1px solid #cbd5e1',
              background: (!scoringReady || startMutation.isPending || scoringJobActive || project.scoringRecords.length === 0) ? '#f8fafc' : 'white',
              color: (!scoringReady || startMutation.isPending || scoringJobActive || project.scoringRecords.length === 0) ? '#94a3b8' : '#0f172a',
              fontWeight: 700,
              cursor: (!scoringReady || startMutation.isPending || scoringJobActive || project.scoringRecords.length === 0) ? 'not-allowed' : 'pointer',
            }}
          >
            <RefreshCw size={16} />
            Notlandırmayı Yeniden Çalıştır
          </button>

        </div>
      </div>

      <div className="scoring-class-filter">
        <ClassSelector
          idPrefix="scoring"
          classes={project.schoolClasses}
          classId={selectedClassId}
          onClassChange={(classId) => {
            const next = new URLSearchParams(searchParams);
            if (classId) next.set('classId', classId); else next.delete('classId');
            setSearchParams(next);
          }}
        />
        <p>Filtre, öğrenci kartlarını ve bu ekrandaki toplamları sınırlar. Notlandırma işlemi proje genelinde çalışır.</p>
      </div>

      {queryError && <ErrorBanner error={queryError} />}

      {activeJob ? (
        <div style={{ background: '#eef2ff', border: '1px solid #c7d2fe', borderRadius: '1rem', padding: '1.25rem', marginBottom: '1.5rem' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '0.5rem' }}>
            <strong style={{ color: '#3730a3', fontSize: '1.1rem' }}>
              Puanlama sürüyor — {activeJob.progress.current} / {activeJob.progress.total} öğrenci
            </strong>
            <span style={{ fontSize: '0.875rem', fontWeight: 600, color: '#4338ca' }}>
              %{Math.round((activeJob.progress.current / (activeJob.progress.total || 1)) * 100)}
            </span>
          </div>
          <div style={{ height: '8px', background: '#c7d2fe', borderRadius: '9999px', overflow: 'hidden', marginBottom: '0.5rem' }}>
            <div style={{ width: `${Math.round((activeJob.progress.current / (activeJob.progress.total || 1)) * 100)}%`, height: '100%', background: '#4f46e5', transition: 'width 0.3s' }} />
          </div>
          <p style={{ margin: 0, color: '#4338ca', fontSize: '0.875rem' }}>{activeJob.progress.message || 'Puanlama modeli yanıtları üretiyor.'}</p>
        </div>
      ) : (
        !scoringReady ? (
          <div style={{ background: "#fff9eb", border: "1px solid #fde68a", borderRadius: "1rem", padding: "1.75rem", marginBottom: "1.5rem", textAlign: "center" }}>
            <h3 style={{ margin: 0, fontSize: "1.2rem", color: "#92400e", fontWeight: 700 }}>Puanlama henüz hazır değil</h3>
            <p style={{ margin: "0.5rem 0 1.25rem 0", color: "#b45309", fontSize: "0.9rem" }}>
              {(project?.studentAnswerOcrRecords ?? []).filter((r) => r.needsReview).length > 0
                ? `${(project?.studentAnswerOcrRecords ?? []).filter((r) => r.needsReview).length} öğrencinin OCR kontrolünü tamamlayın.`
                : "Puanlama başlatabilmek için önce sınav hazırlığı ve OCR kontrolü tamamlanmalıdır."}
            </p>
            <Link to={`/project/${encodeURIComponent(projectId)}/activities/${encodeURIComponent(searchParams.get("assessmentActivityId") || "")}/ocr`} className="button button--primary" style={{ display: "inline-flex", padding: "0.65rem 1.25rem" }}>
              OCR ve Kontrole git
            </Link>
          </div>
        ) : (
          <div style={{ background: "white", border: "1px solid #e2e8f0", borderRadius: "1rem", padding: "1.25rem", marginBottom: "1.5rem", display: "flex", justifyContent: "space-between", alignItems: "center", flexWrap: "wrap", gap: "1rem" }}>
            <div>
              <h3 style={{ margin: 0, fontSize: "1.25rem", fontWeight: 700, color: "#0f172a" }}>Puanlamaya hazır</h3>
              <p style={{ margin: "0.25rem 0 0", color: "#64748b", fontSize: "#0.875rem" }}>
                {project.students.length} öğrenci · {project.questions.length} soru
              </p>
            </div>
            <div>
              <button
                type="button"
                className="button button--primary"
                disabled={startMutation.isPending || scoringJobActive}
                onClick={() => void handleStart(false)}
                style={{ padding: "0.75rem 1.5rem", fontSize: "0.95rem" }}
              >
                <Sparkles size={18} /> Puanlamayı Başlat
              </button>
            </div>
          </div>
        )
      )}

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))', gap: '1rem', marginBottom: '1.5rem' }}>
        <div style={{ background: 'white', border: '1px solid #e2e8f0', borderRadius: '1rem', padding: '1rem' }}>
          <strong style={{ color: '#0f172a' }}>Sonuç özeti</strong>
          <div style={{ display: 'flex', gap: '0.75rem', marginTop: '0.75rem', flexWrap: 'wrap' }}>
            <span style={{ background: '#e0e7ff', color: '#3730a3', borderRadius: '9999px', padding: '0.35rem 0.75rem', fontSize: '0.8125rem', fontWeight: 700 }}>
              Aktif kayıt: {totalRecordCount}
            </span>
            <span style={{ background: '#dcfce7', color: '#166534', borderRadius: '9999px', padding: '0.35rem 0.75rem', fontSize: '0.8125rem', fontWeight: 700 }}>
              Onaylı: {approvedCount}
            </span>
            <span style={{ background: '#fef3c7', color: '#92400e', borderRadius: '9999px', padding: '0.35rem 0.75rem', fontSize: '0.8125rem', fontWeight: 700 }}>
              Kontrol: {needsReviewCount}
            </span>
            <span style={{ background: '#f1f5f9', color: '#334155', borderRadius: '9999px', padding: '0.35rem 0.75rem', fontSize: '0.8125rem', fontWeight: 700 }}>
              Geçmiş kayıt: {totalHistoryCount}
            </span>
            <span style={{ background: '#fff7ed', color: '#9a3412', borderRadius: '9999px', padding: '0.35rem 0.75rem', fontSize: '0.8125rem', fontWeight: 700 }}>
              Yinelenen: {duplicateResultCount}
            </span>
            <span style={{ background: '#f8fafc', color: '#334155', borderRadius: '9999px', padding: '0.35rem 0.75rem', fontSize: '0.8125rem', fontWeight: 700 }}>
              Öğrenci: {activeStudentCount}
            </span>
            <span style={{ background: '#f8fafc', color: '#334155', borderRadius: '9999px', padding: '0.35rem 0.75rem', fontSize: '0.8125rem', fontWeight: 700 }}>
              Soru: {activeRunQuestionCount}
            </span>
          </div>
        </div>
        <div style={{ background: 'white', border: '1px solid #e2e8f0', borderRadius: '1rem', padding: '1rem' }}>
          <strong style={{ color: '#0f172a' }}>Model durumu</strong>
          <p style={{ margin: '0.5rem 0 0 0', color: '#475569', fontSize: '0.875rem' }}>
            {modelStatus?.serverRunning ? 'Model sunucusu çalışıyor.' : 'Model kapalıysa backend scoring sırasında autostart dener.'}
          </p>
          <p style={{ margin: '0.35rem 0 0 0', color: '#64748b', fontSize: '0.8125rem' }}>
            {modelStatus?.healthOk ? 'Sağlık kontrolü açık.' : 'Sağlık kontrolü beklemede veya başarısız.'}
          </p>
        </div>
      </div>

      {scoringBlockers.length > 0 && (
        <div style={{ marginBottom: '1.5rem', padding: '1rem 1.25rem', borderRadius: '1rem', background: '#fef2f2', border: '1px solid #fecaca', color: '#991b1b' }}>
          <div style={{ display: 'flex', gap: '0.75rem', alignItems: 'center', marginBottom: '0.5rem' }}>
            <AlertCircle size={18} />
            <strong>Notlandırma engelleri</strong>
          </div>
          <ul style={{ margin: 0, paddingLeft: '1.5rem' }}>
            {scoringBlockers.map((blocker) => (
              <li key={blocker}>{labelOrFallback(blocker, blockingReasonLabels)}</li>
            ))}
          </ul>
        </div>
      )}

      {duplicateResultCount > 0 && (
        <div style={{ marginBottom: '1rem', padding: '0.9rem 1rem', borderRadius: '0.9rem', background: '#fff7ed', border: '1px solid #fed7aa', color: '#9a3412', fontSize: '0.875rem' }}>
          Aynı öğrenci ve soru için geçmişte birden fazla sonuç bulundu. Toplam puan yalnızca son notlandırma çalışmasından hesaplanıyor.
        </div>
      )}

      {scoringJobActive && activeJob && (
        <div style={{ marginBottom: '1rem', padding: '1rem 1.25rem', borderRadius: '1rem', background: '#eff6ff', border: '1px solid #bfdbfe', color: '#1d4ed8' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', gap: '1rem', alignItems: 'center', flexWrap: 'wrap' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
              <Loader2 size={18} className="animate-spin" />
              <strong>Notlandırma işlemi çalışıyor</strong>
            </div>
            <span style={{ fontSize: '0.875rem', fontWeight: 700 }}>{jobStatusLabels[activeJob.status] ?? activeJob.status}</span>
          </div>
          <p style={{ margin: '0.6rem 0 0 0', fontSize: '0.875rem' }}>{activeJob.progress.message}</p>
          <div style={{ marginTop: '0.75rem', height: '0.5rem', borderRadius: '9999px', background: '#dbeafe', overflow: 'hidden' }}>
            <div
              style={{
                width: activeJob.progress.total > 0 ? `${Math.min(100, (activeJob.progress.current / activeJob.progress.total) * 100)}%` : '0%',
                height: '100%',
                background: '#2563eb',
                transition: 'width 0.2s ease',
              }}
            />
          </div>
        </div>
      )}

      <div style={{ display: 'grid', gap: '1rem' }}>
        {studentRows.length === 0 ? (
          <div style={{ padding: '1rem', borderRadius: '1rem', background: 'white', border: '1px solid #e2e8f0', color: '#64748b' }}>
            {scoringRecords.length > 0 ? 'Aktif notlandırma sonuçları hazırlanıyor.' : 'Henüz notlandırılmadı.'}
          </div>
        ) : (
          studentGroups.map((group) => (
            <div key={group.className} className="scoring-class-group">
              {!selectedClassId && <h2>{group.className}</h2>}
              <div style={{ display: 'grid', gap: '1rem' }}>
              {group.rows.map((row) => {
            const studentLabel = getStudentTeacherLabel(row.student, group.className);
            const isExpanded = expandedSubmissionIds.has(row.submission.id);

            return (
              <section key={row.submission.id} style={{ background: 'white', border: '1px solid #e2e8f0', borderRadius: '1rem', overflow: 'hidden' }}>
                <button
                  type="button"
                  onClick={() => setExpandedSubmissionIds((current) => {
                    const next = new Set(current);
                    if (next.has(row.submission.id)) next.delete(row.submission.id);
                    else next.add(row.submission.id);
                    return next;
                  })}
                  aria-expanded={isExpanded}
                  style={{
                    width: '100%',
                    display: 'flex',
                    justifyContent: 'space-between',
                    gap: '1rem',
                    alignItems: 'flex-start',
                    padding: '1.1rem 1.25rem',
                    border: 'none',
                    background: isExpanded ? '#f8fafc' : 'white',
                    cursor: 'pointer',
                    textAlign: 'left',
                  }}
                >
                  <div style={{ minWidth: 0 }}>
                    <h3 style={{ margin: 0, color: '#0f172a', fontSize: '1.05rem' }}>
                      {studentLabel}
                    </h3>
                    <p style={{ margin: '0.35rem 0 0 0', color: '#64748b', fontSize: '0.875rem' }}>
                      Sayfalar: {row.submission.pageNumbers.join(', ') || '-'}
                    </p>
                    <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.45rem', marginTop: '0.75rem' }}>
                      {row.badges.map((badge) => (
                        <span
                          key={badge}
                          style={{
                            fontSize: '0.75rem',
                            fontWeight: 700,
                            padding: '0.3rem 0.6rem',
                            borderRadius: '9999px',
                            background:
                              badge === 'İnceleme gerekli'
                                ? '#fef3c7'
                                : badge === 'Uyarı var'
                                  ? '#fff7ed'
                                  : badge === 'Onaylandı'
                                    ? '#dcfce7'
                                    : badge === 'Onay bekliyor'
                                      ? '#e0e7ff'
                                      : '#f8fafc',
                            color:
                              badge === 'İnceleme gerekli'
                                ? '#92400e'
                                : badge === 'Uyarı var'
                                  ? '#9a3412'
                                  : badge === 'Onaylandı'
                                    ? '#166534'
                                    : badge === 'Onay bekliyor'
                                      ? '#3730a3'
                                      : '#475569',
                          }}
                        >
                          {badge}
                        </span>
                      ))}
                    </div>
                  </div>

                  <div style={{ textAlign: 'right', flexShrink: 0 }}>
                    <div style={{ fontSize: '0.8rem', color: '#64748b' }}>{row.isComplete ? 'Toplam / Maksimum' : 'Geçici toplam (tamamlanmadı) / Maksimum'}</div>
                    <div style={{ fontSize: '1.45rem', fontWeight: 800, color: '#0f172a' }}>
                      {formatScore(row.totalScore)} / {formatScore(row.maxScore)}
                    </div>
                    <div style={{ marginTop: '0.35rem', fontSize: '0.8rem', color: row.needsReview ? '#92400e' : '#475569', fontWeight: 700 }}>
                      {row.reviewLabel}
                    </div>
                    <div style={{ marginTop: '0.25rem', fontSize: '0.75rem', color: '#64748b' }}>
                      Uyarı: {row.warningCount} · Öğretmen onayı: {row.approvedCount}/{row.records.length}
                    </div>
                    {row.unscoredCount > 0 && (
                      <div style={{ marginTop: '0.25rem', fontSize: '0.75rem', color: '#b91c1c', fontWeight: 700 }}>
                        Puan uygulanmayan soru: {row.unscoredCount}
                      </div>
                    )}
                    {row.duplicateCount > 0 && (
                      <div style={{ marginTop: '0.25rem', fontSize: '0.75rem', color: '#9a3412', fontWeight: 700 }}>
                        Yinelenen tarihsel kayıt: {row.duplicateCount}
                      </div>
                    )}
                  </div>
                </button>

                {isExpanded && (
                  <div style={{ padding: '0 1.25rem 1.25rem 1.25rem' }}>
                    {row.records.length > 0 && (
                      <div style={{ display: 'flex', justifyContent: 'flex-end', padding: '0.9rem 0', borderBottom: '1px solid #e2e8f0', marginBottom: '0.9rem' }}>
                        <Link
                          to={`/graded-exam-review?projectId=${encodeURIComponent(projectId)}&submissionId=${encodeURIComponent(row.submission.id)}`}
                          title="Öğrencinin sınav kâğıdını model puanları soru alanlarına yerleştirilmiş olarak aç"
                          style={{ display: 'inline-flex', alignItems: 'center', gap: '0.5rem', padding: '0.65rem 0.85rem', borderRadius: '0.65rem', border: '1px solid #fecaca', background: '#fef2f2', color: '#b91c1c', fontWeight: 800, textDecoration: 'none' }}
                        >
                          <Eye size={17} />
                          Kâğıt üzerinde incele
                        </Link>
                      </div>
                    )}
                    {row.records.length === 0 ? (
                      <div style={{ padding: '1rem', borderRadius: '0.75rem', background: '#f8fafc', color: '#64748b' }}>
                        Bu öğrenci için aktif scoring sonucu yok.
                      </div>
                    ) : (
                      <div style={{ display: 'grid', gap: '0.75rem' }}>
                        {row.records.map((record) => {
                          const draft = draftScores[record.id] ?? { score: String(record.teacherManualScore ?? record.awardedScore ?? ''), notes: record.teacherNotes ?? '' };
                          const effectiveScore = record.teacherManualScore ?? record.awardedScore;

                          return (
                            <article key={record.id} style={{ border: '1px solid #e2e8f0', borderRadius: '0.9rem', padding: '1rem', background: record.needsReview ? '#fffbeb' : 'white' }}>
                              <div style={{ display: 'flex', justifyContent: 'space-between', gap: '1rem', alignItems: 'flex-start', flexWrap: 'wrap' }}>
                                <div>
                                  <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', flexWrap: 'wrap' }}>
                                    <strong style={{ color: '#0f172a' }}>Soru {record.questionNumber}</strong>
                                    <span style={{ fontSize: '0.75rem', fontWeight: 700, padding: '0.25rem 0.55rem', borderRadius: '9999px', background: record.needsReview ? '#fef3c7' : '#dcfce7', color: record.needsReview ? '#92400e' : '#166534' }}>
                                      {!record.scoringApplied ? 'Puan uygulanmadı' : record.needsReview ? 'İnceleme gerekli' : 'Finale yakın'}
                                    </span>
                                    <span style={{ fontSize: '0.75rem', color: '#64748b' }}>
                                      Öğretmen durumu: {getReviewStatusLabel(record.teacherReviewStatus)}
                                    </span>
                                  </div>
                                  <p style={{ margin: '0.35rem 0 0 0', color: '#475569', fontSize: '0.875rem' }}>
                                    Model güven göstergesi: {formatConfidence(record.confidence)}
                                  </p>
                                </div>
                                <div style={{ textAlign: 'right' }}>
                                  <div style={{ fontSize: '0.8rem', color: '#64748b' }}>{record.scoringApplied ? 'Puan' : 'Öğretmen puanı bekleniyor'}</div>
                                  <div style={{ fontSize: '1.4rem', fontWeight: 800, color: '#0f172a' }}>
                                    {formatScore(effectiveScore)} / {formatScore(record.maxScore)}
                                  </div>
                                </div>
                              </div>

                              {!record.scoringApplied && (
                                <div style={{ marginTop: '0.9rem', padding: '0.8rem', borderRadius: '0.75rem', background: '#fef2f2', border: '1px solid #fecaca', color: '#991b1b', fontSize: '0.875rem' }}>
                                  <strong>Bu kayıt öğrenci toplamına sıfır olarak eklenmedi.</strong> Model sonucu güvenilir biçimde üretilemedi. Manuel puan girerek kaydı tamamlayın.
                                </div>
                              )}

                              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))', gap: '0.75rem', marginTop: '1rem' }}>
                                <label style={{ display: 'flex', flexDirection: 'column', gap: '0.35rem', fontSize: '0.875rem', color: '#334155' }}>
                                  Manuel puan
                                  <input
                                    type="number"
                                    step="0.01"
                                    min="0"
                                    max={record.maxScore}
                                    value={draft.score}
                                    onChange={(event) => {
                                      const value = event.target.value;
                                      setDraftScores((current) => ({
                                        ...current,
                                        [record.id]: { ...draft, score: value },
                                      }));
                                    }}
                                    style={{ padding: '0.6rem 0.75rem', borderRadius: '0.5rem', border: '1px solid #cbd5e1' }}
                                  />
                                </label>

                                <label style={{ display: 'flex', flexDirection: 'column', gap: '0.35rem', fontSize: '0.875rem', color: '#334155' }}>
                                  Öğretmen notu
                                  <input
                                    type="text"
                                    value={draft.notes}
                                    onChange={(event) => {
                                      const value = event.target.value;
                                      setDraftScores((current) => ({
                                        ...current,
                                        [record.id]: { ...draft, notes: value },
                                      }));
                                    }}
                                    placeholder="Kısa açıklama"
                                    style={{ padding: '0.6rem 0.75rem', borderRadius: '0.5rem', border: '1px solid #cbd5e1' }}
                                  />
                                </label>
                              </div>

                              <div style={{ display: 'flex', gap: '0.5rem', marginTop: '0.9rem', flexWrap: 'wrap' }}>
                                <button
                                  type="button"
                                  onClick={() => void handleSaveRecord(record, false)}
                                  disabled={saveMutation.isPending || (!record.scoringApplied && draft.score.trim() === '')}
                                  style={{ padding: '0.55rem 0.85rem', borderRadius: '0.6rem', border: '1px solid #cbd5e1', background: saveMutation.isPending ? '#f8fafc' : '#0f172a', color: saveMutation.isPending ? '#94a3b8' : 'white', fontWeight: 700, cursor: saveMutation.isPending ? 'not-allowed' : 'pointer' }}
                                >
                                  Kaydet
                                </button>
                                <button
                                  type="button"
                                  onClick={() => void handleSaveRecord(record, true)}
                                  disabled={saveMutation.isPending || (!record.scoringApplied && draft.score.trim() === '')}
                                  style={{ padding: '0.55rem 0.85rem', borderRadius: '0.6rem', border: '1px solid #bbf7d0', background: (saveMutation.isPending || (!record.scoringApplied && draft.score.trim() === '')) ? '#f8fafc' : '#dcfce7', color: '#166534', fontWeight: 700, cursor: (saveMutation.isPending || (!record.scoringApplied && draft.score.trim() === '')) ? 'not-allowed' : 'pointer' }}
                                >
                                  <CheckCircle2 size={16} style={{ display: 'inline', marginRight: '0.35rem' }} />
                                  Onayla
                                </button>
                              </div>

                              {record.warnings.length > 0 && (
                                <div style={{ marginTop: '0.9rem', padding: '0.75rem', borderRadius: '0.75rem', background: '#fff7ed', border: '1px solid #fed7aa', color: '#9a3412', fontSize: '0.875rem' }}>
                                  <strong style={{ display: 'block', marginBottom: '0.35rem' }}>Uyarılar</strong>
                                  <ul style={{ margin: 0, paddingLeft: '1.25rem' }}>
                                    {record.warnings.map((warning) => (
                                      <li key={warning}>{scoringIssueLabel(warning)}</li>
                                    ))}
                                  </ul>
                                </div>
                              )}

                              {record.reviewReasons.length > 0 && (
                                <div style={{ marginTop: '0.9rem', padding: '0.75rem', borderRadius: '0.75rem', background: '#fffbeb', border: '1px solid #fde68a', color: '#92400e', fontSize: '0.875rem' }}>
                                  <strong style={{ display: 'block', marginBottom: '0.35rem' }}>Neden kontrol edilmeli?</strong>
                                  <ul style={{ margin: 0, paddingLeft: '1.25rem' }}>
                                    {record.reviewReasons.map((reason) => <li key={reason}>{scoringIssueLabel(reason)}</li>)}
                                  </ul>
                                </div>
                              )}

                              {record.reconciliationDiagnostics?.notes?.length ? (
                                <div style={{ marginTop: '0.9rem', padding: '0.75rem', borderRadius: '0.75rem', background: '#eff6ff', border: '1px solid #bfdbfe', color: '#1d4ed8', fontSize: '0.875rem' }}>
                                  <strong style={{ display: 'block', marginBottom: '0.35rem' }}>Puan doğrulama notu</strong>
                                  <ul style={{ margin: 0, paddingLeft: '1.25rem' }}>
                                    {record.reconciliationDiagnostics.notes.map((note) => (
                                      <li key={note}>{note}</li>
                                    ))}
                                  </ul>
                                </div>
                              ) : null}

                              <div style={{ display: 'grid', gap: '0.6rem', marginTop: '0.9rem' }}>
                                {record.criterionScores.length > 0 && (
                                  <div style={{ borderTop: '1px solid #e2e8f0', paddingTop: '0.75rem' }}>
                                    <strong style={{ display: 'block', marginBottom: '0.5rem', color: '#0f172a' }}>Kriter puanları</strong>
                                    <div style={{ display: 'grid', gap: '0.5rem' }}>
                                      {record.criterionScores.map((criterion) => (
                                        <div key={criterion.criterionId} style={{ padding: '0.75rem', borderRadius: '0.75rem', background: '#f8fafc', border: '1px solid #e2e8f0' }}>
                                          <div style={{ display: 'flex', justifyContent: 'space-between', gap: '1rem', flexWrap: 'wrap' }}>
                                            <strong style={{ color: '#0f172a' }}>{criterion.criterionTitle}</strong>
                                            <span style={{ color: '#0f172a', fontWeight: 700 }}>{formatScore(criterion.awardedScore)} / {formatScore(criterion.criterionMaxScore)}</span>
                                          </div>
                                          <p style={{ margin: '0.35rem 0 0 0', color: '#475569', fontSize: '0.875rem' }}>{criterion.rationale}</p>
                                          {criterion.evidenceQuote ? (
                                            <p style={{ margin: '0.45rem 0 0 0', color: '#334155', fontSize: '0.8125rem' }}>
                                              <strong>Öğrenci cevabındaki kanıt:</strong> “{criterion.evidenceQuote}”
                                            </p>
                                          ) : null}
                                        </div>
                                      ))}
                                    </div>
                                  </div>
                                )}

                                <div style={{ borderTop: '1px solid #e2e8f0', paddingTop: '0.75rem' }}>
                                  <strong style={{ display: 'block', marginBottom: '0.35rem', color: '#0f172a' }}>Gerekçe</strong>
                                  <p style={{ margin: 0, color: '#475569' }}>{record.rationale}</p>
                                </div>
                              </div>
                            </article>
                          );
                        })}
                      </div>
                    )}
                  </div>
                )}
              </section>
            );
              })}
              </div>
            </div>
          ))
        )}
      </div>

      <details style={{ marginTop: '1rem', padding: '1rem', borderRadius: '1rem', background: '#f8fafc', border: '1px solid #e2e8f0' }}>
        <summary style={{ cursor: 'pointer', fontWeight: 700, color: '#475569' }}>Gelişmiş ayrıntılar</summary>
        <pre style={{ marginTop: '1rem', whiteSpace: 'pre-wrap', wordBreak: 'break-word', fontSize: '0.75rem', color: '#334155' }}>
          {JSON.stringify({
            scoringReady,
            blockers: scoringBlockers,
            jobs: jobs.map((job) => ({ id: job.id, kind: job.kind, status: job.status })),
            projectId,
            workflowStage: workflow?.currentStage,
            recordCount: scoringRecords.length,
            activeScoringRunId,
            historyCount: totalHistoryCount,
            duplicateResultCount,
            approvedCount,
            needsReviewCount,
          }, null, 2)}
        </pre>
      </details>
    </div>
  );
}

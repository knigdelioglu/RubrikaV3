import { useEffect, useMemo, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { BadgeCheck, ChevronRight, FileImage, PencilLine } from 'lucide-react';
import { commands } from '../api/commands';
import type { AppError } from '../api/errors';
import type {
  OcrSuggestedCorrection,
  OcrReviewPolicyDto,
  StudentAnswerOcrCropBBox,
  StudentAnswerOcrRecord,
  Student,
  SuggestStudentAnswerOcrIssueCorrectionWithModelOutput,
} from '../api/types';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { ProjectContextState } from '../components/common/ProjectContextState';
import { PdfPageViewer } from '../components/pdf/PdfPageViewer';
import { tauriClient } from '../api/tauriClient';
import { useProjectContext } from '../state/useProjectContext';
import {
  applyStudentAnswerOcrSuggestedCorrection,
  getStudentAnswerOcrActionableIssueEntriesForQuestion,
  getStudentAnswerOcrDraftText,
  getStudentAnswerOcrIssueReviewModelInputRef,
  getStudentAnswerOcrPreviewMessage,
  getStudentAnswerOcrPreprocessSummary,
  getStudentAnswerOcrTextHighlightsForQuestion,
  type StudentAnswerOcrIssueFilter,
  getStudentAnswerOcrTextHighlights,
} from './studentAnswerOcrUi';
import { blockingReasonLabels, ocrIssueTypeLabels, ocrWarningLabels, stageLabels, studentAnswerOcrStatusLabels } from '../utils/labels';
import { projectStudentOperationsPath } from '../app/projectRoutes';
import {
  filterStudentSubmissions,
  getSubmissionClassName,
  getStudentTeacherLabel,
} from './studentOperations';

type IssueRow = {
  record: StudentAnswerOcrRecord;
  issueId: string;
  issueKind: string;
  issueLabel: string;
  issueSummary: string;
  issueDisplayText: string;
  studentLabel: string;
  studentNumber: string;
  studentClassName: string;
  questionNumber: number;
  draftKey: string;
  modelInputRef: string | null;
  observedText: string;
  overlayBoxes: StudentAnswerOcrCropBBox[];
  textHighlights: ReturnType<typeof getStudentAnswerOcrTextHighlights>;
  suggestionText?: string | null;
  correction?: Pick<OcrSuggestedCorrection, 'originalText' | 'suggestedText'> | null;
  debugWarnings: string[];
};

type TextHighlight = ReturnType<typeof getStudentAnswerOcrTextHighlights>[number];

function recordKey(submissionId: string, questionId: string) {
  return `${submissionId}:${questionId}`;
}

function studentNumberLabel(student: Student | undefined) {
  return student?.number?.trim() || '-';
}

function warningMessage(code: string, policy?: OcrReviewPolicyDto | null) {
  const backendLabel = policy?.reasonLabels?.[code];
  if (backendLabel) return backendLabel;
  switch (code) {
    case 'preprocess_failed':
      return 'Görüntü ön hazırlığı başarısız oldu; orijinal crop kullanıldı.';
    case 'preprocess_fallback_used':
      return 'Ön hazırlık yedek moda düştü.';
    case 'parse_failed':
      return 'OCR çıktısı çözülemedi.';
    case 'critical_keyword_uncertain':
      return 'Kritik terim belirsiz.';
    case 'printed_question_leak_detected':
      return 'Soru kökü cevaba karışmış olabilir.';
    case 'printed_text_mixed':
      return 'Basılı metin karışmış olabilir.';
    case 'issue_context_missing':
      return 'İşaretli ifade için yeterli bağlam yok.';
    case 'scope_expansion_blocked':
      return 'Öneri kapsamı genişletildi; kontrol gerekli.';
    case 'visual_reading_unclear':
      return 'Görsel okuma net değil.';
    case 'suggestion_confidence_low':
      return 'Öneri güveni düşük.';
    case 'answer_crop_may_be_incomplete':
    case 'answer_crop_may_be_truncated':
      return 'Crop sınırı kontrol edilmeli.';
    case 'experimental_full_page_review_only':
      return 'Deneysel tam sayfa OCR yalnızca inceleme içindir; notlandırmaya onaylanamaz.';
    case 'structured_answer_invalid':
      return 'Yapısal OCR cevabı soru tipiyle doğrulanamadı; öğretmen kontrolü gerekli.';
    default:
      return ocrWarningLabels[code] ?? ocrIssueTypeLabels[code] ?? 'Ek OCR kontrolü gerekiyor.';
  }
}

function modelDecisionLabel(decision: string) {
  switch (decision) {
    case 'suggest_correction':
      return 'Düzeltme önerisi';
    case 'no_change':
      return 'Değişiklik yok';
    case 'needs_teacher_review':
      return 'Öğretmen incelemesi';
    default:
      return 'Öğretmen değerlendirmesi';
  }
}

function modelScopeLabel(scope: string) {
  switch (scope) {
    case 'single_word':
      return 'Tek kelime';
    case 'short_phrase':
      return 'Kısa ifade';
    default:
      return 'İşaretli ifade';
  }
}

function issueColor(kind: string) {
  switch (kind) {
    case 'suggested_correction':
      return { background: '#dcfce7', color: '#166534', border: '#86efac' };
    case 'critical_term_warning':
    case 'uncertain_span':
    case 'critical_keyword_uncertain':
      return { background: '#ffedd5', color: '#9a3412', border: '#fdba74' };
    case 'parse_warning':
    case 'printed_text_mixed':
    case 'answer_crop_may_be_truncated':
    case 'preprocess_warning':
      return { background: '#fee2e2', color: '#991b1b', border: '#fca5a5' };
    case 'ocr_low_confidence':
      return { background: '#fef3c7', color: '#92400e', border: '#fcd34d' };
    default:
      return { background: '#e0e7ff', color: '#3730a3', border: '#a5b4fc' };
  }
}

function issueTextColor(kind: string) {
  switch (kind) {
    case 'suggested_correction':
      return '#166534';
    case 'critical_term_warning':
    case 'uncertain_span':
    case 'critical_keyword_uncertain':
      return '#c2410c';
    case 'parse_warning':
    case 'printed_text_mixed':
    case 'answer_crop_may_be_truncated':
    case 'preprocess_warning':
      return '#b91c1c';
    case 'ocr_low_confidence':
      return '#92400e';
    default:
      return '#4338ca';
  }
}

function renderHighlightedText(text: string, highlights: TextHighlight[]) {
  if (!text.trim()) {
    return <span style={{ color: '#94a3b8' }}>OCR metni yok.</span>;
  }

  if (highlights.length === 0) {
    return <span>{text}</span>;
  }

  const segments: { text: string; kind?: string; label?: string; suggestionText?: string | null }[] = [];
  let cursor = 0;

  for (const highlight of highlights) {
    if (highlight.start > cursor) {
      segments.push({ text: text.slice(cursor, highlight.start) });
    }
    if (highlight.end > cursor) {
      segments.push({
        text: text.slice(highlight.start, highlight.end),
        kind: highlight.kind,
        label: highlight.label,
        suggestionText: highlight.suggestionText,
      });
      cursor = highlight.end;
    }
  }

  if (cursor < text.length) {
    segments.push({ text: text.slice(cursor) });
  }

  return (
    <span>
      {segments.map((segment, index) => {
        if (!segment.kind) {
          return <span key={`${segment.text}-${index}`}>{segment.text}</span>;
        }

        return (
          <span key={`${segment.text}-${index}`} style={{ display: 'inline-flex', alignItems: 'baseline', gap: '0.35rem', flexWrap: 'wrap' }}>
            <mark
              title={segment.label}
              style={{
                padding: '0.1rem 0.2rem',
                borderRadius: '0.35rem',
                background: issueColor(segment.kind).background,
                color: issueTextColor(segment.kind),
                border: `1px solid ${issueColor(segment.kind).border}`,
              }}
            >
              {segment.text}
            </mark>
            {segment.suggestionText && (
              <span style={{ fontSize: '0.75rem', color: '#64748b' }}>{segment.suggestionText}</span>
            )}
          </span>
        );
      })}
    </span>
  );
}

export function StudentAnswerOcrIssueReviewPage() {
  const [searchParams] = useSearchParams();
  const { projectId, projectPath, isResolving } = useProjectContext();
  const classId = searchParams.get('classId') || '';
  const batchId = searchParams.get('batchId') || '';
  const queryClient = useQueryClient();
  const [error, setError] = useState<AppError | null>(null);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [selectedIssueId, setSelectedIssueId] = useState<string | null>(null);
  const [selectedFilter, setSelectedFilter] = useState<StudentAnswerOcrIssueFilter>('pending_review');
  const [modelSuggestions, setModelSuggestions] = useState<Record<string, SuggestStudentAnswerOcrIssueCorrectionWithModelOutput>>({});

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
    refetchInterval: (query) => {
      const active = query.state.data?.some((job) => job.status === 'queued' || job.status === 'running');
      return active ? 1000 : false;
    },
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

  const rebuildMutation = useMutation({
    mutationFn: () => commands.rebuildStudentAnswerOcrIssues({ projectId: projectId! }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['jobs', projectId] });
    },
    onError: (err: AppError) => setError(err),
  });

  const suggestMutation = useMutation({
    mutationFn: (input: {
      issueId: string;
      projectPath: string;
      ocrRecordId: string;
      observedText: string;
      questionNumber: number;
      highlightRegion?: StudentAnswerOcrCropBBox | null;
      cropRef?: string | null;
      modelInputCropRef?: string | null;
    }) => commands.suggestOcrIssueCorrectionWithModel(input),
    onSuccess: (result, input) => {
      setModelSuggestions((current) => ({
        ...current,
        [input.issueId]: result,
      }));
    },
    onError: (err: AppError) => setError(err),
  });

  useEffect(() => {
    if (!project) return;
    setDrafts((current) => {
      const next = { ...current };
      for (const record of project.studentAnswerOcrRecords) {
        const key = recordKey(record.submissionId, record.questionId);
        if (next[key] === undefined) {
          next[key] = getStudentAnswerOcrDraftText(record);
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

  const studentById = useMemo(() => new Map(project?.students.map((student) => [student.id, student]) ?? []), [project]);
  const submissionById = useMemo(() => new Map(project?.studentSubmissions.map((submission) => [submission.id, submission]) ?? []), [project]);
  const questionById = useMemo(() => new Map(project?.questions.map((question) => [question.id, question]) ?? []), [project]);
  const visibleSubmissions = useMemo(
    () => filterStudentSubmissions(project?.studentSubmissions ?? [], classId, batchId),
    [batchId, classId, project?.studentSubmissions],
  );
  const visibleSubmissionIds = useMemo(
    () => new Set(visibleSubmissions.map((submission) => submission.id)),
    [visibleSubmissions],
  );

  const issueRows = useMemo<IssueRow[]>(() => {
    if (!project) return [];

    const rows = project.studentAnswerOcrRecords.flatMap((record) => {
      if (!visibleSubmissionIds.has(record.submissionId)) return [];
      const question = questionById.get(record.questionId);
      const entries = getStudentAnswerOcrActionableIssueEntriesForQuestion(record, question);
      if (entries.length === 0) return [];
      const submission = submissionById.get(record.submissionId);
      const student = submission ? studentById.get(submission.studentId) : undefined;
      const draftText = drafts[recordKey(record.submissionId, record.questionId)] ?? getStudentAnswerOcrDraftText(record);
      const modelInputRef = getStudentAnswerOcrIssueReviewModelInputRef(record);
      const canonicalClassName = submission ? getSubmissionClassName(project, submission) : 'Sınıfı belirlenmemiş';

      return entries.map((entry, entryIndex) => ({
        record,
        issueId: `${record.id}:${entry.kind}:${entryIndex}`,
        issueKind: entry.kind,
        issueLabel: entry.label,
        issueSummary: entry.summary,
        issueDisplayText: entry.suggestionText
          ? `${entry.originalText ?? entry.summary} → ${entry.suggestionText}`
          : entry.originalText ?? entry.summary,
        studentLabel: getStudentTeacherLabel(student, canonicalClassName),
        studentNumber: studentNumberLabel(student),
        studentClassName: canonicalClassName,
        questionNumber: record.questionNumber,
        draftKey: recordKey(record.submissionId, record.questionId),
        modelInputRef,
        observedText: entry.originalText ?? entry.suggestionText ?? entry.summary,
        overlayBoxes: entry.highlightRegion ? [entry.highlightRegion] : [],
        textHighlights: getStudentAnswerOcrTextHighlightsForQuestion(record, draftText, question),
        suggestionText: entry.suggestionText,
        correction: entry.suggestionText
          ? {
            originalText: entry.originalText ?? entry.suggestionText,
            suggestedText: entry.suggestionText,
          }
          : record.suggestedCorrections.find((item) => item.originalText === entry.originalText) ?? null,
        debugWarnings: [
          ...(record.criticalKeywordUncertain ? ['critical_keyword_uncertain'] : []),
          ...record.reviewReasons,
          ...record.warnings,
          ...(record.preprocessWarnings ?? []),
          ...(record.renderDiagnostics?.printedTextMixed ? ['printed_text_mixed'] : []),
          ...(record.renderDiagnostics?.partialAnswerSuspected &&
            (record.renderDiagnostics?.cropWasClamped || record.renderDiagnostics?.cropMarginApplied)
            ? ['answer_crop_may_be_truncated']
            : []),
        ],
      }));
    }).filter((row) => {
      if (selectedFilter === 'all') {
        return true;
      }
      if (selectedFilter === 'pending_review') {
        return row.record.needsReview && row.record.status !== 'teacher_approved';
      }
      if (selectedFilter === 'critical_term_uncertain') {
        return row.issueKind === 'critical_term_warning' || row.issueKind === 'uncertain_span' || row.issueKind === 'critical_keyword_uncertain';
      }
      if (selectedFilter === 'suggested_correction') {
        return row.issueKind === 'suggested_correction';
      }
      if (selectedFilter === 'ocr_low_confidence') {
        return row.record.needsReview && row.record.reviewReasons.includes('ocr_low_confidence');
      }
      if (selectedFilter === 'resolved') {
        return row.record.status === 'teacher_approved';
      }
      return true;
    });

    return rows.sort((left, right) => {
      const byStudent = left.studentLabel.localeCompare(right.studentLabel, 'tr');
      if (byStudent !== 0) return byStudent;
      const byNumber = left.studentNumber.localeCompare(right.studentNumber, 'tr', { numeric: true });
      if (byNumber !== 0) return byNumber;
      const byQuestion = left.questionNumber - right.questionNumber;
      if (byQuestion !== 0) return byQuestion;
      return left.issueLabel.localeCompare(right.issueLabel, 'tr');
    });
  }, [drafts, project, questionById, selectedFilter, studentById, submissionById, visibleSubmissionIds]);

  useEffect(() => {
    if (!issueRows.length) {
      setSelectedIssueId(null);
      return;
    }
    if (!selectedIssueId || !issueRows.some((row) => row.issueId === selectedIssueId)) {
      setSelectedIssueId(issueRows[0]?.issueId ?? null);
    }
  }, [issueRows, selectedIssueId]);

  if (isResolving) {
    return <ProjectContextState pageLabel="OCR Sorun İnceleme" loading projectPath={projectPath} />;
  }

  if (!projectId || !project) {
    return <ProjectContextState pageLabel="OCR Sorun İnceleme" projectPath={projectPath} />;
  }

  const queryError = (projectError as AppError | null) || error;
  const activeJob = jobs.find((job) => job.kind === 'student_answer_ocr' && (job.status === 'queued' || job.status === 'running'));
  const currentRow = issueRows.find((row) => row.issueId === selectedIssueId) ?? issueRows[0] ?? null;
  const workflowStage = project.workflow.currentStage;
  const workflowLabel = stageLabels[workflowStage] ?? 'İş akışı kontrolü gerekli';
  const blockerLabels = project.workflow.blockingReasons.map((reason) => blockingReasonLabels[reason] ?? 'İş akışı için ek kontrol gerekiyor');
  const issueRecordCount = new Set(issueRows.map((row) => row.record.id)).size;
  const issueExpressionCount = issueRows.length;
  const uniqueStudentCount = new Set(issueRows.map((row) => row.studentLabel)).size;
  const reviewedIssueRecordCount = new Set(
    project.studentAnswerOcrRecords
      .filter((record) => visibleSubmissionIds.has(record.submissionId) && record.status === 'teacher_approved')
      .map((record) => record.id),
  ).size;
  const ocrRunning = !!activeJob;
  const currentModelSuggestion = currentRow ? modelSuggestions[currentRow.issueId] ?? null : null;
  const currentRecordNonApprovable = currentRow?.record.ocrProvenance?.approvableForScoring === false;
  const currentApprovalDisabledReason = currentRecordNonApprovable
    ? 'Bu sonuç deneysel tam sayfa OCR çıktısıdır; notlandırmaya onaylanamaz.'
    : undefined;
  const filterOptions: { key: StudentAnswerOcrIssueFilter; label: string }[] = [
    { key: 'all', label: 'Tümü' },
    { key: 'pending_review', label: 'İncelenecekler' },
    { key: 'suggested_correction', label: 'Önerilen düzeltmeler' },
    { key: 'critical_term_uncertain', label: 'Kritik terim' },
    { key: 'ocr_low_confidence', label: 'OCR düşük güven' },
    { key: 'resolved', label: 'Çözülenler' },
  ];

  const goToNext = () => {
    if (!currentRow || issueRows.length === 0) return;
    const currentIndex = issueRows.findIndex((row) => row.issueId === currentRow.issueId);
    const nextIndex = currentIndex >= 0 ? (currentIndex + 1) % issueRows.length : 0;
    setSelectedIssueId(issueRows[nextIndex]?.issueId ?? null);
  };

  const handleApplySuggestion = async (
    row: IssueRow,
    correction: Pick<OcrSuggestedCorrection, 'originalText' | 'suggestedText'>,
  ) => {
    setError(null);
    const currentText = drafts[row.draftKey] ?? getStudentAnswerOcrDraftText(row.record);
    const applied = applyStudentAnswerOcrSuggestedCorrection(currentText, correction);
    await saveMutation.mutateAsync({
      submissionId: row.record.submissionId,
      questionId: row.record.questionId,
      text: applied.text,
    });
    setDrafts((current) => ({
      ...current,
      [row.draftKey]: applied.text,
    }));
  };

  const handleCheckWithGemma = async (row: IssueRow) => {
    setError(null);
    await suggestMutation.mutateAsync({
      issueId: row.issueId,
      projectPath: projectPath!,
      ocrRecordId: row.record.id,
      observedText: row.observedText,
      questionNumber: row.questionNumber,
      highlightRegion: row.overlayBoxes[0] ?? null,
      cropRef: row.modelInputRef,
      modelInputCropRef: row.record.modelInputCropRef ?? null,
    });
  };

  const handleSave = async (row: IssueRow) => {
    setError(null);
    await saveMutation.mutateAsync({
      submissionId: row.record.submissionId,
      questionId: row.record.questionId,
      text: drafts[row.draftKey] ?? getStudentAnswerOcrDraftText(row.record),
    });
  };

  const handleApprove = async (row: IssueRow) => {
    setError(null);
    await saveMutation.mutateAsync({
      submissionId: row.record.submissionId,
      questionId: row.record.questionId,
      text: drafts[row.draftKey] ?? getStudentAnswerOcrDraftText(row.record),
    });
    await approveMutation.mutateAsync({
      submissionId: row.record.submissionId,
      questionId: row.record.questionId,
    });
    goToNext();
  };

  const selectedDraft = currentRow ? (drafts[currentRow.draftKey] ?? getStudentAnswerOcrDraftText(currentRow.record)) : '';
  const selectedTextHighlights = currentRow ? currentRow.textHighlights : [];

  return (
    <div style={{ padding: '2rem', maxWidth: '1600px', margin: '0 auto', fontFamily: 'system-ui, -apple-system, sans-serif', minHeight: 'calc(100vh - 4rem)', background: 'linear-gradient(180deg, #f8fafc 0%, #ffffff 100%)' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', marginBottom: '1.25rem', fontSize: '0.875rem' }}>
        <Link to={`/project/${encodeURIComponent(projectId)}/overview`} style={{ color: '#64748b', textDecoration: 'none' }}>İş Akışı</Link>
        <span style={{ color: '#cbd5e1' }}>/</span>
        <Link to={projectStudentOperationsPath(projectId, 'ocr', searchParams.toString())} style={{ color: '#64748b', textDecoration: 'none' }}>Öğrenci Cevap OCR</Link>
        <span style={{ color: '#cbd5e1' }}>/</span>
        <span style={{ color: '#0f172a', fontWeight: 700 }}>OCR Sorun İnceleme</span>
      </div>

      <div style={{ display: 'flex', justifyContent: 'space-between', gap: '1rem', alignItems: 'flex-start', flexWrap: 'wrap', marginBottom: '1.5rem' }}>
        <div>
          <h2 style={{ fontSize: '1.75rem', fontWeight: 800, color: '#0f172a', margin: 0, letterSpacing: '-0.03em' }}>OCR Sorun İnceleme</h2>
          <p style={{ margin: '0.35rem 0 0 0', color: '#64748b', fontSize: '0.875rem' }}>
            Sorunlu OCR kayıtlarını öğrenciye göre tek yerde toplayın, crop ve metin üstünde hızlıca inceleyin.
          </p>
          <p style={{ margin: '0.35rem 0 0 0', color: '#64748b', fontSize: '0.8125rem' }}>
            Proje: {project.name}
          </p>
        </div>

        <div style={{ display: 'flex', gap: '0.75rem', flexWrap: 'wrap', alignItems: 'center' }}>
          <div style={{ padding: '0.5rem 0.75rem', borderRadius: '999px', border: '1px solid #cbd5e1', background: 'white', color: '#475569', fontSize: '0.875rem' }}>
            Aşama: <strong>{workflowLabel}</strong>
          </div>
          {activeJob && (
            <div style={{ padding: '0.5rem 0.75rem', borderRadius: '999px', border: '1px solid #fde68a', background: '#fffbeb', color: '#92400e', fontSize: '0.875rem' }}>
              OCR işi çalışıyor: {activeJob.progress.message}
            </div>
          )}
          {ocrReadiness?.ocrReviewPolicy && (
            <div title={ocrReadiness.ocrReviewPolicy.fingerprint} style={{ padding: '0.5rem 0.75rem', borderRadius: '999px', border: '1px solid #cbd5e1', background: 'white', color: '#475569', fontSize: '0.75rem' }}>
              OCR politikası: {ocrReadiness.ocrReviewPolicy.version}
            </div>
          )}
        </div>
      </div>

      {queryError && <div style={{ marginBottom: '1rem' }}><ErrorBanner error={queryError} /></div>}

      {blockerLabels.length > 0 && (
        <div style={{ marginBottom: '1rem', padding: '0.875rem 1rem', borderRadius: '1rem', border: '1px solid #bfdbfe', background: '#eff6ff', color: '#1e3a8a', fontSize: '0.875rem' }}>
          <strong style={{ marginRight: '0.5rem' }}>Workflow engeli:</strong>
          {blockerLabels.join(' · ')}
        </div>
      )}

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, minmax(0, 1fr))', gap: '0.75rem', marginBottom: '1.5rem' }}>
        {[
          { label: 'Sorunlu kayıt', value: issueRecordCount },
          { label: 'Sorunlu ifade', value: issueExpressionCount },
          { label: 'Öğrenci', value: uniqueStudentCount },
          { label: 'Çözülen / onaylı', value: reviewedIssueRecordCount },
        ].map((item) => (
          <div key={item.label} style={{ padding: '1rem', borderRadius: '1rem', border: '1px solid #e2e8f0', background: 'white', boxShadow: '0 1px 2px rgba(15, 23, 42, 0.04)' }}>
            <div style={{ fontSize: '0.75rem', textTransform: 'uppercase', letterSpacing: '0.08em', color: '#64748b', fontWeight: 700 }}>{item.label}</div>
            <div style={{ fontSize: '1.5rem', fontWeight: 800, color: '#0f172a', marginTop: '0.25rem' }}>{item.value}</div>
          </div>
        ))}
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: 'minmax(340px, 0.45fr) minmax(0, 1.55fr)', gap: '1rem', minHeight: 'calc(100vh - 14rem)' }}>
        <aside style={{ background: 'white', border: '1px solid #e2e8f0', borderRadius: '1.25rem', overflow: 'hidden', display: 'flex', flexDirection: 'column', minHeight: 0 }}>
          <div style={{ padding: '1rem 1.25rem', borderBottom: '1px solid #f1f5f9', background: 'linear-gradient(180deg, #ffffff 0%, #f8fafc 100%)' }}>
            <div style={{ fontSize: '0.875rem', fontWeight: 700, color: '#334155' }}>Sorunlu ifadeler</div>
            <div style={{ fontSize: '0.75rem', color: '#64748b', marginTop: '0.25rem' }}>Öğrenci adına göre sıralı, internal id gösterilmez.</div>
          </div>
          <div style={{ padding: '0.9rem 1.1rem 0.65rem', borderBottom: '1px solid #f1f5f9', display: 'grid', gap: '0.7rem' }}>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.4rem' }}>
              {filterOptions.map((option) => {
                const active = selectedFilter === option.key;
                return (
                  <button
                    key={option.key}
                    type="button"
                    data-project-write="false"
                    onClick={() => setSelectedFilter(option.key)}
                    style={{
                      border: '1px solid',
                      borderColor: active ? '#c7d2fe' : '#e2e8f0',
                      background: active ? '#eef2ff' : 'white',
                      color: active ? '#3730a3' : '#475569',
                      borderRadius: '999px',
                      padding: '0.32rem 0.65rem',
                      fontSize: '0.75rem',
                      fontWeight: 700,
                      cursor: 'pointer',
                    }}
                  >
                    {option.label}
                  </button>
                );
              })}
            </div>
            <button
              type="button"
              onClick={() => {
                setSelectedIssueId(null);
                void rebuildMutation.mutateAsync();
              }}
              disabled={rebuildMutation.isPending}
              style={{
                width: 'fit-content',
                border: '1px solid #cbd5e1',
                background: 'white',
                color: '#334155',
                borderRadius: '999px',
                padding: '0.35rem 0.7rem',
                fontSize: '0.75rem',
                fontWeight: 700,
                cursor: rebuildMutation.isPending ? 'not-allowed' : 'pointer',
                opacity: rebuildMutation.isPending ? 0.6 : 1,
              }}
            >
              {rebuildMutation.isPending ? 'Yeniden taranıyor...' : 'Sorunları yeniden tara'}
            </button>
          </div>
          <div style={{ overflowY: 'auto', flex: 1 }}>
            {issueRows.length === 0 ? (
              <div style={{ padding: '2rem', color: '#64748b', textAlign: 'center', display: 'grid', gap: '0.5rem' }}>
                <div>
                  {selectedFilter === 'pending_review' && reviewedIssueRecordCount > 0
                    ? 'Açık incelenecek OCR sorunu yok. Çözülen veya onaylanmış sorunları görmek için Tümü/Çözülenler filtresini açın.'
                    : 'Bu filtrede incelenecek somut OCR sorunu yok.'}
                </div>
                {selectedFilter === 'pending_review' && reviewedIssueRecordCount > 0 && (
                  <div style={{ fontSize: '0.75rem' }}>Q5 gibi onaylı kayıtlar Tümü veya Çözülenler altında görünür.</div>
                )}
              </div>
            ) : (
              issueRows.map((row) => {
                const isSelected = row.issueId === currentRow?.issueId;
                const selectedColor = isSelected ? '#3730a3' : '#0f172a';
                return (
                  <button
                    key={row.issueId}
                    data-project-write="false"
                    onClick={() => setSelectedIssueId(row.issueId)}
                    style={{
                      width: '100%',
                      border: 'none',
                      borderBottom: '1px solid #f1f5f9',
                      background: isSelected ? '#eef2ff' : 'white',
                      padding: '0.9rem 1rem',
                      textAlign: 'left',
                      cursor: 'pointer',
                    }}
                  >
                    <div style={{ display: 'flex', justifyContent: 'space-between', gap: '0.75rem', alignItems: 'flex-start' }}>
                      <div style={{ minWidth: 0 }}>
                        <div style={{ fontSize: '0.95rem', fontWeight: 800, color: selectedColor, lineHeight: 1.3 }}>
                          {row.studentLabel}
                        </div>
                        <div style={{ marginTop: '0.2rem', fontSize: '0.75rem', color: '#64748b' }}>
                          No: {row.studentNumber} · Sınıf: {row.studentClassName} · Soru {row.questionNumber}
                        </div>
                      </div>
                      <span style={{ fontSize: '0.7rem', fontWeight: 700, color: issueColor(row.issueKind).color, background: issueColor(row.issueKind).background, border: `1px solid ${issueColor(row.issueKind).border}`, padding: '0.18rem 0.45rem', borderRadius: '999px', whiteSpace: 'nowrap' }}>
                        {row.issueLabel}
                      </span>
                    </div>
                    <div style={{ marginTop: '0.35rem', fontSize: '0.825rem', color: '#334155', lineHeight: 1.35 }}>
                      <span style={{ fontWeight: 700 }}>{row.issueKind === 'suggested_correction' ? 'Öneri:' : 'Sorunlu ifade:'}</span> {row.issueDisplayText}
                    </div>
                  </button>
                );
              })
            )}
          </div>
        </aside>

        <section style={{ background: 'white', border: '1px solid #e2e8f0', borderRadius: '1.25rem', overflow: 'hidden', display: 'flex', flexDirection: 'column', minHeight: 0 }}>
          {!currentRow ? (
            <div style={{ padding: '2rem', color: '#64748b' }}>Detay görmek için soldan bir kayıt seçin.</div>
          ) : (
            <>
              <div style={{ padding: '1rem 1.25rem', borderBottom: '1px solid #f1f5f9', display: 'flex', justifyContent: 'space-between', gap: '1rem', alignItems: 'center', flexWrap: 'wrap', background: 'linear-gradient(180deg, #ffffff 0%, #f8fafc 100%)' }}>
                <div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', flexWrap: 'wrap' }}>
                    <h3 style={{ margin: 0, fontSize: '1.125rem', fontWeight: 800, color: '#0f172a' }}>{currentRow.studentLabel}</h3>
                    <span style={{ fontSize: '0.75rem', fontWeight: 700, color: '#475569', background: '#e2e8f0', padding: '0.2rem 0.45rem', borderRadius: '999px' }}>No {currentRow.studentNumber}</span>
                    <span style={{ fontSize: '0.75rem', fontWeight: 700, color: '#475569', background: '#e2e8f0', padding: '0.2rem 0.45rem', borderRadius: '999px' }}>Sınıf {currentRow.studentClassName}</span>
                    <span style={{ fontSize: '0.75rem', fontWeight: 700, color: '#1d4ed8', background: '#dbeafe', padding: '0.2rem 0.45rem', borderRadius: '999px' }}>Soru {currentRow.questionNumber}</span>
                    <span style={{ fontSize: '0.75rem', fontWeight: 700, color: '#92400e', background: '#fef3c7', padding: '0.2rem 0.45rem', borderRadius: '999px' }}>{studentAnswerOcrStatusLabels[currentRow.record.status] ?? 'İnceleme gerekli'}</span>
                    <span style={{ fontSize: '0.75rem', fontWeight: 700, color: issueColor(currentRow.issueKind).color, background: issueColor(currentRow.issueKind).background, border: `1px solid ${issueColor(currentRow.issueKind).border}`, padding: '0.2rem 0.45rem', borderRadius: '999px' }}>{currentRow.issueLabel}</span>
                  </div>
                  <div style={{ marginTop: '0.35rem', fontSize: '0.875rem', color: '#64748b' }}>
                    {currentRow.issueSummary}
                  </div>
                  {currentRecordNonApprovable && (
                    <div style={{ marginTop: '0.5rem', color: '#9a3412', background: '#fff7ed', border: '1px solid #fdba74', borderRadius: '0.5rem', padding: '0.5rem 0.65rem', fontSize: '0.75rem' }}>
                      Yalnızca inceleme ve metin düzeltme referansı. Bu sonuç notlandırmaya onaylanamaz.
                    </div>
                  )}
                </div>

                <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
                  <button
                    type="button"
                    data-project-write="true"
                    onClick={() => void handleApprove(currentRow)}
                    disabled={ocrRunning || saveMutation.isPending || approveMutation.isPending || !!currentApprovalDisabledReason}
                    title={currentApprovalDisabledReason}
                    style={{ display: 'inline-flex', alignItems: 'center', gap: '0.4rem', padding: '0.55rem 0.85rem', borderRadius: '0.75rem', border: '1px solid #86efac', background: '#dcfce7', color: '#166534', fontWeight: 700, cursor: (ocrRunning || saveMutation.isPending || approveMutation.isPending || !!currentApprovalDisabledReason) ? 'not-allowed' : 'pointer', opacity: (ocrRunning || saveMutation.isPending || approveMutation.isPending || !!currentApprovalDisabledReason) ? 0.6 : 1 }}
                  >
                    <BadgeCheck size={16} /> Bu OCR doğru
                  </button>
                  <button
                    type="button"
                    data-project-write="true"
                    onClick={() => void handleSave(currentRow)}
                    disabled={ocrRunning || saveMutation.isPending || approveMutation.isPending}
                    style={{ display: 'inline-flex', alignItems: 'center', gap: '0.4rem', padding: '0.55rem 0.85rem', borderRadius: '0.75rem', border: '1px solid #cbd5e1', background: 'white', color: '#334155', fontWeight: 700, cursor: (ocrRunning || saveMutation.isPending || approveMutation.isPending) ? 'not-allowed' : 'pointer', opacity: (ocrRunning || saveMutation.isPending || approveMutation.isPending) ? 0.6 : 1 }}
                  >
                    <PencilLine size={16} /> Düzenlemeyi kaydet
                  </button>
                  <button
                    type="button"
                    data-project-write="true"
                    onClick={() => void handleCheckWithGemma(currentRow)}
                    disabled={ocrRunning || saveMutation.isPending || approveMutation.isPending || suggestMutation.isPending || (!currentRow.suggestionText && !currentRow.correction?.suggestedText)}
                  style={{ display: 'inline-flex', alignItems: 'center', gap: '0.4rem', padding: '0.55rem 0.85rem', borderRadius: '0.75rem', border: '1px solid #fde68a', background: '#fffbeb', color: '#92400e', fontWeight: 700, cursor: (ocrRunning || saveMutation.isPending || approveMutation.isPending || suggestMutation.isPending) ? 'not-allowed' : 'pointer', opacity: (ocrRunning || saveMutation.isPending || approveMutation.isPending || suggestMutation.isPending) ? 0.6 : 1 }}
                  >
                    Gemma ile öneriyi kontrol et
                  </button>
                  <button
                    type="button"
                    data-project-write="false"
                    onClick={goToNext}
                    disabled={issueRows.length < 2}
                    style={{ display: 'inline-flex', alignItems: 'center', gap: '0.4rem', padding: '0.55rem 0.85rem', borderRadius: '0.75rem', border: '1px solid #cbd5e1', background: '#f8fafc', color: '#334155', fontWeight: 700, cursor: issueRows.length < 2 ? 'not-allowed' : 'pointer', opacity: issueRows.length < 2 ? 0.6 : 1 }}
                  >
                    <ChevronRight size={16} /> Sonraki kayıt
                  </button>
                </div>
              </div>

              <div style={{ padding: '1rem 1.25rem', display: 'grid', gridTemplateColumns: 'minmax(320px, 0.95fr) minmax(0, 1.05fr)', gap: '1rem', minHeight: 0, overflow: 'hidden' }}>
                <div style={{ display: 'grid', gap: '0.9rem', minHeight: 0 }}>
                  <div style={{ border: '1px solid #e2e8f0', borderRadius: '1rem', overflow: 'hidden', background: '#0f172a' }}>
                    <div style={{ padding: '0.75rem 0.9rem', background: '#111827', color: 'white', display: 'flex', justifyContent: 'space-between', gap: '1rem', alignItems: 'center' }}>
                      <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', fontWeight: 700 }}><FileImage size={16} /> Crop</div>
                      <span style={{ fontSize: '0.75rem', color: '#cbd5e1' }}>{getStudentAnswerOcrPreprocessSummary(currentRow.record)}</span>
                    </div>
                    <div style={{ background: '#0f172a', padding: '0.75rem', minHeight: '320px' }}>
                      <PdfPageViewer
                        imagePath={currentRow.record.modelInputCropRef ?? currentRow.record.originalCropRefs?.[0] ?? currentRow.record.cropRefs[0] ?? currentRow.record.preprocessedCropRefs?.[0] ?? currentRow.record.fullPagePreviewRefs[0] ?? null}
                        projectId={projectId}
                        pageNumber={currentRow.record.questionNumber}
                        zoom={0.95}
                        overlayItems={currentRow.overlayBoxes.map((box) => ({ box, label: currentRow.issueSummary }))}
                        minimal
                        emptyState={<div style={{ color: '#cbd5e1', fontSize: '0.875rem' }}>{getStudentAnswerOcrPreviewMessage(currentRow.record)}</div>}
                      />
                      {!currentRow.overlayBoxes.length && currentRow.textHighlights.length > 0 && (
                        <div style={{ marginTop: '0.5rem', color: '#cbd5e1', fontSize: '0.75rem' }}>Görsel konumu bulunamadı; metin vurgusu kullanılıyor.</div>
                      )}
                    </div>
                  </div>

                  <div style={{ border: '1px solid #e2e8f0', borderRadius: '1rem', padding: '0.9rem 1rem', display: 'grid', gap: '0.65rem' }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', gap: '0.75rem', alignItems: 'center', flexWrap: 'wrap' }}>
                      <div style={{ fontSize: '0.75rem', fontWeight: 800, color: '#64748b', textTransform: 'uppercase', letterSpacing: '0.08em' }}>Sorunlu ifade</div>
                      {currentRow.overlayBoxes.length > 0 ? (
                        <span style={{ fontSize: '0.72rem', color: '#166534', background: '#dcfce7', padding: '0.18rem 0.45rem', borderRadius: '999px' }}>BBox var</span>
                      ) : (
                        <span style={{ fontSize: '0.72rem', color: '#92400e', background: '#fef3c7', padding: '0.18rem 0.45rem', borderRadius: '999px' }}>Metin vurgusu</span>
                      )}
                    </div>
                    <div style={{ fontSize: '0.95rem', fontWeight: 700, color: '#0f172a' }}>
                      {currentRow.issueDisplayText}
                    </div>
                    {currentRow.suggestionText && (
                      <div style={{ fontSize: '0.85rem', color: '#166534' }}>
                        Öneri: {currentRow.suggestionText}
                      </div>
                    )}
                    {!currentRow.overlayBoxes.length && currentRow.textHighlights.length === 0 && (
                      <div style={{ fontSize: '0.75rem', color: '#92400e' }}>Görsel konumu bulunamadı; yalnızca OCR metni vurgulanıyor.</div>
                    )}
                  </div>
                </div>

                <div style={{ display: 'grid', gap: '0.9rem', minHeight: 0 }}>
                  <div style={{ border: '1px solid #e2e8f0', borderRadius: '1rem', padding: '1rem', display: 'grid', gap: '0.75rem', minHeight: 0 }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', gap: '1rem', alignItems: 'center' }}>
                      <div style={{ fontSize: '0.75rem', fontWeight: 800, color: '#64748b', textTransform: 'uppercase', letterSpacing: '0.08em' }}>OCR metni</div>
                      <span style={{ fontSize: '0.75rem', color: '#64748b' }}>{currentRow.record.sourcePageNumbers.length ? `Sayfa ${currentRow.record.sourcePageNumbers.join(', ')}` : 'Sayfa bilinmiyor'}</span>
                    </div>
                    <textarea
                      value={selectedDraft}
                      onChange={(event) => setDrafts((current) => ({ ...current, [currentRow.draftKey]: event.target.value }))}
                      rows={8}
                      style={{ width: '100%', minHeight: '220px', borderRadius: '0.85rem', border: '1px solid #cbd5e1', padding: '0.9rem 1rem', fontSize: '0.95rem', lineHeight: 1.6, color: '#0f172a', resize: 'vertical', outline: 'none', background: '#fff' }}
                    />
                    <div style={{ padding: '0.85rem', borderRadius: '0.85rem', background: '#f8fafc', border: '1px solid #e2e8f0', lineHeight: 1.85, fontSize: '0.95rem', color: '#1f2937' }}>
                      {renderHighlightedText(selectedDraft, selectedTextHighlights)}
                    </div>
                  </div>

                  <div style={{ border: '1px solid #e2e8f0', borderRadius: '1rem', padding: '1rem', display: 'grid', gap: '1rem' }}>
                    <div style={{ display: 'grid', gap: '0.6rem' }}>
                      <div style={{ display: 'flex', justifyContent: 'space-between', gap: '0.75rem', alignItems: 'center', flexWrap: 'wrap' }}>
                        <div style={{ fontSize: '0.75rem', fontWeight: 800, color: '#64748b', textTransform: 'uppercase', letterSpacing: '0.08em' }}>Sorunlu ifade</div>
                        <span style={{ fontSize: '0.72rem', color: '#475569', background: '#e2e8f0', padding: '0.18rem 0.45rem', borderRadius: '999px' }}>{currentRow.issueLabel}</span>
                      </div>
                      <div style={{ padding: '0.85rem', borderRadius: '0.85rem', border: '1px solid #dbeafe', background: '#eff6ff', display: 'grid', gap: '0.35rem' }}>
                        <div style={{ fontSize: '0.9rem', fontWeight: 700, color: '#1d4ed8' }}>{currentRow.issueDisplayText}</div>
                        <div style={{ fontSize: '0.8rem', color: '#475569' }}>{currentRow.issueSummary}</div>
                        {currentRow.correction && (
                          <button
                            type="button"
                            data-project-write="true"
                            onClick={() => void handleApplySuggestion(currentRow, currentRow.correction!)}
                            disabled={ocrRunning || saveMutation.isPending || approveMutation.isPending}
                            style={{ width: 'fit-content', display: 'inline-flex', alignItems: 'center', gap: '0.35rem', padding: '0.45rem 0.7rem', borderRadius: '0.75rem', border: '1px solid #86efac', background: 'white', color: '#166534', fontWeight: 700, cursor: (ocrRunning || saveMutation.isPending || approveMutation.isPending) ? 'not-allowed' : 'pointer', opacity: (ocrRunning || saveMutation.isPending || approveMutation.isPending) ? 0.6 : 1 }}
                          >
                            Bu düzeltmeyi uygula
                          </button>
                        )}
                      </div>
                    </div>

                    {currentRow.debugWarnings.length > 0 && (
                      <details style={{ border: '1px solid #e2e8f0', borderRadius: '0.85rem', padding: '0.75rem 0.85rem', background: '#fff' }}>
                        <summary style={{ cursor: 'pointer', fontSize: '0.8rem', fontWeight: 700, color: '#64748b' }}>Ek uyarılar</summary>
                        <div style={{ marginTop: '0.75rem', display: 'grid', gap: '0.5rem' }}>
                          <ul style={{ margin: 0, paddingLeft: '1.1rem', color: '#475569', display: 'grid', gap: '0.3rem' }}>
                            {currentRow.debugWarnings.map((warning) => (
                              <li key={warning}>{warningMessage(warning, currentRow.record.reviewPolicy ?? ocrReadiness?.ocrReviewPolicy)}</li>
                            ))}
                          </ul>
                        </div>
                      </details>
                    )}

                    {currentModelSuggestion && (
                      <div style={{ border: '1px solid #fde68a', borderRadius: '0.85rem', padding: '0.85rem', background: '#fffbeb', display: 'grid', gap: '0.5rem' }}>
                        <div style={{ display: 'flex', justifyContent: 'space-between', gap: '0.75rem', alignItems: 'center', flexWrap: 'wrap' }}>
                          <div style={{ fontSize: '0.75rem', fontWeight: 800, color: '#92400e', textTransform: 'uppercase', letterSpacing: '0.08em' }}>Gemma kontrol sonucu</div>
                          <span style={{ fontSize: '0.72rem', color: '#92400e', background: '#fde68a', padding: '0.18rem 0.45rem', borderRadius: '999px' }}>
                            {modelDecisionLabel(currentModelSuggestion.suggestion.decision)}
                          </span>
                        </div>
                        <div style={{ fontSize: '0.9rem', fontWeight: 700, color: '#0f172a' }}>
                          {currentModelSuggestion.suggestion.suggestedText
                            ? `Öneri: ${currentModelSuggestion.suggestion.suggestedText}`
                            : 'Gemma bu issue için metin değişikliği önermedi.'}
                        </div>
                        <div style={{ fontSize: '0.8rem', color: '#475569', lineHeight: 1.55 }}>
                          <div>Kapsam: {modelScopeLabel(currentModelSuggestion.suggestion.scope)}</div>
                          <div>Görsel okuma: {currentModelSuggestion.suggestion.visualReading ?? 'okunamadı'}</div>
                          <div>Gerekçe: {currentModelSuggestion.suggestion.contextReason}</div>
                          <div>Güven: {Math.round(currentModelSuggestion.suggestion.confidence * 100)}%</div>
                          <div>Öğretmen onayı gerekli: evet</div>
                        </div>
                        {currentModelSuggestion.suggestion.warnings.length > 0 && (
                          <div style={{ fontSize: '0.78rem', color: '#b45309' }}>
                            Uyarılar: {currentModelSuggestion.suggestion.warnings.map((warning) => warningMessage(warning)).join(' · ')}
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                </div>
              </div>

              <div style={{ padding: '0 1.25rem 1.25rem 1.25rem', display: 'flex', justifyContent: 'space-between', gap: '1rem', flexWrap: 'wrap', alignItems: 'center' }}>
                <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
                  {currentRow.overlayBoxes.length > 0 ? (
                    <span style={{ fontSize: '0.75rem', color: '#166534', background: '#dcfce7', padding: '0.25rem 0.45rem', borderRadius: '999px' }}>Crop üzerinde highlight mevcut</span>
                  ) : (
                    <span style={{ fontSize: '0.75rem', color: '#92400e', background: '#fef3c7', padding: '0.25rem 0.45rem', borderRadius: '999px' }}>BBox yok, metin vurgusu kullanılıyor</span>
                  )}
                  {ocrRunning && (
                    <span style={{ fontSize: '0.75rem', color: '#92400e', background: '#fffbeb', padding: '0.25rem 0.45rem', borderRadius: '999px' }}>Aktif OCR işi var</span>
                  )}
                </div>
                <button
                  type="button"
                  data-project-write="false"
                  onClick={goToNext}
                  disabled={issueRows.length < 2}
                  style={{ display: 'inline-flex', alignItems: 'center', gap: '0.4rem', padding: '0.55rem 0.85rem', borderRadius: '0.75rem', border: '1px solid #cbd5e1', background: '#f8fafc', color: '#334155', fontWeight: 700, cursor: issueRows.length < 2 ? 'not-allowed' : 'pointer', opacity: issueRows.length < 2 ? 0.6 : 1 }}
                >
                  <ChevronRight size={16} /> Sonraki sorunlu kayıt
                </button>
              </div>
            </>
          )}
        </section>
      </div>
    </div>
  );
}

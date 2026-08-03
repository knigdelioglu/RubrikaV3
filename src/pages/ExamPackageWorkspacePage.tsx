import { useEffect, useMemo, useRef, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { open } from '@tauri-apps/plugin-dialog';
import {
  AlertCircle,
  ArrowLeft,
  ArrowRight,
  CheckCircle2,
  FileSearch,
  FileUp,
  Loader2,
  LockKeyhole,
  PlayCircle,
  RefreshCcw,
  Save,
} from 'lucide-react';
import { commands } from '../api/commands';
import type { AppError } from '../api/errors';
import type { RubricDraft } from '../components/rubric/RubricQuestionCard';
import { RubricQuestionCard } from '../components/rubric/RubricQuestionCard';
import { ConfirmationDialog } from '../components/common/ConfirmationDialog';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { LoadingButton } from '../components/common/LoadingButton';
import { ProjectContextState } from '../components/common/ProjectContextState';
import { QuestionCountDialog } from '../components/common/QuestionCountDialog';
import { tauriClient } from '../api/tauriClient';
import { useProjectContext } from '../state/useProjectContext';
import { DocumentsPage } from './DocumentsPage';
import {
  answerTypeLabels,
  blockingReasonLabels,
  rubricStatusLabels,
  textFieldSourceLabels,
  textFieldStatusLabels,
} from '../utils/labels';
import { getExamPackageFreezeUiState } from '../utils/examPackageFreeze';
import { projectExamPackagePath, type ExamPackageTab } from '../app/projectRoutes';
import {
  buildExamPackageQuestionItems,
  buildExamPackageWorkspaceSummary,
  createSingleFlightAction,
  mergePersistedDrafts,
  normalizeExamPackageTab,
  resolveSelectedQuestionId,
  rubricDraftFromItem,
  splitRubricLines,
} from './examPackageWorkspace';

const ACTIVE_JOB_KINDS = new Set(['question_text_extraction', 'rubric_pdf_import', 'exam_package_build']);

function updateSearch(
  locationSearch: string,
  navigate: ReturnType<typeof useNavigate>,
  projectId: string,
  patch: { tab?: ExamPackageTab; questionId?: string | null },
) {
  const search = new URLSearchParams(locationSearch);
  if (patch.tab) search.set('tab', patch.tab);
  if (patch.questionId === null) search.delete('questionId');
  else if (patch.questionId) search.set('questionId', patch.questionId);
  navigate(projectExamPackagePath(projectId, normalizeExamPackageTab(search.get('tab')), search.toString()));
}

function nextQuestionId(questionIds: string[], selectedId: string | null, direction: -1 | 1): string | null {
  if (!selectedId) return questionIds[0] ?? null;
  const index = questionIds.indexOf(selectedId);
  if (index < 0) return questionIds[0] ?? null;
  return questionIds[index + direction] ?? null;
}

export function ExamPackageWorkspacePage() {
  const { projectId, projectPath, isResolving } = useProjectContext();
  const location = useLocation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [error, setError] = useState<AppError | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const [questionDrafts, setQuestionDrafts] = useState<Record<string, string>>({});
  const [rubricDrafts, setRubricDrafts] = useState<Record<string, RubricDraft>>({});
  const [dirtyQuestionIds, setDirtyQuestionIds] = useState<Set<string>>(new Set());
  const [dirtyRubricIds, setDirtyRubricIds] = useState<Set<string>>(new Set());
  const [editingQuestionId, setEditingQuestionId] = useState<string | null>(null);
  const [savingQuestionId, setSavingQuestionId] = useState<string | null>(null);
  const [savingRubricId, setSavingRubricId] = useState<string | null>(null);
  const [freezeDialogOpen, setFreezeDialogOpen] = useState(false);
  const [questionCountDialogOpen, setQuestionCountDialogOpen] = useState(false);
  const freezeGuardRef = useRef(false);
  const mutationGuardRef = useRef(new Set<string>());
  const jobStartGuardRef = useRef(new Set<string>());
  const guardedFreezeCommand = useMemo(
    () => createSingleFlightAction(() => commands.confirmAllRubrics({ projectId })),
    [projectId],
  );

  const tab = normalizeExamPackageTab(new URLSearchParams(location.search).get('tab'));
  const requestedQuestionId = new URLSearchParams(location.search).get('questionId');

  const projectQuery = useQuery({
    queryKey: ['project-snapshot', projectId],
    queryFn: () => commands.getProjectSnapshot(projectId),
    enabled: !!projectId,
  });
  const workflowQuery = useQuery({
    queryKey: ['workflow-snapshot', projectId],
    queryFn: () => commands.getWorkflowSnapshot(projectId),
    enabled: !!projectId,
  });
  const rubricItemsQuery = useQuery({
    queryKey: ['rubric-items', projectId],
    queryFn: () => commands.listRubricItems(projectId),
    enabled: !!projectId,
  });
  const validationQuery = useQuery({
    queryKey: ['rubric-validation', projectId],
    queryFn: () => commands.validateRubrics({ projectId }),
    enabled: !!projectId,
  });
  const questionStatusQuery = useQuery({
    queryKey: ['question-text-status', projectId],
    queryFn: () => commands.getQuestionTextExtractionStatus({ projectId }),
    enabled: !!projectId,
  });
  const jobsQuery = useQuery({
    queryKey: ['jobs', projectId],
    queryFn: () => commands.listJobs(projectId),
    enabled: !!projectId,
    refetchInterval: (query) => query.state.data?.some((job) => job.status === 'queued' || job.status === 'running') ? 1000 : false,
  });

  const project = projectQuery.data;
  const workflow = workflowQuery.data;
  const rubricItems = useMemo(() => rubricItemsQuery.data ?? [], [rubricItemsQuery.data]);
  const validation = validationQuery.data ?? null;
  const selectedQuestionId = resolveSelectedQuestionId(project?.questions ?? [], requestedQuestionId);
  const selectedQuestion = project?.questions.find((question) => question.id === selectedQuestionId) ?? null;
  const selectedRubricItem = rubricItems.find((item) => item.question.id === selectedQuestionId) ?? null;
  const orderedQuestionIds = useMemo(
    () => [...(project?.questions ?? [])].sort((a, b) => a.number - b.number).map((question) => question.id),
    [project?.questions],
  );

  const invalidateWorkspace = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['rubric-state', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['rubric-items', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['rubric-validation', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['question-text-status', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['question-text-suggestions', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['jobs', projectId] }),
    ]);
  };

  useEffect(() => {
    if (!project) return;
    setQuestionDrafts((current) => mergePersistedDrafts(
      current,
      project.questions.map((question) => [question.id, question.questionText.value] as const),
      dirtyQuestionIds,
    ));
  }, [dirtyQuestionIds, project]);

  useEffect(() => {
    if (rubricItems.length === 0) return;
    setRubricDrafts((current) => mergePersistedDrafts(
      current,
      rubricItems.map((item) => [item.question.id, rubricDraftFromItem(item)] as const),
      dirtyRubricIds,
    ));
  }, [dirtyRubricIds, rubricItems]);

  useEffect(() => {
    if (!projectId) return;
    let mounted = true;
    let unlisten: (() => void) | undefined;
    void tauriClient.listenToJobEvents(() => {
      if (!mounted) return;
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['rubric-state', projectId] });
      queryClient.invalidateQueries({ queryKey: ['rubric-items', projectId] });
      queryClient.invalidateQueries({ queryKey: ['rubric-validation', projectId] });
      queryClient.invalidateQueries({ queryKey: ['question-text-status', projectId] });
      queryClient.invalidateQueries({ queryKey: ['jobs', projectId] });
    }).then((cleanup) => {
      unlisten = cleanup;
      if (!mounted) cleanup();
    });
    return () => {
      mounted = false;
      unlisten?.();
    };
  }, [projectId, queryClient]);

  useEffect(() => {
    if (dirtyQuestionIds.size === 0 && dirtyRubricIds.size === 0) return;
    const preventUnload = (event: BeforeUnloadEvent) => event.preventDefault();
    window.addEventListener('beforeunload', preventUnload);
    return () => window.removeEventListener('beforeunload', preventUnload);
  }, [dirtyQuestionIds, dirtyRubricIds]);

  useEffect(() => {
    if (!projectId || !project || requestedQuestionId === selectedQuestionId) return;
    updateSearch(location.search, navigate, projectId, { questionId: selectedQuestionId });
  }, [location.search, navigate, project, projectId, requestedQuestionId, selectedQuestionId]);

  const activeJobs = (jobsQuery.data ?? []).filter(
    (job) => ACTIVE_JOB_KINDS.has(job.kind) && (job.status === 'queued' || job.status === 'running'),
  );
  const questionItems = project ? buildExamPackageQuestionItems(project.questions, rubricItems) : [];
  const summary = project && workflow
    ? buildExamPackageWorkspaceSummary(project, workflow, validation)
    : null;
  const freezeUiState = getExamPackageFreezeUiState(project, workflow, freezeGuardRef.current);

  const saveQuestion = async (confirm: boolean): Promise<boolean> => {
    if (!selectedQuestion || mutationGuardRef.current.has(`question:${selectedQuestion.id}`)) return false;
    mutationGuardRef.current.add(`question:${selectedQuestion.id}`);
    setSavingQuestionId(selectedQuestion.id);
    setError(null);
    setSuccessMessage(null);
    try {
      if (editingQuestionId === selectedQuestion.id || dirtyQuestionIds.has(selectedQuestion.id)) {
        await commands.editQuestionText({
          projectId,
          questionId: selectedQuestion.id,
          text: questionDrafts[selectedQuestion.id] ?? '',
        });
      }
      if (confirm) await commands.confirmQuestionText({ projectId, questionId: selectedQuestion.id });
      setDirtyQuestionIds((current) => {
        const next = new Set(current);
        next.delete(selectedQuestion.id);
        return next;
      });
      setEditingQuestionId(null);
      setSuccessMessage(confirm ? 'Soru metni kaydedildi ve onaylandı.' : 'Soru metni kaydedildi.');
      await invalidateWorkspace();
      return true;
    } catch (caught) {
      setError(caught as AppError);
      return false;
    } finally {
      mutationGuardRef.current.delete(`question:${selectedQuestion.id}`);
      setSavingQuestionId(null);
    }
  };

  const saveRubric = async (questionId: string, confirm: boolean): Promise<boolean> => {
    if (mutationGuardRef.current.has(`rubric:${questionId}`)) return false;
    const draft = rubricDrafts[questionId];
    if (!draft) return false;
    mutationGuardRef.current.add(`rubric:${questionId}`);
    setSavingRubricId(questionId);
    setError(null);
    setSuccessMessage(null);
    try {
      await commands.updateQuestionRubric({
        projectId,
        questionId,
        answerType: draft.answerType,
        maxScore: draft.maxScore.trim() ? Number(draft.maxScore) : null,
        expectedAnswer: draft.expectedAnswer.trim() || null,
        keyConcepts: splitRubricLines(draft.keyConcepts),
          criteria: draft.criteria.map((criterion) => ({
          id: criterion.id,
          label: criterion.label.trim(),
          description: criterion.description.trim(),
          points: Number(criterion.points || 0),
          levels: (criterion.levels ?? []).map((level) => ({
            id: level.id.trim(),
            title: level.title.trim(),
            requiredConditions: level.requiredConditions.map((condition) => condition.trim()).filter(Boolean),
            disqualifyingConditions: level.disqualifyingConditions.map((condition) => condition.trim()).filter(Boolean),
            score: Number(level.score || 0),
            evidenceRequired: level.evidenceRequired,
            version: level.version.trim(),
          })),
        })),
        partialCreditHints: splitRubricLines(draft.partialCreditHints),
        zeroScoreConditions: splitRubricLines(draft.zeroScoreConditions),
        commonMistakes: splitRubricLines(draft.commonMistakes),
      });
      if (confirm) await commands.confirmQuestionRubric({ projectId, questionId });
      setDirtyRubricIds((current) => {
        const next = new Set(current);
        next.delete(questionId);
        return next;
      });
      setSuccessMessage(confirm ? 'Rubrik kaydedildi ve onaylandı.' : 'Rubrik kaydedildi.');
      await invalidateWorkspace();
      return true;
    } catch (caught) {
      setError(caught as AppError);
      return false;
    } finally {
      mutationGuardRef.current.delete(`rubric:${questionId}`);
      setSavingRubricId(null);
    }
  };

  const freezeMutation = useMutation({
    mutationFn: guardedFreezeCommand,
    onSuccess: async () => {
      setSuccessMessage('Sınav paketi notlandırma için donduruldu.');
      setFreezeDialogOpen(false);
      await invalidateWorkspace();
    },
    onError: (caught: AppError) => setError(caught),
    onSettled: () => {
      freezeGuardRef.current = false;
    },
  });

  const confirmFreeze = () => {
    if (freezeGuardRef.current || !freezeUiState.readyToFreeze) return;
    freezeGuardRef.current = true;
    freezeMutation.mutate();
  };

  const startQuestionExtraction = async (visionFallback = false) => {
    if (activeJobs.some((job) => job.kind === 'question_text_extraction') || jobStartGuardRef.current.has('question-text')) return;
    jobStartGuardRef.current.add('question-text');
    setError(null);
    try {
      if (visionFallback) {
        await commands.startQuestionTextVisionFallback({ projectId });
      } else {
        const documentId = questionStatusQuery.data?.documentId;
        if (!documentId) throw {
          code: 'DOCUMENT_NOT_FOUND',
          safeMessage: 'Soru metni çıkarmak için sınav PDF’i bulunamadı.',
          recoveryAction: 'Önce sınav PDF’ini içe aktarın.',
          retryable: true,
          correlationId: 'unknown',
          detailsAvailable: false,
        } as AppError;
        await commands.startQuestionTextExtraction({ projectId, documentId });
      }
      setSuccessMessage(visionFallback ? 'Görsel yeniden çıkarma işi başlatıldı.' : 'Soru metni çıkarma işi başlatıldı.');
      await invalidateWorkspace();
    } catch (caught) {
      setError(caught as AppError);
    } finally {
      jobStartGuardRef.current.delete('question-text');
    }
  };

  const importRubricJson = async () => {
    if (mutationGuardRef.current.has('rubric-import')) return;
    mutationGuardRef.current.add('rubric-import');
    setSavingRubricId('import');
    try {
      const selected = await open({ multiple: false, filters: [{ name: 'Rubrik JSON', extensions: ['json'] }] });
      if (!selected || typeof selected !== 'string') return;
      await commands.importRubricJson({ projectId, filePath: selected });
      setSuccessMessage('Rubrik JSON içe aktarıldı.');
      await invalidateWorkspace();
    } catch (caught) {
      setError(caught as AppError);
    } finally {
      mutationGuardRef.current.delete('rubric-import');
      setSavingRubricId(null);
    }
  };

  const migrateRubricLevels = async () => {
    if (!selectedQuestionId || mutationGuardRef.current.has('rubric-level-migration')) return;
    mutationGuardRef.current.add('rubric-level-migration');
    setSavingRubricId('level-migration');
    setError(null);
    try {
      const result = await commands.migrateRubricLevels({ projectId, questionId: selectedQuestionId });
      setSuccessMessage(
        result.migratedCount > 0
          ? 'Eski rubrik seviyeleri öğretmen incelemesi için öneri olarak taşındı.'
          : 'Bu rubrik için taşınacak eski seviye verisi bulunmadı.',
      );
      await invalidateWorkspace();
    } catch (caught) {
      setError(caught as AppError);
    } finally {
      mutationGuardRef.current.delete('rubric-level-migration');
      setSavingRubricId(null);
    }
  };

  const queryError = (
    projectQuery.error
    ?? workflowQuery.error
    ?? rubricItemsQuery.error
    ?? validationQuery.error
    ?? questionStatusQuery.error
  ) as AppError | null;

  if (isResolving) return <ProjectContextState pageLabel="Sınav Paketi" loading projectPath={projectPath} />;
  if (!projectId || !project || !workflow || !summary) return <ProjectContextState pageLabel="Sınav Paketi" projectPath={projectPath} />;

  const previousId = nextQuestionId(orderedQuestionIds, selectedQuestionId, -1);
  const followingId = nextQuestionId(orderedQuestionIds, selectedQuestionId, 1);
  const questionBusy = activeJobs.some((job) => job.kind === 'question_text_extraction');
  const rubricBusy = activeJobs.some((job) => job.kind === 'rubric_pdf_import' || job.kind === 'exam_package_build');
  const selectedRubricDraft = selectedQuestionId ? rubricDrafts[selectedQuestionId] : undefined;

  const examSourceDoc = project.documents.find((doc) => doc.role === 'exam_source');
  const answerKeyDoc = project.documents.find((doc) => doc.role === 'answer_key' || doc.role === 'rubric');
  const examQuestionsUploaded = !!examSourceDoc;
  const answerKeyUploaded = !!answerKeyDoc;

  return (
    <div className="exam-package-workspace">
      <header className="exam-package-workspace__header">
        <div>
          <h2>Sınav Hazırlığı</h2>
          <p>Sınav belgelerini, soru metinlerini ve puanlama rubriklerini bu çalışma alanında kontrol edip tamamlayın.</p>
        </div>
        <div style={{ display: 'flex', gap: '0.75rem', alignItems: 'center' }}>
          <span className={`package-status ${summary.frozen ? 'is-ready' : summary.invalidated ? 'is-warning' : ''}`}>
            {freezeUiState.statusText}
          </span>
          <LoadingButton
            type="button"
            projectWrite={false}
            className="button button--primary"
            loading={freezeMutation.isPending}
            disabledReason={freezeUiState.freezeButtonDisabledReason}
            onClick={() => setFreezeDialogOpen(true)}
          >
            <CheckCircle2 size={17} /> Sınav Hazırlığını Tamamla
          </LoadingButton>
        </div>
      </header>

      <dl className="exam-package-summary" aria-label="Sınav hazırlık özeti">
        <div><dt>Sınav soruları</dt><dd>{examQuestionsUploaded ? 'Yüklendi' : 'Eksik'}</dd></div>
        <div><dt>Cevap anahtarı</dt><dd>{answerKeyUploaded ? 'Yüklendi' : 'Eksik'}</dd></div>
        <div><dt>Rubrikler</dt><dd>{summary.readyRubricCount}/{summary.totalQuestions} hazır</dd></div>
        <div><dt>Sınav toplamı</dt><dd>{summary.totalScore} puan</dd></div>
      </dl>

      {(error || queryError) && <ErrorBanner error={(error || queryError)!} />}
      {successMessage && <div className="package-notice package-notice--success" role="status"><CheckCircle2 size={17} />{successMessage}</div>}
      {activeJobs.length > 0 && (
        <div className="package-local-jobs" role="status" aria-live="polite">
          <Loader2 size={18} className="animate-spin" aria-hidden="true" />
          <div>
            <strong>İşlem devam ediyor</strong>
            {activeJobs.map((job) => <span key={job.id}>{job.progress.message || 'İşlem sürüyor.'} ({job.progress.current}/{job.progress.total})</span>)}
          </div>
        </div>
      )}

      <div className="exam-package-tabs" role="tablist" aria-label="Sınav hazırlığı bölümleri">
        {([
          ['documents', 'Belgeler'],
          ['question', 'Sorular'],
          ['rubric', 'Rubrikler'],
        ] as const).map(([value, label]) => (
          <button
            key={value}
            type="button"
            data-project-write="false"
            role="tab"
            aria-selected={tab === value || (tab === 'freeze' && value === 'rubric')}
            className={tab === value || (tab === 'freeze' && value === 'rubric') ? 'is-active' : ''}
            onClick={() => updateSearch(location.search, navigate, projectId, { tab: value })}
          >
            {label}
          </button>
        ))}
      </div>

      {tab === 'documents' ? (
        <div style={{ marginTop: '1rem' }}>
          <DocumentsPage hideHeader />
        </div>
      ) : (
        <div className={`exam-package-layout ${tab === 'freeze' ? 'is-freeze' : ''}`}>
        {tab !== 'freeze' && (
          <nav className="package-question-list" aria-label="Soru listesi">
            <div className="package-question-list__heading">
              <strong>Sorular</strong>
              <span>{summary.readyQuestionCount}/{summary.totalQuestions} metin · {summary.readyRubricCount}/{summary.totalQuestions} rubrik</span>
            </div>
            <div className="package-question-list__items" role="listbox" aria-label="İncelenecek soruyu seçin">
              {questionItems.map((item) => (
                <button
                  key={item.id}
                  type="button"
                  data-project-write="false"
                  role="option"
                  aria-selected={selectedQuestionId === item.id}
                  className={selectedQuestionId === item.id ? 'is-active' : ''}
                  onClick={() => updateSearch(location.search, navigate, projectId, { questionId: item.id })}
                >
                  <span className="package-question-list__number">{item.number}</span>
                  <span className="package-question-list__content">
                    <strong>Soru {item.number} · {item.maxScore} puan</strong>
                    <small>{item.questionLabel}</small>
                    <small>{item.rubricLabel}</small>
                  </span>
                  {item.needsReview ? <span className="package-question-list__review">Kontrol</span> : <CheckCircle2 size={17} aria-label="Hazır" />}
                </button>
              ))}
            </div>
          </nav>
        )}

        <section className="package-detail" aria-live="polite">
          {tab === 'question' && selectedQuestion && (
            <>
              <div className="package-detail__heading">
                <div>
                  <span className="package-detail__eyebrow">Soru {selectedQuestion.number}</span>
                  <h3>Soru Metni</h3>
                  <p>{answerTypeLabels[selectedQuestion.answerType] ?? 'Genel metin'} · {textFieldSourceLabels[selectedQuestion.questionText.source]}</p>
                </div>
                <span className={`package-status ${selectedQuestion.questionText.status === 'confirmed' || selectedQuestion.questionText.status === 'edited' ? 'is-ready' : 'is-warning'}`}>
                  {textFieldStatusLabels[selectedQuestion.questionText.status]}
                </span>
              </div>

              <div className="package-operation-bar">
                <LoadingButton
                  type="button"
                  onClick={() => void startQuestionExtraction(false)}
                  loading={questionBusy}
                  disabledReason={!questionStatusQuery.data?.previewReady ? 'Sınav önizlemesi arka planda hazırlanıyor.' : undefined}
                >
                  <PlayCircle size={16} /> PDF’den Yeniden Çıkar
                </LoadingButton>
                {questionStatusQuery.data?.visionFallbackAvailable && (
                  <LoadingButton type="button" onClick={() => void startQuestionExtraction(true)} loading={questionBusy}>
                    <RefreshCcw size={16} /> Görsel Okumayı Dene
                  </LoadingButton>
                )}
              </div>

              <div className="question-text-editor">
                <label htmlFor={`question-text-${selectedQuestion.id}`}>Soru metni</label>
                <textarea
                  id={`question-text-${selectedQuestion.id}`}
                  rows={10}
                  readOnly={editingQuestionId !== selectedQuestion.id}
                  value={questionDrafts[selectedQuestion.id] ?? selectedQuestion.questionText.value}
                  onChange={(event) => {
                    setQuestionDrafts((current) => ({ ...current, [selectedQuestion.id]: event.target.value }));
                    setDirtyQuestionIds((current) => new Set(current).add(selectedQuestion.id));
                  }}
                />
                <div className="question-text-editor__meta">
                  <span>Maksimum puan: {selectedQuestion.rubric.maxScore ?? selectedQuestion.maxScore}</span>
                  {selectedQuestion.questionText.confidence !== undefined && <span>Okuma güveni: %{Math.round(selectedQuestion.questionText.confidence * 100)}</span>}
                </div>
                <div className="question-text-editor__actions">
                  {editingQuestionId !== selectedQuestion.id ? (
                    <button type="button" data-project-write="false" className="button button--secondary" onClick={() => setEditingQuestionId(selectedQuestion.id)}>Düzenle</button>
                  ) : (
                    <button type="button" data-project-write="false" className="button button--secondary" onClick={() => {
                      setQuestionDrafts((current) => ({ ...current, [selectedQuestion.id]: selectedQuestion.questionText.value }));
                      setDirtyQuestionIds((current) => {
                        const next = new Set(current);
                        next.delete(selectedQuestion.id);
                        return next;
                      });
                      setEditingQuestionId(null);
                    }}>İptal</button>
                  )}
                  {editingQuestionId === selectedQuestion.id && (
                    <LoadingButton type="button" className="button button--secondary" loading={savingQuestionId === selectedQuestion.id} onClick={() => void saveQuestion(false)}>
                      <Save size={16} /> Kaydet
                    </LoadingButton>
                  )}
                  <LoadingButton type="button" className="button button--primary" loading={savingQuestionId === selectedQuestion.id} onClick={() => void saveQuestion(true)}>
                    <CheckCircle2 size={16} /> {editingQuestionId === selectedQuestion.id ? 'Kaydet ve Onayla' : 'Onayla'}
                  </LoadingButton>
                </div>
              </div>

              {selectedQuestion.questionText.warnings.length > 0 && (
                <div className="package-warning"><AlertCircle size={17} />Çıkarma sırasında kontrol edilmesi gereken bir uyarı kaydedildi.</div>
              )}
              <details className="package-technical-details">
                <summary>Teknik tanı ayrıntıları</summary>
                <dl>
                  <div><dt>Soru kimliği</dt><dd>{selectedQuestion.id}</dd></div>
                  <div><dt>Durum</dt><dd>{selectedQuestion.questionText.status}</dd></div>
                  <div><dt>Uyarılar</dt><dd>{selectedQuestion.questionText.warnings.join(', ') || 'Yok'}</dd></div>
                </dl>
              </details>
            </>
          )}

          {tab === 'rubric' && (
            <>
              <div className="package-detail__heading">
                <div>
                  <span className="package-detail__eyebrow">{selectedQuestion ? `Soru ${selectedQuestion.number}` : 'Rubrik'}</span>
                  <h3>Rubrik</h3>
                  <p>Beklenen cevap, ölçütler ve puanlama notları</p>
                </div>
                {selectedQuestion && <span className={`package-status ${selectedQuestion.rubric.status === 'confirmed' ? 'is-ready' : 'is-warning'}`}>{rubricStatusLabels[selectedQuestion.rubric.status]}</span>}
              </div>
              <div className="package-operation-bar">
                <LoadingButton type="button" loading={savingRubricId === 'import'} onClick={() => void importRubricJson()}>
                  <FileUp size={16} /> JSON Yükle
                </LoadingButton>
                <LoadingButton type="button" projectWrite={false} loading={rubricBusy} onClick={() => setQuestionCountDialogOpen(true)}>
                  <FileSearch size={16} /> PDF’den Yeniden Çıkar
                </LoadingButton>
                <LoadingButton type="button" projectWrite={false} loading={validationQuery.isFetching} onClick={() => void validationQuery.refetch()}>
                  <CheckCircle2 size={16} /> Rubrikleri Doğrula
                </LoadingButton>
                <LoadingButton
                  type="button"
                  loading={savingRubricId === 'level-migration'}
                  disabledReason={!selectedQuestionId ? 'Önce bir soru seçin.' : undefined}
                  onClick={() => void migrateRubricLevels()}
                >
                  <RefreshCcw size={16} /> Eski seviyeleri öneriye taşı
                </LoadingButton>
              </div>
              {selectedRubricItem && selectedRubricDraft ? (
                <RubricQuestionCard
                  key={selectedRubricItem.question.id}
                  item={selectedRubricItem}
                  draft={selectedRubricDraft}
                  saving={savingRubricId === selectedRubricItem.question.id}
                  onDraftChange={(questionId, next) => {
                    setRubricDrafts((current) => ({ ...current, [questionId]: next }));
                    setDirtyRubricIds((current) => new Set(current).add(questionId));
                  }}
                  onSave={(questionId) => saveRubric(questionId, false)}
                  onConfirm={(questionId) => saveRubric(questionId, true)}
                />
              ) : (
                <div className="package-empty-state">Bu soru için rubrik verisi bulunamadı.</div>
              )}
              {selectedRubricItem && (
                <details className="package-technical-details">
                  <summary>Teknik tanı ayrıntıları</summary>
                  <dl>
                    <div><dt>Soru kimliği</dt><dd>{selectedRubricItem.question.id}</dd></div>
                    <div><dt>Rubrik durumu</dt><dd>{selectedRubricItem.question.rubric.status}</dd></div>
                    <div><dt>Ham uyarılar</dt><dd>{selectedRubricItem.validation.warnings.join(', ') || 'Yok'}</dd></div>
                  </dl>
                </details>
              )}
            </>
          )}

          {tab === 'freeze' && (
            <div className="package-freeze-panel">
              <div className="package-detail__heading">
                <div>
                  <span className="package-detail__eyebrow">Paket ve Dondurma</span>
                  <h3>Notlandırma paketini hazırla</h3>
                  <p>Backend doğrulamasının bildirdiği kapsamı ve engelleri inceleyin.</p>
                </div>
                <LockKeyhole size={32} aria-hidden="true" />
              </div>

              <div className="package-freeze-grid">
                <article><span>Soru kapsamı</span><strong>{summary.readyQuestionCount}/{summary.totalQuestions}</strong></article>
                <article><span>Rubrik kapsamı</span><strong>{summary.readyRubricCount}/{summary.totalQuestions}</strong></article>
                <article><span>Toplam sınav puanı</span><strong>{summary.totalScore}</strong></article>
                <article><span>Dondurmaya hazır</span><strong>{summary.freezeReady ? 'Evet' : 'Hayır'}</strong></article>
              </div>

              {summary.frozen && project.examPackageFreeze ? (
                <div className="frozen-package-callout">
                  <CheckCircle2 size={22} />
                  <div>
                    <strong>Dondurulmuş notlandırma paketi</strong>
                    <span>{new Intl.DateTimeFormat('tr-TR', { dateStyle: 'long', timeStyle: 'short' }).format(new Date(project.examPackageFreeze.frozenAt))} tarihinde oluşturuldu.</span>
                    <span>Taslakta daha sonra yapılan değişiklikler bu paketi geçersiz kılar ve yeniden dondurma gerektirir.</span>
                  </div>
                </div>
              ) : summary.invalidated ? (
                <div className="package-warning"><AlertCircle size={18} />Önceki dondurulmuş paket taslak değişikliği nedeniyle geçersiz. Notlandırmadan önce yeniden dondurun.</div>
              ) : null}

              <section className="package-blockers" aria-labelledby="package-blockers-title">
                <h4 id="package-blockers-title">Dondurma engelleri</h4>
                {workflow.blockingReasons.length === 0 && (validation?.blockingQuestions.length ?? 0) === 0 ? (
                  <p>Backend herhangi bir dondurma engeli bildirmiyor.</p>
                ) : (
                  <ul>
                    {workflow.blockingReasons.map((reason) => <li key={reason}>{blockingReasonLabels[reason] ?? 'Çalışma akışında tamamlanması gereken bir adım var.'}</li>)}
                    {validation?.blockingQuestions.map((number) => <li key={`question-${number}`}>Soru {number} rubriğinde doğrulama sorunu var.</li>)}
                  </ul>
                )}
              </section>

              <div className="package-freeze-actions">
                <p>{freezeUiState.nextStepText}</p>
                <LoadingButton
                  type="button"
                  projectWrite={false}
                  className="button button--primary"
                  loading={freezeMutation.isPending}
                  disabledReason={freezeUiState.freezeButtonDisabledReason}
                  onClick={() => setFreezeDialogOpen(true)}
                >
                  <LockKeyhole size={17} /> Sınav Paketini Dondur
                </LoadingButton>
              </div>

              {project.examPackageFreeze && (
                <details className="package-technical-details">
                  <summary>Teknik tanı ayrıntıları</summary>
                  <dl>
                    <div><dt>Paket sürümü</dt><dd>{project.examPackageFreeze.examPackageVersion}</dd></div>
                    <div><dt>Kaynak özeti</dt><dd>{project.examPackageFreeze.sourceHash}</dd></div>
                    <div><dt>Soru metni özeti</dt><dd>{project.examPackageFreeze.questionTextHash}</dd></div>
                    <div><dt>Rubrik özeti</dt><dd>{project.examPackageFreeze.rubricHash}</dd></div>
                  </dl>
                </details>
              )}
            </div>
          )}

          {tab !== 'freeze' && (
            <footer className="package-question-navigation">
              <button type="button" data-project-write="false" className="button button--secondary" disabled={!previousId} onClick={() => updateSearch(location.search, navigate, projectId, { questionId: previousId })}><ArrowLeft size={16} /> Önceki Soru</button>
              {tab === 'question' ? (
                <button type="button" data-project-write="false" className="button button--primary" onClick={() => updateSearch(location.search, navigate, projectId, { tab: 'rubric' })}>Rubriğe Geç <ArrowRight size={16} /></button>
              ) : followingId ? (
                <button type="button" data-project-write="false" className="button button--primary" onClick={() => updateSearch(location.search, navigate, projectId, { tab: 'question', questionId: followingId })}>Sonraki Soru <ArrowRight size={16} /></button>
              ) : (
                <button type="button" data-project-write="false" className="button button--primary" onClick={() => updateSearch(location.search, navigate, projectId, { tab: 'freeze' })}>Paket Özetine Geç <ArrowRight size={16} /></button>
              )}
            </footer>
          )}
        </section>
      </div>
      )}

      <ConfirmationDialog
        open={freezeDialogOpen}
        title="Sınav hazırlığını tamamla"
        description="Geçerli soru metinleri ve rubrikler notlandırmada kullanılmak üzere kilitlenecek ve onaylanacak. Taslakta daha sonra yapılacak değişiklikler bu hazırlığı geçersiz kılar."
        confirmLabel="Hazırlığı Tamamla"
        busy={freezeMutation.isPending}
        onCancel={() => setFreezeDialogOpen(false)}
        onConfirm={confirmFreeze}
      />

      <QuestionCountDialog
        open={questionCountDialogOpen}
        title="Soru sayısını doğrulayın"
        description="Cevap anahtarı veya rubrik PDF’inden kaç soru çıkarılacağını belirtin."
        confirmLabel="Rubriği Çıkar"
        initialValue={project.expectedQuestionCount ?? (project.questions.length || 1)}
        onCancel={() => setQuestionCountDialogOpen(false)}
        onConfirm={async (expectedQuestionCount) => {
          if (rubricBusy || jobStartGuardRef.current.has('rubric-import')) return false;
          jobStartGuardRef.current.add('rubric-import');
          try {
            await commands.startRubricPdfImport({ projectId, expectedQuestionCount });
            setSuccessMessage('Rubrik çıkarma işi başlatıldı.');
            await invalidateWorkspace();
            return true;
          } catch (caught) {
            setError(caught as AppError);
            return false;
          } finally {
            jobStartGuardRef.current.delete('rubric-import');
          }
        }}
      />
    </div>
  );
}

export function ExamPackageCompatibilityRedirect({ tab }: { tab: ExamPackageTab }) {
  const { projectId, isResolving } = useProjectContext();
  const location = useLocation();
  if (isResolving) return <ProjectContextState pageLabel="Sınav Paketi" loading />;
  if (!projectId) return <ProjectContextState pageLabel="Sınav Paketi" />;
  return <RedirectToExamPackage projectId={projectId} tab={tab} search={location.search} />;
}

function RedirectToExamPackage({ projectId, tab, search }: { projectId: string; tab: ExamPackageTab; search: string }) {
  const navigate = useNavigate();
  useEffect(() => {
    navigate(projectExamPackagePath(projectId, tab, search), { replace: true });
  }, [navigate, projectId, search, tab]);
  return <ProjectContextState pageLabel="Sınav Paketi" loading />;
}

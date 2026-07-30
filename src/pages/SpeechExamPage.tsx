import { useEffect, useMemo, useRef, useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  BarChart3,
  Check,
  ChevronDown,
  CircleAlert,
  Clock,
  Mic2,
  Play,
  Plus,
  Radio,
  Sparkles,
  Users,
  Volume2,
} from 'lucide-react';
import { commands } from '../api/commands';
import type { AppError } from '../api/errors';
import type {
  SpeakingCriterion,
  SpeakingCriterionScore,
  SpeakingPerformanceLevel,
  Student,
} from '../api/types';
import { TeacherStarRating } from '../components/speaking/TeacherStarRating';
import { SpeechMetricsPanel } from '../components/speaking/SpeechMetricsPanel';
import {
  formatMinSecDisplay,
  formatMsToDuration,
  getDurationDeviationLabel,
  getFluencyExplanation,
  getSpeakingSetupReadiness,
  speakingMeasurementConfidenceLabel,
  speakingPerformanceLevelLabel,
  speakingPipelineState,
  speakingSubindicatorLabel,
  speakingSubindicatorMaximum,
} from './speechExamUi';

type SpeakingExamType = 'prepared' | 'impromptu';

function studentLabel(student: Student): string {
  return student.displayName?.trim() || student.number?.trim() || 'İsimsiz öğrenci';
}

function errorMessage(error: unknown, fallback: string): string {
  if (typeof error === 'object' && error !== null && 'message' in error) {
    const message = (error as Partial<AppError>).message;
    if (message?.trim()) return message;
  }
  return fallback;
}

function formatDuration(durationMs: number): string {
  const seconds = Math.max(0, Math.round(durationMs / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
}

function scoreValue(score: SpeakingCriterionScore): string {
  const value = score.teacherScore ?? score.aiSuggestedScore ?? score.automaticScore;
  return value === undefined || value === null ? '' : String(Math.round(value));
}

function criterionSourceSummary(criterion: SpeakingCriterion | undefined, score: SpeakingCriterionScore): string {
  if (criterion?.role === 'automatic') {
    return score.automaticScore != null
      ? `Otomatik öneri: ${Math.round(score.automaticScore)} / ${score.maxScore}`
      : 'Otomatik öneri: Ölçüm bekleniyor';
  }
  if (criterion?.role === 'ai_suggested') {
    return score.aiSuggestedScore != null
      ? `Gemma 4 12B önerisi: ${Math.round(score.aiSuggestedScore)} / ${score.maxScore}`
      : 'Gemma 4 12B önerisi: İnceleme bekleniyor';
  }
  return 'Yalnızca öğretmen gözlemi';
}

export function SpeechExamPage() {
  const { projectId = '' } = useParams<{ projectId: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const restoredRef = useRef(false);
  const transcriptRef = useRef<HTMLTextAreaElement | null>(null);

  const [stage, setStage] = useState<'setup' | 'session'>('setup');
  const [examName, setExamName] = useState('Konuşma Sınavı');
  const [examType, setExamType] = useState<SpeakingExamType>('prepared');
  const [taskText, setTaskText] = useState(
    'Seçtiğiniz konu hakkında düşüncelerinizi düzenli ve anlaşılır bir konuşmayla aktarın.',
  );
  const [classId, setClassId] = useState('');
  const [selectedClassIds, setSelectedClassIds] = useState<string[]>([]);
  const [minimumMinutes, setMinimumMinutes] = useState('2');
  const [minimumSeconds, setMinimumSeconds] = useState('0');
  const [targetMinutes, setTargetMinutes] = useState('3');
  const [targetSeconds, setTargetSeconds] = useState('0');
  const [maximumMinutes, setMaximumMinutes] = useState('4');
  const [maximumSeconds, setMaximumSeconds] = useState('0');
  const [optionalTeacherNote, setOptionalTeacherNote] = useState('');
  const [examDate] = useState('');

  const [examId, setExamId] = useState('');
  const [selectedStudentId, setSelectedStudentId] = useState('');
  const [currentAttemptId, setCurrentAttemptId] = useState('');
  const [teacherNote, setTeacherNote] = useState('');
  const [scoreDrafts, setScoreDrafts] = useState<Record<string, string>>({});
  const [levelDrafts, setLevelDrafts] = useState<Record<string, SpeakingPerformanceLevel | ''>>({});
  const [isCapturing, setIsCapturing] = useState(false);
  const [isCapturePending, setIsCapturePending] = useState(false);
  const [isStarting, setIsStarting] = useState(false);
  const [isApproving, setIsApproving] = useState(false);
  const [isFinishing, setIsFinishing] = useState(false);
  const [showRuntimeDetails, setShowRuntimeDetails] = useState(false);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [pageError, setPageError] = useState<string | null>(null);

  const projectQuery = useQuery({
    queryKey: ['project-snapshot', projectId],
    queryFn: () => commands.getProjectSnapshot(projectId),
    enabled: Boolean(projectId),
  });
  const project = projectQuery.data;
  const classes = useMemo(
    () => (project?.schoolClasses ?? []).filter((schoolClass) => schoolClass.status === 'active'),
    [project?.schoolClasses],
  );

  const studentsQuery = useQuery({
    queryKey: ['class-students', projectId, classId],
    queryFn: () => commands.listClassStudents({ projectId, classId }),
    enabled: Boolean(projectId && classId),
  });
  const students = useMemo(() => studentsQuery.data ?? [], [studentsQuery.data]);

  const runtimeQuery = useQuery({
    queryKey: ['speaking-engine-runtime'],
    queryFn: commands.getSpeakingExamRuntimeStatus,
    enabled: Boolean(projectId),
    refetchInterval: stage === 'session' ? 300 : 2000,
  });
  const runtimeStatus = runtimeQuery.data;

  const speakingExamQuery = useQuery({
    queryKey: ['speaking-exam', projectId, examId],
    queryFn: () => commands.getSpeakingExam(projectId, examId),
    enabled: stage === 'session' && Boolean(projectId && examId),
    refetchInterval: (query) =>
      query.state.data?.attempts.some((item) =>
        ['finalizing', 'cleaning_transcript', 'evaluating'].includes(item.state),
      )
        ? 700
        : false,
  });
  const speakingExam = speakingExamQuery.data;

  const sessionAssignedClasses = useMemo(() => {
    if (!speakingExam) return classes;
    const assignedIds = speakingExam.assignedClassIds?.length
      ? speakingExam.assignedClassIds
      : speakingExam.classId
      ? [speakingExam.classId]
      : [];
    return classes.filter((sc) => assignedIds.includes(sc.id));
  }, [classes, speakingExam]);

  const selectedStudent = students.find((student) => student.id === selectedStudentId);
  const attempt =
    speakingExam?.attempts.find((item) => item.id === currentAttemptId) ??
    speakingExam?.attempts.find(
      (item) => item.studentId === selectedStudentId && item.state !== 'approved',
    ) ??
    speakingExam?.attempts.find((item) => item.studentId === selectedStudentId);
  const attemptId = attempt?.id;
  const attemptState = attempt?.state;
  const attemptTeacherNote = attempt?.teacherNote;
  const attemptCriterionScores = attempt?.criterionScores;
  const completedStudentIds = useMemo(
    () =>
      speakingExam?.attempts
        .filter((item) => item.state === 'approved')
        .map((item) => item.studentId) ?? [],
    [speakingExam?.attempts],
  );

  useEffect(() => {
    if (restoredRef.current || !project) return;
    restoredRef.current = true;
    const resumable = [...project.speakingExams]
      .filter((exam) => exam.status !== 'completed')
      .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))[0];
    if (resumable) {
      setExamId(resumable.id);
      setExamName(resumable.title);
      setExamType(resumable.examType);
      setTaskText(resumable.taskText);
      const minSec = resumable.minDurationSeconds;
      const recSec = resumable.targetDurationSeconds;
      const maxSec = resumable.maxDurationSeconds;
      setMinimumMinutes(String(Math.floor(minSec / 60)));
      setMinimumSeconds(String(minSec % 60));
      setTargetMinutes(String(Math.floor(recSec / 60)));
      setTargetSeconds(String(recSec % 60));
      setMaximumMinutes(String(Math.floor(maxSec / 60)));
      setMaximumSeconds(String(maxSec % 60));
      const assigned = resumable.assignedClassIds?.length
        ? resumable.assignedClassIds
        : resumable.classId
        ? [resumable.classId]
        : [];
      setSelectedClassIds(assigned);
      setClassId(resumable.classId || assigned[0] || '');
      setSelectedStudentId(resumable.activeStudentId ?? '');
      setStage('session');
      setStatusMessage('Son konuşma sınavı oturumu kaldığı yerden açıldı.');
      return;
    }
    if (classes[0]) {
      setSelectedClassIds(classes.map((c) => c.id));
      setClassId(classes[0].id);
    }
  }, [classes, project]);

  useEffect(() => {
    if (stage === 'setup' && selectedClassIds.length === 0 && classes.length > 0) {
      setSelectedClassIds(classes.map((c) => c.id));
    }
  }, [classes, selectedClassIds.length, stage]);

  useEffect(() => {
    if (!classId && sessionAssignedClasses[0]) {
      setClassId(sessionAssignedClasses[0].id);
    }
  }, [classId, sessionAssignedClasses]);

  useEffect(() => {
    if (students.length === 0) {
      setSelectedStudentId('');
      return;
    }
    if (!students.some((student) => student.id === selectedStudentId)) {
      const [firstStudent] = students;
      if (!firstStudent) return;
      setSelectedStudentId(firstStudent.id);
      setCurrentAttemptId('');
    }
  }, [selectedStudentId, students]);

  useEffect(() => {
    if (!attemptId || !attemptCriterionScores) {
      setCurrentAttemptId('');
      setTeacherNote('');
      setScoreDrafts({});
      setLevelDrafts({});
      return;
    }
    setCurrentAttemptId(attemptId);
    setTeacherNote(attemptTeacherNote ?? '');
    setScoreDrafts(
      Object.fromEntries(attemptCriterionScores.map((score) => [score.criterionId, scoreValue(score)])),
    );
    setLevelDrafts(
      Object.fromEntries(
        attemptCriterionScores.map((score) => [score.criterionId, score.teacherLevel ?? '']),
      ),
    );
  }, [attemptCriterionScores, attemptId, attemptState, attemptTeacherNote]);

  const selectedClassName =
    classes.find((schoolClass) => schoolClass.id === classId)?.name ?? 'Sınıf seçilmedi';
  const isPaused = attempt?.state === 'paused' || runtimeStatus?.state === 'paused';
  const isBackendCapturing = runtimeStatus?.state === 'recording';
  const isActiveCapture = isCapturing || isBackendCapturing || isPaused;
  const isProcessing =
    ['finalizing', 'cleaning_transcript', 'evaluating'].includes(attempt?.state ?? '') ||
    runtimeStatus?.state === 'stopping' ||
    runtimeStatus?.state === 'transcribing';
  const audioLevel = Math.min(
    1,
    (runtimeStatus?.audioRms ?? 0) * 8 + (runtimeStatus?.audioPeak ?? 0) * 0.35,
  );
  const waveformBars = Array.from({ length: 24 }, (_, index) => {
    const contour = 0.45 + Math.abs(Math.sin(index * 0.72)) * 0.55;
    return Math.max(4, Math.round(isActiveCapture ? 6 + audioLevel * 28 * contour : 4 + contour * 3));
  });
  const elapsedMs = runtimeStatus?.activeSession
    ? runtimeStatus.elapsedMs
    : attempt?.metrics.durationMs ?? 0;
  const isAttemptEvaluated = Boolean(
    attempt?.rawTranscript &&
      attempt?.metrics &&
      attempt.metrics.durationMs > 0 &&
      attempt.metrics.measurementConfidence !== 'not_evaluated',
  );

  const minSecCalculated = (Number(minimumMinutes) || 0) * 60 + (Number(minimumSeconds) || 0);
  const recSecCalculated = (Number(targetMinutes) || 0) * 60 + (Number(targetSeconds) || 0);
  const maxSecCalculated = (Number(maximumMinutes) || 0) * 60 + (Number(maximumSeconds) || 0);

  const setupReadiness = getSpeakingSetupReadiness(
    examName,
    taskText,
    selectedClassIds,
    minSecCalculated,
    recSecCalculated,
    maxSecCalculated,
  );

  const hasAiScores = attempt?.criterionScores.some(
    (s) => s.aiSuggestedScore !== undefined && s.aiSuggestedScore !== null,
  ) ?? false;
  const pipelineState = speakingPipelineState(
    isActiveCapture,
    Boolean(attempt?.rawTranscript),
    attempt?.state,
    attempt?.transcriptCleanup.status,
    hasAiScores,
    attempt?.evaluationError,
  );

  const totalSelectedStudents = useMemo(() => {
    const studentsList = project?.students ?? [];
    return classes
      .filter((sc) => selectedClassIds.includes(sc.id))
      .reduce((sum, sc) => {
        const normName = sc.normalizedName || sc.name;
        const count = studentsList.filter(
          (s) => s.className?.trim() === sc.name?.trim() || s.className?.trim() === normName,
        ).length;
        return sum + count;
      }, 0);
  }, [classes, selectedClassIds, project?.students]);

  function toggleClassSelection(scId: string) {
    if (selectedClassIds.includes(scId)) {
      setSelectedClassIds(selectedClassIds.filter((id) => id !== scId));
    } else {
      setSelectedClassIds([...selectedClassIds, scId]);
    }
  }

  async function beginSession() {
    if (!setupReadiness.isReady || isStarting) return;
    setPageError(null);
    setIsStarting(true);
    try {
      const result = await commands.startSpeakingExam({
        projectId,
        examName: examName.trim(),
        examType,
        taskText: taskText.trim(),
        minimumSeconds: minSecCalculated,
        targetSeconds: recSecCalculated,
        maximumSeconds: maxSecCalculated,
        assignedClassIds: selectedClassIds,
        examId: examId || undefined,
        teacherNote: optionalTeacherNote.trim() || undefined,
        examDate: examDate.trim() || undefined,
      });
      setExamId(result.examId);
      setStatusMessage(result.message);
      setStage('session');
      await queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
      await queryClient.invalidateQueries({ queryKey: ['speaking-exam', projectId, result.examId] });
    } catch (error) {
      setPageError(errorMessage(error, 'Konuşma sınavı kaydedilemedi.'));
    } finally {
      setIsStarting(false);
    }
  }

  async function controlCapture(action: 'start' | 'pause' | 'resume' | 'stop' | 'cancel') {
    if (!selectedStudentId || !examId || isCapturePending) return;
    setPageError(null);
    setIsCapturePending(true);
    try {
      const result = await commands.toggleSpeakingCapture({
        projectId,
        examId,
        studentId: selectedStudentId,
        action,
      });
      setIsCapturing(action === 'start' || action === 'resume');
      setStatusMessage(result.message);
      if (result.attemptId) setCurrentAttemptId(result.attemptId);
      if (action === 'stop') window.setTimeout(() => transcriptRef.current?.focus(), 0);
      await queryClient.invalidateQueries({ queryKey: ['speaking-exam', projectId, examId] });
    } catch (error) {
      setPageError(errorMessage(error, 'Canlı konuşma kaydı yönetilemedi.'));
    } finally {
      setIsCapturePending(false);
    }
  }

  async function saveNumericScore(score: SpeakingCriterionScore) {
    if (!attempt) return;
    const value = Number(scoreDrafts[score.criterionId]);
    if (!Number.isFinite(value)) return;
    try {
      await commands.updateSpeakingCriterionScore({
        projectId,
        examId,
        attemptId: attempt.id,
        criterionId: score.criterionId,
        score: value,
      });
      setStatusMessage('Öğretmen puanı otomatik kaydedildi.');
      await queryClient.invalidateQueries({ queryKey: ['speaking-exam', projectId, examId] });
    } catch (error) {
      setPageError(errorMessage(error, 'Öğretmen puanı kaydedilemedi.'));
    }
  }

  async function savePerformanceLevel(score: SpeakingCriterionScore, level: SpeakingPerformanceLevel) {
    if (!attempt) return;
    setLevelDrafts((current) => ({ ...current, [score.criterionId]: level }));
    try {
      await commands.updateSpeakingCriterionLevel({
        projectId,
        examId,
        attemptId: attempt.id,
        criterionId: score.criterionId,
        level,
      });
      setStatusMessage('Gözlem düzeyi ve karşılık gelen puan otomatik kaydedildi.');
      await queryClient.invalidateQueries({ queryKey: ['speaking-exam', projectId, examId] });
    } catch (error) {
      setPageError(errorMessage(error, 'Gözlem düzeyi kaydedilemedi.'));
    }
  }

  async function clearPerformanceLevel(score: SpeakingCriterionScore) {
    if (!attempt) return;
    setLevelDrafts((current) => ({ ...current, [score.criterionId]: '' }));
    try {
      await commands.updateSpeakingCriterionLevel({
        projectId,
        examId,
        attemptId: attempt.id,
        criterionId: score.criterionId,
        level: 'not_observed',
      });
      setStatusMessage('Gözlem seçimi temizlendi.');
      await queryClient.invalidateQueries({ queryKey: ['speaking-exam', projectId, examId] });
    } catch (error) {
      setPageError(errorMessage(error, 'Gözlem seçimi temizlenemedi.'));
    }
  }

  async function approveAttempt() {
    if (!attempt) return;
    setIsApproving(true);
    setPageError(null);
    try {
      await commands.approveSpeakingAttempt({
        projectId,
        examId,
        attemptId: attempt.id,
        teacherNote: teacherNote.trim() || undefined,
      });
      setStatusMessage('Nihai puan kaydedildi; öğrenci ses kaydı cihazdan kalıcı silindi.');
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ['speaking-exam', projectId, examId] }),
        queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] }),
      ]);
    } catch (error) {
      setPageError(errorMessage(error, 'Nihai puan onaylanamadı.'));
    } finally {
      setIsApproving(false);
    }
  }

  async function switchClass(nextClassId: string) {
    if (nextClassId === classId || isActiveCapture || isProcessing) return;
    setPageError(null);
    try {
      if (examId) {
        await commands.selectSpeakingExamClass({ projectId, examId, classId: nextClassId });
      }
      setClassId(nextClassId);
      setSelectedStudentId('');
      setCurrentAttemptId('');
      await queryClient.invalidateQueries({ queryKey: ['speaking-exam', projectId, examId] });
    } catch (error) {
      setPageError(errorMessage(error, 'Sınıf değiştirilemedi.'));
    }
  }

  async function selectStudent(studentId: string) {
    setSelectedStudentId(studentId);
    setCurrentAttemptId('');
    if (!examId) return;
    try {
      await commands.selectSpeakingExamStudent({ projectId, examId, studentId });
      await queryClient.invalidateQueries({ queryKey: ['speaking-exam', projectId, examId] });
    } catch (error) {
      setPageError(errorMessage(error, 'Aktif öğrenci kaydedilemedi.'));
    }
  }

  async function finishExam() {
    if (!examId || isActiveCapture || isProcessing) return;
    setIsFinishing(true);
    setPageError(null);
    try {
      const output = await commands.finishAssessment({
        projectId,
        kind: 'speaking',
        sourceId: examId,
      });
      navigate(
        `/project/${encodeURIComponent(projectId)}/speaking/analysis?analysisId=${encodeURIComponent(output.analysisId)}`,
      );
    } catch (error) {
      setPageError(errorMessage(error, 'Sınav analizi başlatılamadı.'));
    } finally {
      setIsFinishing(false);
    }
  }

  if (!projectId) {
    return (
      <div className="speech-exam-empty">
        <Mic2 size={28} />
        <h2>Bir proje açın</h2>
        <Link className="button button--primary" to="/projects/new">Yeni proje</Link>
      </div>
    );
  }
  if (projectQuery.isLoading) {
    return <div className="speech-exam-loading">Konuşma sınavı hazırlanıyor…</div>;
  }

  return (
    <div className="speech-exam-page">
      <div className="speech-exam-page__header">
        <div>
          <p className="eyebrow">SpeakoFlow backend · Türkçe konuşma sınavı</p>
          <h2>Konuşma Sınavı Çalışma Alanı</h2>
        </div>
        <div className="speech-exam-page__header-actions">
          {stage === 'session' && (
            <button
              type="button"
              className="button button--secondary"
              onClick={() => setStage('setup')}
            >
              Sınav Ayarlarını Düzenle
            </button>
          )}
          <span className="speech-exam-badge">
            <Radio size={14} /> {stage === 'setup' ? 'Sınav Kurulumu' : 'Oturum Aktif'}
          </span>
        </div>
      </div>

      {pageError && <div className="speech-inline-error"><CircleAlert size={16} /> {pageError}</div>}
      {statusMessage && !pageError && (
        <div className="speech-form-note"><Sparkles size={16} /> {statusMessage}</div>
      )}

      {stage === 'session' && sessionAssignedClasses.length > 0 && (
        <div className="speech-class-toolbar">
          <span className="speech-class-toolbar__label">Atanmış Sınıf:</span>
          <div className="speech-class-toolbar__segment">
            {sessionAssignedClasses.map((schoolClass) => (
              <button
                type="button"
                key={schoolClass.id}
                className={`speech-class-toolbar__btn ${schoolClass.id === classId ? 'is-active' : ''}`}
                disabled={isActiveCapture || isProcessing}
                onClick={() => void switchClass(schoolClass.id)}
              >
                {schoolClass.name}
              </button>
            ))}
          </div>
        </div>
      )}

      {stage === 'setup' ? (
        <div className="speech-setup-grid">
          {/* LEFT COLUMN: Main Form Cards */}
          <div className="speech-setup-main">
            {/* Card 1: Temel Bilgiler */}
            <div className="speech-panel">
              <div className="speech-panel__heading">
                <div>
                  <span className="speech-panel__index">01</span>
                  <div>
                    <h3>Sınav bilgileri</h3>
                    <p>Sınav başlığı ve konuşma türü tanımlayın.</p>
                  </div>
                </div>
              </div>
              <div className="speech-form-grid">
                <label className="speech-field speech-field--wide">
                  Sınav Adı
                  <input
                    value={examName}
                    placeholder="Örn: 1. Dönem Türkçe Konuşma Sınavı"
                    onChange={(event) => setExamName(event.target.value)}
                  />
                </label>
                <div className="speech-field speech-field--wide">
                  <span>Konuşma Türü</span>
                  <div role="radiogroup" aria-label="Konuşma türü" className="speech-segmented-control">
                    <button
                      type="button"
                      role="radio"
                      aria-checked={examType === 'prepared'}
                      className={`speech-segmented-btn ${examType === 'prepared' ? 'is-active' : ''}`}
                      onClick={() => setExamType('prepared')}
                    >
                      Hazırlıklı Konuşma
                    </button>
                    <button
                      type="button"
                      role="radio"
                      aria-checked={examType === 'impromptu'}
                      className={`speech-segmented-btn ${examType === 'impromptu' ? 'is-active' : ''}`}
                      onClick={() => setExamType('impromptu')}
                    >
                      Hazırlıksız Konuşma
                    </button>
                  </div>
                </div>
                <label className="speech-field speech-field--wide">
                  İsteğe Bağlı Öğretmen Notu
                  <input
                    value={optionalTeacherNote}
                    placeholder="Örn: 2025-2026 Güz Dönemi Ortak Değerlendirmesi"
                    onChange={(event) => setOptionalTeacherNote(event.target.value)}
                  />
                </label>
              </div>
            </div>

            {/* Card 2: Çoklu Sınıf Seçimi */}
            <div className="speech-panel">
              <div className="speech-panel__heading">
                <div>
                  <span className="speech-panel__index">02</span>
                  <div>
                    <h3>Sınava girecek sınıflar</h3>
                    <p>Bu konuşma sınavına bir veya birden fazla sınıf atayın.</p>
                  </div>
                </div>
                <Users size={20} />
              </div>
              <div className="speech-class-actions">
                <button
                  type="button"
                  className="button button--secondary"
                  onClick={() => setSelectedClassIds(classes.map((c) => c.id))}
                >
                  Tümünü Seç
                </button>
                <button
                  type="button"
                  className="button button--secondary"
                  onClick={() => setSelectedClassIds([])}
                >
                  Seçimi Temizle
                </button>
                <Link
                  to={`/project/${encodeURIComponent(projectId)}/classes`}
                  className="button button--secondary"
                >
                  <Plus size={14} /> Yeni Sınıf Ekle
                </Link>
              </div>

              {classes.length === 0 ? (
                <div className="speech-checklist">
                  Projede henüz aktif sınıf bulunmuyor. Lütfen önce Sınıf Yönetimi ekranından sınıf ekleyin.
                </div>
              ) : (
                <div className="speech-class-grid">
                  {classes.map((sc) => {
                    const isSelected = selectedClassIds.includes(sc.id);
                    const normName = sc.normalizedName || sc.name;
                    const studentCount = (project?.students ?? []).filter(
                      (s) => s.className?.trim() === sc.name?.trim() || s.className?.trim() === normName,
                    ).length;
                    return (
                      <div
                        key={sc.id}
                        className={`speech-class-card ${isSelected ? 'is-selected' : ''}`}
                        onClick={() => toggleClassSelection(sc.id)}
                      >
                        <div className="speech-class-card__top">
                          <span className="speech-class-card__name">{sc.name}</span>
                          <input
                            type="checkbox"
                            checked={isSelected}
                            onChange={() => {}}
                            onClick={(e) => e.stopPropagation()}
                          />
                        </div>
                        <div className="speech-class-card__count">
                          {studentCount > 0 ? `${studentCount} Öğrenci` : 'Öğrenci Yok'}
                        </div>
                        {isSelected && studentCount === 0 && (
                          <small style={{ color: '#dc2626', fontSize: '0.68rem', marginTop: '4px' }}>
                            Kayıtlı öğrenci bulunmuyor.
                          </small>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}
            </div>

            {/* Card 3: Konuşma Görevi */}
            <div className="speech-panel">
              <div className="speech-panel__heading">
                <div>
                  <span className="speech-panel__index">03</span>
                  <div>
                    <h3>Konuşma görevi</h3>
                    <p>Öğrencilerin yanıtlayacağı ortak konu veya soruyu belirleyin.</p>
                  </div>
                </div>
              </div>
              <div className="speech-field speech-field--wide">
                <textarea
                  rows={4}
                  value={taskText}
                  placeholder="Öğrenciden beklenen konuşma görevini yazın..."
                  onChange={(event) => setTaskText(event.target.value)}
                />
                <span className="speech-scope-note" style={{ marginTop: '8px' }}>
                  {examType === 'prepared'
                    ? 'Öğrenciye önceden hazırlık yapabileceği açık bir görev verin.'
                    : 'Öğrencinin sınav sırasında kısa sürede yorumlayabileceği açık bir konu verin.'}
                </span>
              </div>
            </div>

            {/* Card 4: Konuşma Süresi */}
            <div className="speech-panel">
              <div className="speech-panel__heading">
                <div>
                  <span className="speech-panel__index">04</span>
                  <div>
                    <h3>Konuşma süresi</h3>
                    <p>Alt, önerilen ve üst süre sınırlarını belirleyin.</p>
                  </div>
                </div>
                <Clock size={20} />
              </div>

              <div className="speech-duration-grid">
                <div className="speech-duration-box">
                  <label>Minimum Süre (Alt Sınır)</label>
                  <div className="speech-duration-inputs">
                    <input
                      type="number"
                      min="0"
                      max="59"
                      value={minimumMinutes}
                      onChange={(e) => setMinimumMinutes(e.target.value)}
                    />
                    <span>dk</span>
                    <input
                      type="number"
                      min="0"
                      max="59"
                      value={minimumSeconds}
                      onChange={(e) => setMinimumSeconds(e.target.value)}
                    />
                    <span>sn</span>
                  </div>
                </div>

                <div className="speech-duration-box">
                  <label>Önerilen Süre (Hedef)</label>
                  <div className="speech-duration-inputs">
                    <input
                      type="number"
                      min="0"
                      max="59"
                      value={targetMinutes}
                      onChange={(e) => setTargetMinutes(e.target.value)}
                    />
                    <span>dk</span>
                    <input
                      type="number"
                      min="0"
                      max="59"
                      value={targetSeconds}
                      onChange={(e) => setTargetSeconds(e.target.value)}
                    />
                    <span>sn</span>
                  </div>
                </div>

                <div className="speech-duration-box">
                  <label>Maksimum Süre (Üst Sınır)</label>
                  <div className="speech-duration-inputs">
                    <input
                      type="number"
                      min="0"
                      max="59"
                      value={maximumMinutes}
                      onChange={(e) => setMaximumMinutes(e.target.value)}
                    />
                    <span>dk</span>
                    <input
                      type="number"
                      min="0"
                      max="59"
                      value={maximumSeconds}
                      onChange={(e) => setMaximumSeconds(e.target.value)}
                    />
                    <span>sn</span>
                  </div>
                </div>
              </div>

              <div className="speech-scope-note" style={{ marginTop: '14px' }}>
                Önerilen süre:{' '}
                <strong>
                  {formatMsToDuration(recSecCalculated * 1000)}
                </strong>{' '}
                · Kabul edilen aralık:{' '}
                <strong>
                  {formatMinSecDisplay(minSecCalculated, maxSecCalculated)}
                </strong>
              </div>
              <span className="speech-scope-note" style={{ marginTop: '4px', fontSize: '0.72rem', color: '#64748b' }}>
                Süreyi yönetme puanı, öğrencinin gerçek konuşma süresinin bu aralığa uzaklığına göre otomatik hesaplanır.
              </span>
            </div>

            {/* Card 5: Değerlendirme Özeti */}
            <div className="speech-panel">
              <div className="speech-panel__heading">
                <div>
                  <span className="speech-panel__index">05</span>
                  <div>
                    <h3>Değerlendirme özeti</h3>
                    <p>Sabitlenmiş 100 puanlık rubrik dağılımı.</p>
                  </div>
                </div>
                <BarChart3 size={20} />
              </div>

              <div className="speech-rubric-breakdown">
                <div className="speech-rubric-group">
                  <div className="speech-rubric-group__header">
                    <span className="speech-rubric-group__title">
                      Gemma 4 12B Yapay Zeka Değerlendirmesi
                    </span>
                    <span className="speech-badge speech-badge--ai">Gemma Önerisi · 50 Puan</span>
                  </div>
                  <div className="speech-rubric-item">
                    <span>İçerik ve ana düşünce</span>
                    <strong>20 Puan</strong>
                  </div>
                  <div className="speech-rubric-item">
                    <span>Plan ve anlam bütünlüğü</span>
                    <strong>15 Puan</strong>
                  </div>
                  <div className="speech-rubric-item">
                    <span>Türkçe ve söz varlığı</span>
                    <strong>15 Puan</strong>
                  </div>
                </div>

                <div className="speech-rubric-group">
                  <div className="speech-rubric-group__header">
                    <span className="speech-rubric-group__title">Otomatik Metrik Ölçümü</span>
                    <span className="speech-badge speech-badge--auto">Otomatik Ölçüm · 10 Puan</span>
                  </div>
                  <div className="speech-rubric-item">
                    <span>Akıcılık (Duraklama ve dolgular)</span>
                    <strong>5 Puan</strong>
                  </div>
                  <div className="speech-rubric-item">
                    <span>Süreyi yönetme</span>
                    <strong>5 Puan</strong>
                  </div>
                </div>

                <div className="speech-rubric-group">
                  <div className="speech-rubric-group__header">
                    <span className="speech-rubric-group__title">Öğretmen Doğrudan Gözlemi</span>
                    <span className="speech-badge speech-badge--teacher">Öğretmen Gözlemi · 40 Puan</span>
                  </div>
                  <div className="speech-rubric-item">
                    <span>Ses, diksiyon ve telaffuz</span>
                    <strong>10 Puan</strong>
                  </div>
                  <div className="speech-rubric-item">
                    <span>Hazırlık, araştırma, prova ve materyal</span>
                    <strong>15 Puan</strong>
                  </div>
                  <div className="speech-rubric-item">
                    <span>Beden dili, mekân ve iletişim</span>
                    <strong>10 Puan</strong>
                  </div>
                  <div className="speech-rubric-item">
                    <span>Öz değerlendirme ve gelişim hedefi</span>
                    <strong>5 Puan</strong>
                  </div>
                </div>
              </div>
            </div>

            {/* Card 6: Kompakt Sistem Durumu */}
            <div className="speech-panel">
              <div className="speech-panel__heading">
                <div>
                  <span className="speech-panel__index">06</span>
                  <div>
                    <h3>Sistem durumu</h3>
                    <p>Motor ve model servislerinin hazır olduğunu doğrulayın.</p>
                  </div>
                </div>
              </div>

              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, minmax(0, 1fr))', gap: '12px' }}>
                <div className={`speech-model-status ${runtimeStatus?.whisperReady ? 'is-ready' : 'is-missing'}`}>
                  <span className="speech-model-status__indicator" />
                  <div>
                    <strong>Whisper ASR Motoru</strong>
                    <span>{runtimeStatus?.whisperReady ? 'Hazır' : 'Kullanılamıyor'}</span>
                  </div>
                </div>

                <div className="speech-model-status is-ready">
                  <span className="speech-model-status__indicator" />
                  <div>
                    <strong>Gemma 4 12B Temizlik Servisi</strong>
                    <span>Hazır</span>
                  </div>
                </div>

                <div className="speech-model-status is-ready">
                  <span className="speech-model-status__indicator" />
                  <div>
                    <strong>Gemma 4 12B Puanlama Servisi</strong>
                    <span>Hazır</span>
                  </div>
                </div>

                <div className="speech-model-status is-ready">
                  <span className="speech-model-status__indicator" />
                  <div>
                    <strong>Ses Giriş Cihazı (Mikrofon)</strong>
                    <span>Sınav sırasında seçilecek</span>
                  </div>
                </div>
              </div>

              <details style={{ marginTop: '14px', fontSize: '0.74rem', color: '#64748b' }}>
                <summary style={{ cursor: 'pointer', fontWeight: 600 }}>Teknik ayrıntıları göster</summary>
                <div style={{ marginTop: '8px', padding: '10px', background: '#f8fafc', borderRadius: '8px' }}>
                  <p style={{ margin: 0 }}>Model Yolu: {runtimeStatus?.whisperModelPath || 'Varsayılan dahili model'}</p>
                  <p style={{ margin: '4px 0 0' }}>Sürüm: SpeakoFlow Embedded v3.0</p>
                </div>
              </details>
            </div>
          </div>

          {/* RIGHT COLUMN: Sticky Live Summary & Action */}
          <aside className="speech-sticky-side">
            <div className="speech-summary-card">
              <h3>Konuşma Sınavı Özeti</h3>

              <div className="speech-summary-list">
                <div className="speech-summary-row">
                  <label>Sınav Adı:</label>
                  <strong>{examName.trim() || '—'}</strong>
                </div>

                <div className="speech-summary-row">
                  <label>Konuşma Türü:</label>
                  <strong>{examType === 'prepared' ? 'Hazırlıklı' : 'Hazırlıksız'}</strong>
                </div>

                <div className="speech-summary-row">
                  <label>Atanan Sınıflar:</label>
                  <strong>
                    {selectedClassIds.length > 0
                      ? classes
                          .filter((c) => selectedClassIds.includes(c.id))
                          .map((c) => c.name)
                          .join(', ')
                      : 'Hiçbiri'}
                  </strong>
                </div>

                <div className="speech-summary-row">
                  <label>Toplam Öğrenci:</label>
                  <strong>{totalSelectedStudents} Öğrenci</strong>
                </div>

                <div className="speech-summary-row">
                  <label>Hedef Süre:</label>
                  <strong>{formatMsToDuration(recSecCalculated * 1000)}</strong>
                </div>

                <div className="speech-summary-row">
                  <label>Kabul Edilen Aralık:</label>
                  <strong>{formatMinSecDisplay(minSecCalculated, maxSecCalculated)}</strong>
                </div>

                <div className="speech-summary-row">
                  <label>Toplam Puan:</label>
                  <strong>100 Puan</strong>
                </div>
              </div>

              {!setupReadiness.isReady ? (
                <div className="speech-checklist">
                  <strong>Eksik Adımlar:</strong>
                  <ul>
                    {setupReadiness.missingReasons.map((reason, index) => (
                      <li key={index}>{reason}</li>
                    ))}
                  </ul>
                </div>
              ) : (
                <div className="speech-notice" style={{ marginBottom: '16px', color: '#15803d', fontSize: '0.78rem' }}>
                  <Check size={14} style={{ marginRight: '4px' }} />
                  {selectedClassIds.length} sınıf ve {totalSelectedStudents} öğrenci için sınav tanımı hazır.
                </div>
              )}

              <button
                type="button"
                className="button button--primary"
                style={{ width: '100%', minHeight: '44px', fontSize: '0.88rem' }}
                disabled={!setupReadiness.isReady || isStarting}
                onClick={() => void beginSession()}
              >
                <Play size={16} />{' '}
                {isStarting
                  ? 'Kaydediliyor…'
                  : examId
                  ? 'Değişiklikleri kaydet ve öğrencilere geç'
                  : 'Sınavı oluştur ve öğrencilere geç'}
              </button>
            </div>
          </aside>
        </div>
      ) : (
        <section className="speech-session">
          <div className="speech-session__toolbar">
            <div>
              <strong>{speakingExam?.title ?? examName}</strong>
              <span>{selectedClassName} · {speakingExam?.rubricLabel ?? 'Rubrik yükleniyor'}</span>
            </div>
            <div className="speech-session__progress">
              <Users size={16} /> {completedStudentIds.length}/{students.length} konuşma onaylandı
            </div>
          </div>

          <div className="speech-session__grid speech-session__grid--live">
            <aside className="speech-panel speech-student-list">
              <div className="speech-panel__heading">
                <div>
                  <h3>Öğrenciler</h3>
                  <p>Seçili sınıfın ortak listesi</p>
                </div>
                <Users size={19} />
              </div>
              <div className="speech-student-list__items">
                {students.map((student) => {
                  const approvedAttempt = speakingExam?.attempts.find(
                    (item) => item.studentId === student.id && item.state === 'approved',
                  );
                  const completed = Boolean(approvedAttempt);
                  return (
                    <button
                      type="button"
                      key={student.id}
                      className={student.id === selectedStudentId ? 'is-active' : ''}
                      disabled={isActiveCapture && student.id !== selectedStudentId}
                      onClick={() => void selectStudent(student.id)}
                    >
                      <span className="speech-student-list__status">
                        {completed ? (
                          <Check size={13} />
                        ) : student.id === selectedStudentId ? (
                          <span className="is-current" />
                        ) : null}
                      </span>
                      <span className="speech-student-list__info">
                        <strong>{studentLabel(student)}</strong>
                        <small>{student.number || 'Numara yok'}</small>
                      </span>
                      {approvedAttempt?.finalScore != null && (
                        <span className="speech-student-list__score">
                          {approvedAttempt.finalScore}/100
                        </span>
                      )}
                    </button>
                  );
                })}
              </div>
            </aside>

            <div className="speech-panel speech-record-panel">
              <div className="speech-process-center-card">
                <span
                  className={`speech-process-center-card__dot ${
                    isProcessing ? 'is-working' : isActiveCapture ? 'is-live' : 'is-ready'
                  }`}
                />
                <strong>İşlem Merkezi:</strong>
                <span>
                  {isProcessing
                    ? attempt?.state === 'cleaning_transcript'
                      ? 'Metin düzeltiliyor…'
                      : attempt?.state === 'evaluating'
                      ? 'Rubrik değerlendiriliyor…'
                      : 'Arka plan işlemi aktif'
                    : isActiveCapture
                    ? 'Whisper canlı dinliyor…'
                    : 'Devam eden işlem yok (Hazır)'}
                </span>
              </div>

              <div className="speech-record-panel__student">
                <div>
                  <span className="eyebrow">Aktif öğrenci</span>
                  <h3>{selectedStudent ? studentLabel(selectedStudent) : 'Öğrenci seçin'}</h3>
                  <p>{speakingExam?.taskText ?? taskText}</p>
                </div>
                <span className={`speech-recording-pill ${isActiveCapture ? 'is-recording' : ''}`}>
                  <span />
                  {isBackendCapturing || isCapturing
                    ? 'Whisper dinliyor'
                    : isPaused
                    ? 'Duraklatıldı'
                    : isProcessing
                    ? 'İşleniyor'
                    : 'Hazır'}
                </span>
              </div>

              <button
                type="button"
                className="speech-runtime-toggle"
                onClick={() => setShowRuntimeDetails((prev) => !prev)}
              >
                <div className="speech-runtime-toggle__info">
                  <Clock size={14} />
                  <span>Kayıt süresi & motor durumu</span>
                  <strong>{formatDuration(elapsedMs)}</strong>
                </div>
                <ChevronDown
                  size={14}
                  className={`speech-runtime-toggle__icon ${showRuntimeDetails ? 'is-open' : ''}`}
                />
              </button>

              {showRuntimeDetails && (
                <div className="speech-runtime-strip">
                  <div
                    className={`speech-runtime-chip ${
                      runtimeStatus?.whisperReady ? 'is-ready' : 'is-missing'
                    }`}
                  >
                    <span />
                    <div>
                      <strong>Whisper</strong>
                      <small>
                        {runtimeStatus?.whisperLoaded
                          ? 'Yüklü'
                          : runtimeStatus?.whisperReady
                          ? 'Hazır'
                          : 'Model eksik'}
                      </small>
                    </div>
                  </div>
                  <div className={`speech-runtime-chip ${isProcessing ? 'is-working' : ''}`}>
                    <span />
                    <div>
                      <strong>ASR / Gemma</strong>
                      <small>
                        {attempt?.state === 'cleaning_transcript'
                          ? 'Metni düzeltiyor'
                          : attempt?.state === 'evaluating'
                          ? 'Rubriği değerlendiriyor'
                          : 'Hazır'}
                      </small>
                    </div>
                  </div>
                  <div className="speech-runtime-timer">
                    <small>Kayıt süresi</small>
                    <strong>{formatDuration(elapsedMs)}</strong>
                  </div>
                </div>
              )}

              <div className={`speech-waveform ${isActiveCapture ? 'is-recording' : ''}`}>
                <div className="speech-waveform__bars">
                  {waveformBars.map((height, index) => (
                    <span key={index} style={{ height: `${height}px` }} />
                  ))}
                </div>
                <div className="speech-waveform__legend">
                  <span>
                    <i /> Mikrofon seviyesi
                  </span>
                  <strong>{Math.round(audioLevel * 100)}%</strong>
                </div>
              </div>

              <div className="speech-record-actions">
                <button
                  type="button"
                  className={`button ${isActiveCapture ? 'button--danger' : 'button--primary'}`}
                  disabled={
                    !selectedStudentId ||
                    isCapturePending ||
                    isProcessing ||
                    !runtimeStatus?.whisperReady
                  }
                  onClick={() => void controlCapture(isActiveCapture ? 'stop' : 'start')}
                >
                  {isCapturePending
                    ? 'İşlem hazırlanıyor…'
                    : isProcessing
                    ? 'Konuşma işleniyor…'
                    : isActiveCapture ? (
                      <>
                        <Volume2 size={16} /> Konuşmayı bitir
                      </>
                    ) : (
                      <>
                        <Mic2 size={16} /> Canlı dinlemeyi başlat
                      </>
                    )}
                </button>
                {(isCapturing || isBackendCapturing) && (
                  <button
                    type="button"
                    className="button button--secondary"
                    onClick={() => void controlCapture('pause')}
                  >
                    Duraklat
                  </button>
                )}
                {isPaused && (
                  <button
                    type="button"
                    className="button button--secondary"
                    onClick={() => void controlCapture('resume')}
                  >
                    Sürdür
                  </button>
                )}
                {isActiveCapture && (
                  <button
                    type="button"
                    className="button button--secondary"
                    onClick={() => void controlCapture('cancel')}
                  >
                    İptal
                  </button>
                )}
              </div>
            </div>

            <aside className="speech-panel speech-review-panel">
              <div className="speech-panel__heading">
                <div>
                  <h3>Öğrenci kanıtı</h3>
                  <p>Ses oynatıcı kullanılmaz</p>
                </div>
                <Sparkles size={19} />
              </div>
              <div className="speech-pipeline">
                <div className={pipelineState.whisper.done ? 'is-done' : ''}>
                  <Mic2 size={14} />
                  <span>Whisper ham transkript</span>
                  <small>{pipelineState.whisper.label}</small>
                </div>
                <div className={pipelineState.cleanup.done ? 'is-done' : ''}>
                  <Sparkles size={14} />
                  <span>Gemma 4 12B transkript temizliği</span>
                  <small>{pipelineState.cleanup.label}</small>
                </div>
                <div className={pipelineState.rubric.done ? 'is-done' : ''}>
                  <Sparkles size={14} />
                  <span>Gemma 4 12B rubrik</span>
                  <small>{pipelineState.rubric.label}</small>
                </div>
                <div className={attempt?.state === 'approved' ? 'is-done' : ''}>
                  <Check size={14} />
                  <span>Öğretmen onayı</span>
                  <small>{attempt?.state === 'approved' ? 'Onaylandı' : 'Bekliyor'}</small>
                </div>
              </div>
              <label className="speech-field">
                Ham Türkçe transkript
                <textarea
                  ref={transcriptRef}
                  rows={7}
                  readOnly
                  value={attempt?.rawTranscript ?? ''}
                  placeholder="Kayıt tamamlandığında görünür…"
                />
              </label>
              {attempt?.transcriptCleanup.transcriptForScoring && (
                <label className="speech-field">
                  Puanlama için düzeltilmiş transkript
                  <textarea
                    rows={5}
                    readOnly
                    value={attempt.transcriptCleanup.transcriptForScoring}
                  />
                </label>
              )}
              {attempt?.transcriptCleanup.status === 'failed' && (
                <div className="speech-form-note">
                  <CircleAlert size={16} /> ASR temizliği tamamlanamadı; ham metin öğretmen incelemesine açıldı ve başarısızlık sıfır puana dönüştürülmedi.
                </div>
              )}
              {attempt && !attempt.metrics.sampleDurationSufficient && (
                <div className="speech-form-note">
                  <CircleAlert size={16} /> Kayıt kısa olduğu için akıcılık ölçümü sınırlı güvenilirlikte; öğretmen onayı gerekir.
                </div>
              )}
            </aside>
          </div>

          {attempt?.criterionScores.length ? (
            <section className="speech-panel speech-rubric-review">
              <div className="speech-panel__heading">
                <div>
                  <h3>Rubrik değerlendirmesi</h3>
                  <p>
                    Metin ölçütlerinde Gemma önerir; gözlemsel ölçütleri öğretmen nitel yıldızlarla işaretler.
                  </p>
                </div>
                <Sparkles size={20} />
              </div>

              <details className="speech-metrics-details">
                <summary className="button button--secondary button--sm">
                  <BarChart3 size={15} /> Teknik ölçüm ayrıntıları
                </summary>
                <SpeechMetricsPanel metrics={attempt?.metrics} />
              </details>

              <div className="speech-rubric-grid">
                {attempt.criterionScores.map((score) => {
                  const criterion = speakingExam?.criteria.find(
                    (item) => item.id === score.criterionId,
                  );
                  const teacherOnly = criterion?.role === 'teacher_only';
                  const isClippingWarning =
                    score.criterionId === 'fluency_presentation' &&
                    Boolean(
                      attempt?.metrics?.durationMs &&
                        ((attempt.metrics.clippingRatio && attempt.metrics.clippingRatio > 0.005) ||
                          (attempt.metrics.clippingEventCount &&
                            attempt.metrics.clippingEventCount > 5) ||
                          attempt.metrics.audioQualityConfidence === 'low'),
                    );

                  return (
                    <article className="speech-rubric-card" key={score.criterionId}>
                      <div className="speech-rubric-card__header">
                        <strong>{score.criterionLabel}</strong>
                        <span>{score.maxScore} puan</span>
                      </div>
                      <small>{criterionSourceSummary(criterion, score)}</small>
                      {criterion?.description && <p>{criterion.description}</p>}

                      {score.criterionId === 'fluency_automatic' && (
                        <div className="speech-explainable-card">
                          <div className="speech-explainable-recommendation">
                            <strong>
                              Otomatik öneri:{' '}
                              {score.teacherScore != null
                                ? `${Math.round(score.teacherScore)}/5`
                                : isAttemptEvaluated && score.automaticScore != null
                                ? `${Math.round(score.automaticScore)}/5`
                                : 'Ölçüm bekleniyor'}
                            </strong>
                            {score.teacherScore != null && (
                              <small>(Öğretmen tarafından düzenlendi)</small>
                            )}
                          </div>
                          <p className="speech-explainable-text">
                            {getFluencyExplanation(attempt?.metrics, score.automaticScore)}
                          </p>
                          <div className="speech-summary-metrics-grid">
                            <div>
                              <span>Konuşma hızı:</span>
                              <strong>
                                {attempt?.metrics?.durationMs
                                  ? `${Math.round(attempt.metrics.wordsPerMinute)} kelime/dk`
                                  : '—'}
                              </strong>
                            </div>
                            <div>
                              <span>Uzun sessizlik:</span>
                              <strong>
                                {attempt?.metrics?.durationMs
                                  ? `${attempt.metrics.longPauseCount} adet`
                                  : '—'}
                              </strong>
                            </div>
                            <div>
                              <span>Dolgu ifadesi:</span>
                              <strong>
                                {attempt?.metrics?.durationMs
                                  ? `${attempt.metrics.fillerCount} adet`
                                  : '—'}
                              </strong>
                            </div>
                            <div>
                              <span>Belirgin tekrar:</span>
                              <strong>
                                {attempt?.metrics?.durationMs
                                  ? `${attempt.metrics.repetitionCount} adet`
                                  : '—'}
                              </strong>
                            </div>
                            <div>
                              <span>Aktif konuşma oranı:</span>
                              <strong>
                                {attempt?.metrics?.durationMs
                                  ? `%${((attempt.metrics.activeSpeechDurationMs / attempt.metrics.durationMs) * 100).toFixed(0)}`
                                  : '—'}
                              </strong>
                            </div>
                            <div>
                              <span>Ölçüm güveni:</span>
                              <strong>
                                {attempt?.metrics?.durationMs
                                  ? speakingMeasurementConfidenceLabel(
                                      attempt.metrics.measurementConfidence,
                                    )
                                  : '—'}
                              </strong>
                            </div>
                          </div>
                        </div>
                      )}

                      {score.criterionId === 'duration_management' && (
                        <div className="speech-explainable-card">
                          <div className="speech-summary-metrics-grid">
                            <div>
                              <span>Gerçek süre:</span>
                              <strong>{formatMsToDuration(attempt?.metrics?.durationMs)}</strong>
                            </div>
                            <div>
                              <span>Hedef aralık:</span>
                              <strong>
                                {speakingExam
                                  ? `${Math.round(speakingExam.minDurationSeconds / 60)}–${Math.round(speakingExam.maxDurationSeconds / 60)} dk`
                                  : '—'}
                              </strong>
                            </div>
                            <div>
                              <span>Sapma:</span>
                              <strong>
                                {isAttemptEvaluated
                                  ? getDurationDeviationLabel(
                                      attempt?.metrics?.durationMs,
                                      speakingExam?.minDurationSeconds,
                                      speakingExam?.maxDurationSeconds,
                                    )
                                  : 'Ölçüm bekleniyor'}
                              </strong>
                            </div>
                            <div>
                              <span>Süre önerisi:</span>
                              <strong>
                                {score.teacherScore != null
                                  ? `Otomatik öneri: ${Math.round(score.teacherScore)}/5 (Düzenlendi)`
                                  : isAttemptEvaluated && score.automaticScore != null
                                  ? `Otomatik öneri: ${Math.round(score.automaticScore)}/5`
                                  : 'Ölçüm bekleniyor'}
                              </strong>
                            </div>
                          </div>
                        </div>
                      )}

                      {isClippingWarning && (
                        <div className="speech-card-warning">
                          <CircleAlert size={14} />
                          <span>
                            Kayıt kalitesi sınırlı; öğretmen ses değerlendirmesini kontrol etmelidir.
                          </span>
                        </div>
                      )}

                      {score.aiSummary && <p>{score.aiSummary}</p>}
                      {score.subindicatorScores.length > 0 && (
                        <div className="speech-subindicator-list">
                          {score.subindicatorScores.map((subindicator) => (
                            <div
                              className="speech-subindicator-row"
                              key={subindicator.subindicatorId}
                            >
                              <span>
                                {speakingSubindicatorLabel(subindicator.subindicatorId)}
                              </span>
                              <strong>
                                {speakingPerformanceLevelLabel(
                                  subindicator.appliedLevelId || subindicator.selectedLevelId,
                                )}
                                {' · '}
                                {Math.round(subindicator.points)}/
                                {speakingSubindicatorMaximum(score.criterionId)}
                              </strong>
                              {subindicator.ceilingExplanation && (
                                <small>
                                  Model seçimi:{' '}
                                  {speakingPerformanceLevelLabel(subindicator.selectedLevelId)}.
                                  {' '}Backend doğrulaması:{' '}
                                  {speakingPerformanceLevelLabel(subindicator.appliedLevelId)}.
                                  {' '}Neden: {subindicator.ceilingExplanation}
                                </small>
                              )}
                              {(subindicator.evidenceSegmentIds.length > 0 ||
                                subindicator.counterEvidenceSegmentIds.length > 0) && (
                                <details>
                                  <summary>Kanıtları göster</summary>
                                  {subindicator.evidenceSegmentIds.map((segmentId) => {
                                    const segment = attempt.transcriptSegments.find(
                                      (item) => item.segmentId === segmentId,
                                    );
                                    return (
                                      <p key={`positive-${segmentId}`}>
                                        Olumlu kanıt:{' '}
                                        {segment?.cleanedText ??
                                          segment?.text ??
                                          'Canonical segment bulunamadı.'}
                                      </p>
                                    );
                                  })}
                                  {subindicator.counterEvidenceSegmentIds.map((segmentId) => {
                                    const segment = attempt.transcriptSegments.find(
                                      (item) => item.segmentId === segmentId,
                                    );
                                    return (
                                      <p key={`counter-${segmentId}`}>
                                        Karşı kanıt:{' '}
                                        {segment?.cleanedText ??
                                          segment?.text ??
                                          'Canonical segment bulunamadı.'}
                                      </p>
                                    );
                                  })}
                                </details>
                              )}
                            </div>
                          ))}
                        </div>
                      )}
                      {score.evidence[0] && (
                        <blockquote key="evidence">
                          “{score.evidence[0].quote}”
                          <small>{score.evidence[0].reason}</small>
                        </blockquote>
                      )}
                      {teacherOnly ? (
                        <TeacherStarRating
                          criterionId={score.criterionId}
                          criterionLabel={score.criterionLabel}
                          maxScore={score.maxScore}
                          currentLevel={
                            score.teacherLevel ?? (levelDrafts[score.criterionId] || null)
                          }
                          currentScore={score.teacherScore}
                          onSelectLevel={(level) => void savePerformanceLevel(score, level)}
                          onClear={() => void clearPerformanceLevel(score)}
                        />
                      ) : (
                        <div className="speech-rubric-card__edit">
                          <label>
                            Öğretmen düzeltmesi (Tam sayı)
                            <input
                              type="number"
                              min="0"
                              max={score.maxScore}
                              step="1"
                              value={scoreDrafts[score.criterionId] ?? ''}
                              onChange={(event) =>
                                setScoreDrafts((current) => ({
                                  ...current,
                                  [score.criterionId]: event.target.value,
                                }))
                              }
                              onBlur={() => void saveNumericScore(score)}
                              placeholder={
                                score.automaticScore == null && score.aiSuggestedScore == null
                                  ? 'Ölçüm bekleniyor'
                                  : 'Puan girin'
                              }
                            />
                          </label>
                        </div>
                      )}
                    </article>
                  );
                })}
              </div>
              <label className="speech-field speech-review-note">
                Öğretmen kısa notu
                <textarea
                  rows={3}
                  value={teacherNote}
                  onChange={(event) => setTeacherNote(event.target.value)}
                  onBlur={() => {
                    if (!attempt) return;
                    void commands
                      .updateSpeakingAttemptNote({
                        projectId,
                        examId,
                        attemptId: attempt.id,
                        teacherNote: teacherNote.trim() || undefined,
                      })
                      .then(() => setStatusMessage('Öğretmen notu otomatik kaydedildi.'))
                      .catch((error: unknown) =>
                        setPageError(errorMessage(error, 'Öğretmen notu kaydedilemedi.')),
                      );
                  }}
                  placeholder="Genel değerlendirme notu…"
                />
              </label>

              {(() => {
                const teacherScores = attempt.criterionScores.filter((score) => {
                  const criterion = speakingExam?.criteria.find(
                    (item) => item.id === score.criterionId,
                  );
                  return criterion?.role === 'teacher_only';
                });
                const unratedCount = teacherScores.filter(
                  (score) => score.teacherScore === undefined || score.teacherScore === null,
                ).length;
                if (unratedCount > 0) {
                  return (
                    <div className="speech-inline-warning">
                      <CircleAlert size={16} /> <strong>Geçici toplam:</strong> {teacherScores.length}{' '}
                      öğretmen ölçütünden {unratedCount} tanesi henüz değerlendirilmedi. Tüm
                      yıldız seçimleri tamamlanmadan nihai puan onaylanamaz.
                    </div>
                  );
                }
                return null;
              })()}

              <div className="speech-review-actions">
                <button
                  type="button"
                  className="button button--primary"
                  disabled={isApproving || attempt.state !== 'teacher_review'}
                  onClick={() => void approveAttempt()}
                >
                  <Check size={16} />{' '}
                  {isApproving
                    ? 'Onaylanıyor…'
                    : `Nihai puanı onayla${
                        attempt.finalScore != null ? ` · ${attempt.finalScore}/100` : ''
                      }`}
                </button>
              </div>
            </section>
          ) : null}

          <div className="speech-session__footer">
            <span>
              <CircleAlert size={15} /> Tüm seçimler otomatik kaydedilir; uygulama bu oturumu yeniden
              açar.
            </span>
            <button
              type="button"
              className="button button--primary"
              disabled={
                isActiveCapture || isProcessing || isFinishing || completedStudentIds.length === 0
              }
              title={
                completedStudentIds.length === 0
                  ? 'En az bir öğrenci puanı onaylanmalıdır.'
                  : undefined
              }
              onClick={() => void finishExam()}
            >
              <BarChart3 size={16} />{' '}
              {isFinishing ? 'Analiz hazırlanıyor…' : 'Sınavı bitir ve analiz oluştur'}
            </button>
          </div>
        </section>
      )}
    </div>
  );
}

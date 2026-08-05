import { useEffect, useMemo, useState } from 'react';
import { createPortal } from 'react-dom';
import { Link, useParams, useSearchParams } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  AlertTriangle,
  BadgeCheck,
  CheckCircle2,
  CircleAlert,
  FileSpreadsheet,
  FileText,
  Inbox,
  ListChecks,
  MinusCircle,
  Printer,
  Save,
  Send,
  UserX,
  Users,
} from 'lucide-react';
import { commands } from '../api/commands';
import type { AppError } from '../api/errors';
import type {
  CriterionRating,
  PerformanceAssessment,
  PerformanceAssessmentStatus,
  PerformanceReport,
  PerformanceRubric,
  Student,
} from '../api/types';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { LoadingButton } from '../components/common/LoadingButton';
import { ProjectContextState } from '../components/common/ProjectContextState';
import { useProjectContext } from '../state/useProjectContext';
import { formatDateTime } from '../utils/formatting';
import {
  buildPerformanceCsv,
  downloadTextFile,
  performanceReportCsvFileName,
  performanceReportRowTotal,
  performanceReportStatusLabel,
} from './performanceReportUi';
import {
  latestPublishedPerformanceRubric,
  performanceAssessmentStatusLabels,
  performanceMaxPoints,
  performanceMissingCriteria,
  performanceProvisionalTotal,
  performanceSkillAreaLabels,
  performanceWorkModeLabels,
} from './performanceOrganizationUi';

function studentLabel(student: Student): string {
  return student.displayName?.trim() || student.number?.trim() || 'İsimsiz öğrenci';
}

function studentNumber(student: Student): string {
  return student.number?.trim() || '';
}

function statusLabel(status: PerformanceAssessmentStatus | undefined): string {
  return status ? performanceAssessmentStatusLabels[status] : 'Başlanmadı';
}

export function PerformanceScoringPage({
  activityId: propActivityId,
}: {
  activityId?: string;
}) {
  const { projectId: contextProjectId, isResolving } = useProjectContext();
  const params = useParams<{ assessmentActivityId?: string; activityId?: string }>();
  const [searchParams] = useSearchParams();
  const queryClient = useQueryClient();

  const projectId = contextProjectId;
  const activityId = propActivityId || params.assessmentActivityId || params.activityId || '';
  const requestedClassApplicationId = searchParams.get('classApplicationId') || '';

  const [selectedStudentId, setSelectedStudentId] = useState('');
  const [ratingDrafts, setRatingDrafts] = useState<Record<string, string>>({});
  const [feedback, setFeedback] = useState('');
  const [error, setError] = useState<AppError | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);

  const activityQuery = useQuery({
    queryKey: ['assessment-activity', projectId, activityId],
    queryFn: () => commands.getPerformanceTask({ projectId, activityId }),
    enabled: !!projectId && !!activityId,
  });
  const activity = activityQuery.data;

  const applications = useMemo(
    () => (activity?.classApplications ?? []).filter((application) => application.status !== 'archived'),
    [activity?.classApplications],
  );
  const classApplicationId = applications.some(
    (application) => application.id === requestedClassApplicationId,
  )
    ? requestedClassApplicationId
    : (applications[0]?.id ?? '');

  const studentsQuery = useQuery({
    queryKey: ['class-application-students', projectId, activityId, classApplicationId],
    queryFn: () =>
      commands.getClassApplicationStudents({
        projectId,
        activityId,
        applicationId: classApplicationId,
      }),
    enabled: !!projectId && !!activityId && !!classApplicationId,
  });
  const students = studentsQuery.data ?? [];

  const assessmentsQuery = useQuery({
    queryKey: ['performance-assessments', projectId, activityId, classApplicationId],
    queryFn: () =>
      commands.listPerformanceAssessments({
        projectId,
        activityId,
        applicationId: classApplicationId,
      }),
    enabled: !!projectId && !!activityId && !!classApplicationId,
  });
  const assessments = assessmentsQuery.data ?? [];

  const rubric = useMemo(
    () =>
      latestPublishedPerformanceRubric(activity?.performanceDetails?.rubricVersions),
    [activity?.performanceDetails?.rubricVersions],
  );
  const rubricCriteria = rubric?.criteria ?? [];
  const rubricLevels = rubric?.levels ?? [];
  const maxPoints = performanceMaxPoints(rubric);

  const assessmentByStudent = useMemo(
    () => new Map(assessments.map((assessment) => [assessment.studentId, assessment])),
    [assessments],
  );

  const selectedStudent = students.find((student) => student.id === selectedStudentId);
  const selectedAssessment = selectedStudentId
    ? assessmentByStudent.get(selectedStudentId)
    : undefined;
  const selectedStatus: PerformanceAssessmentStatus | undefined = selectedAssessment?.status;
  const isApproved = selectedStatus === 'approved';
  const isNonRatedStatus =
    selectedStatus === 'missing' || selectedStatus === 'not_performed';

  useEffect(() => {
    if (!selectedStudentId) {
      setRatingDrafts({});
      setFeedback('');
      return;
    }
    const assessment = assessmentByStudent.get(selectedStudentId);
    setRatingDrafts(
      Object.fromEntries(
        (assessment?.ratings ?? []).map((rating) => [rating.criterionId, rating.levelId]),
      ),
    );
    setFeedback(assessment?.feedback ?? '');
  }, [selectedStudentId, assessmentByStudent]);

  useEffect(() => {
    if (students.length === 0) {
      setSelectedStudentId('');
      return;
    }
    if (!students.some((student) => student.id === selectedStudentId)) {
      const [firstStudent] = students;
      if (firstStudent) setSelectedStudentId(firstStudent.id);
    }
  }, [selectedStudentId, students]);

  const refreshAssessments = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ['performance-assessments', projectId, activityId] }),
      queryClient.invalidateQueries({ queryKey: ['assessment-activity', projectId, activityId] }),
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] }),
    ]);
  };

  const selectedRatings: CriterionRating[] = [];
  for (const criterion of rubricCriteria) {
    const levelId = ratingDrafts[criterion.id];
    if (levelId) {
      selectedRatings.push({ criterionId: criterion.id, levelId, note: null });
    }
  }

  const provisionalTotal = rubric ? performanceProvisionalTotal(rubric, selectedRatings) : 0;
  const missingCriteria = rubric ? performanceMissingCriteria(rubric, selectedRatings) : [];

  const saveMutation = useMutation({
    mutationFn: () =>
      commands.savePerformanceAssessment({
        projectId,
        activityId,
        applicationId: classApplicationId,
        studentId: selectedStudentId,
        assessmentId: selectedAssessment?.id,
        ratings: selectedRatings,
        feedback: feedback.trim() || undefined,
      }),
    onMutate: () => {
      setError(null);
      setSuccessMessage(null);
    },
    onSuccess: async (saved) => {
      setSuccessMessage(
        saved.status === 'in_progress'
          ? 'Değerlendirme taslağı kaydedildi; geçici toplam servis tarafından hesaplandı.'
          : 'Değerlendirme kaydedildi.',
      );
      await refreshAssessments();
    },
    onError: (caught: AppError) => setError(caught),
  });

  const approveMutation = useMutation({
    mutationFn: () =>
      commands.approvePerformanceAssessment({
        projectId,
        activityId,
        applicationId: classApplicationId,
        assessmentId: selectedAssessment?.id ?? '',
      }),
    onMutate: () => {
      setError(null);
      setSuccessMessage(null);
    },
    onSuccess: async () => {
      setSuccessMessage('Değerlendirme onaylandı. Onay sonrası düzenleme reddedilir.');
      await refreshAssessments();
    },
    onError: (caught: AppError) => setError(caught),
  });

  const statusMutation = useMutation({
    mutationFn: (status: 'missing' | 'not_performed') =>
      commands.setPerformanceAssessmentStatus({
        projectId,
        activityId,
        applicationId: classApplicationId,
        studentId: selectedStudentId,
        assessmentId: selectedAssessment?.id,
        status,
      }),
    onMutate: () => {
      setError(null);
      setSuccessMessage(null);
    },
    onSuccess: async () => {
      setSuccessMessage('Öğrenci durumu güncellendi. Bu kayıt toplam hesaplarına girmez.');
      await refreshAssessments();
    },
    onError: (caught: AppError) => setError(caught),
  });

  const revertStatusMutation = useMutation({
    mutationFn: () =>
      commands.savePerformanceAssessment({
        projectId,
        activityId,
        applicationId: classApplicationId,
        studentId: selectedStudentId,
        assessmentId: selectedAssessment?.id,
        ratings: selectedRatings,
        feedback: feedback.trim() || undefined,
      }),
    onMutate: () => {
      setError(null);
      setSuccessMessage(null);
    },
    onSuccess: async () => {
      setSuccessMessage('Öğrenci değerlendirmeye alındı; düzey seçimlerine geçebilirsiniz.');
      await refreshAssessments();
    },
    onError: (caught: AppError) => setError(caught),
  });

  const canApprove =
    Boolean(rubric) &&
    !isApproved &&
    !isNonRatedStatus &&
    missingCriteria.length === 0 &&
    Boolean(selectedAssessment) &&
    !approveMutation.isPending;

  const assessedStudentCount = assessments.filter(
    (assessment) => assessment.status !== 'missing' && assessment.status !== 'not_performed',
  ).length;
  const approvedStudentCount = assessments.filter(
    (assessment) => assessment.status === 'approved',
  ).length;
  const notAssessedCount = students.length - assessedStudentCount;

  if (isResolving) {
    return <ProjectContextState pageLabel="Performans Değerlendirme" loading projectPath="" />;
  }
  if (!projectId || !activityId) {
    return <ProjectContextState pageLabel="Performans Değerlendirme" projectPath="" />;
  }

  if (activityQuery.isLoading || studentsQuery.isLoading || assessmentsQuery.isLoading) {
    return <div style={{ padding: '2rem', color: '#64748b' }}>Değerlendirme ekranı yükleniyor…</div>;
  }
  if (!activity) {
    return (
      <div style={{ padding: '2rem', color: '#ef4444' }}>
        Performans görevi bulunamadı.
      </div>
    );
  }

  const details = activity.performanceDetails;

  return (
    <div className="performance-scoring-page">
      <div className="performance-scoring-page__header">
        <div>
          <p className="eyebrow">Performans değerlendirme</p>
          <h2>{activity.title || `${activity.term}. Dönem ${activity.sequenceNumber}. Performans`}</h2>
          <p>
            {activity.courseName} · {activity.gradeLevel}. sınıf · {activity.term}. dönem
            {details && (
              <>
                {' '}
                · {performanceSkillAreaLabels[details.skillArea]} ·{' '}
                {performanceWorkModeLabels[details.workMode]}
              </>
            )}
          </p>
        </div>
        <div className="performance-scoring-page__summary">
          <span className="performance-summary-chip">
            <CheckCircle2 size={15} /> {approvedStudentCount} onaylı
          </span>
          <span className="performance-summary-chip">
            <ListChecks size={15} /> {assessedStudentCount}/{students.length} değerlendirildi
          </span>
        </div>
      </div>

      {(error || activityQuery.error || studentsQuery.error || assessmentsQuery.error) && (
        <ErrorBanner
          error={
            (error || activityQuery.error || studentsQuery.error || assessmentsQuery.error) as AppError
          }
        />
      )}
      {successMessage && (
        <div className="classes-notice" role="status">
          <CheckCircle2 size={17} />
          {successMessage}
        </div>
      )}

      {!rubric && (
        <div className="performance-empty-state">
          <AlertTriangle size={24} />
          <strong>Rubrik yayınlanmadı</strong>
          <span>Değerlendirme başlatmak için önce rubriği yayınlayın.</span>
          <Link
            className="button button--primary"
            to={`/project/${encodeURIComponent(projectId)}/activities/${encodeURIComponent(activityId)}/task`}
          >
            Görev ve Rubrik sayfasına git
          </Link>
        </div>
      )}

      {rubric && applications.length === 0 && (
        <div className="performance-empty-state">
          <Users size={24} />
          <strong>Sınıf uygulaması yok</strong>
          <span>Bu göreve bağlı aktif sınıf bulunmuyor.</span>
        </div>
      )}

      {rubric && applications.length > 0 && (
        <>
          {notAssessedCount > 0 && (
            <div className="performance-warnings performance-warnings--notice" role="status">
              <Inbox size={16} />
              <span>
                {notAssessedCount} öğrenci için değerlendirme başlatılmadı. Listeden bir öğrenci
                seçerek düzey işaretlemeye başlayın.
              </span>
            </div>
          )}

          <div className="performance-scoring-grid">
            <aside className="performance-panel performance-student-list">
              <div className="performance-panel__heading">
                <div>
                  <h3>Öğrenciler</h3>
                  <p>Seçili sınıfın ortak listesi · {classApplicationId ? students.length : '—'} öğrenci</p>
                </div>
                <Users size={18} />
              </div>
              <div className="performance-student-list__items">
                {students.map((student) => {
                  const assessment = assessmentByStudent.get(student.id);
                  const status = assessment?.status;
                  const total = assessment?.provisionalTotal;
                  const isSelected = student.id === selectedStudentId;
                  return (
                    <button
                      type="button"
                      data-project-write="false"
                      key={student.id}
                      className={isSelected ? 'is-active' : ''}
                      onClick={() => setSelectedStudentId(student.id)}
                    >
                      <span className={`performance-student-badge is-${status ?? 'none'}`}>
                        {status === 'approved' ? (
                          <BadgeCheck size={14} />
                        ) : status === 'missing' ? (
                          <MinusCircle size={14} />
                        ) : status === 'not_performed' ? (
                          <UserX size={14} />
                        ) : status === 'in_progress' ? (
                          <ListChecks size={14} />
                        ) : (
                          <span className="is-dot" />
                        )}
                      </span>
                      <span className="performance-student-list__info">
                        <strong>{studentLabel(student)}</strong>
                        <small>{studentNumber(student) || 'Numara yok'}</small>
                      </span>
                      <span className="performance-student-list__score">
                        {status === 'missing' || status === 'not_performed' ? (
                          <em>{statusLabel(status)}</em>
                        ) : status === 'approved' || status === 'in_progress' ? (
                          <>{total != null ? `${total}/${maxPoints}` : '—'}</>
                        ) : (
                          '—'
                        )}
                      </span>
                    </button>
                  );
                })}
              </div>
            </aside>

            <div className="performance-scoring-main">
              {!selectedStudent ? (
                <div className="performance-empty-state">
                  <Users size={24} />
                  <strong>Öğrenci seçin</strong>
                  <span>Sınıf listesinden bir öğrenci seçerek değerlendirmeye başlayın.</span>
                </div>
              ) : isApproved ? (
                <div className="performance-panel">
                  <div className="performance-panel__heading">
                    <div>
                      <h3>{studentLabel(selectedStudent)}</h3>
                      <p>Onaylanmış değerlendirme — düzenlenemez.</p>
                    </div>
                    <span className="performance-status-badge performance-status-badge--approved">
                      <BadgeCheck size={14} /> Onaylandı
                    </span>
                  </div>
                  <ApprovedAssessmentView
                    rubric={rubric}
                    assessment={selectedAssessment}
                    feedback={feedback}
                  />
                </div>
              ) : isNonRatedStatus ? (
                <div className="performance-panel">
                  <div className="performance-panel__heading">
                    <div>
                      <h3>{studentLabel(selectedStudent)}</h3>
                      <p>
                        Bu öğrenci {selectedStatus === 'missing' ? 'teslim etmedi' : 'performans gösterdiği gözlenmedi'}.
                        Sıfır puan yazılmaz; bu kayıt toplam hesaplarına girmez.
                      </p>
                    </div>
                    <span
                      className={`performance-status-badge performance-status-badge--${selectedStatus}`}
                    >
                      {selectedStatus === 'missing' ? <MinusCircle size={14} /> : <UserX size={14} />}{' '}
                      {statusLabel(selectedStatus)}
                    </span>
                  </div>
                  <div className="performance-non-rated-card">
                    <p>
                      {selectedStatus === 'missing'
                        ? 'Teslim edilmemiş (Missing) olarak işaretlendi. Bu durum sıfır puanla karıştırılmaz; raporda ayrı gösterilir.'
                        : 'Gösterilmedi (NotPerformed) olarak işaretlendi. Bu durum sıfır puanla karıştırılmaz; raporda ayrı gösterilir.'}
                    </p>
                    <LoadingButton
                      type="button"
                      className="button button--secondary"
                      loading={revertStatusMutation.isPending}
                      onClick={() => revertStatusMutation.mutate()}
                    >
                      <ListChecks size={15} /> Değerlendirmeye al
                    </LoadingButton>
                  </div>
                </div>
              ) : (
                <div className="performance-panel">
                  <div className="performance-panel__heading">
                    <div>
                      <h3>{studentLabel(selectedStudent)}</h3>
                      <p>
                        Her ölçüt için bir düzey seçin. Tek sayı girişi yok; puan rubrik düzeyinden gelir.
                      </p>
                    </div>
                    <span className="performance-status-badge performance-status-badge--in-progress">
                      <ListChecks size={14} /> {statusLabel(selectedStatus)}
                    </span>
                  </div>

                  {details?.taskInstruction && (
                    <div className="performance-task-instruction">
                      <FileText size={15} />
                      <span>{details.taskInstruction}</span>
                    </div>
                  )}

                  <div className="performance-rating-list">
                    {rubricCriteria.map((criterion) => {
                      const selectedLevelId = ratingDrafts[criterion.id];
                      const selectedLevel = rubricLevels.find(
                        (level) => level.id === selectedLevelId,
                      );
                      return (
                        <div key={criterion.id} className="performance-rating-criterion">
                          <div className="performance-rating-criterion__head">
                            <div>
                              <strong>{criterion.name || 'Ölçüt'}</strong>
                              <span>{criterion.description}</span>
                            </div>
                          </div>
                          <div className="performance-rating-levels" role="radiogroup" aria-label={criterion.name}>
                            {rubricLevels.map((level) => {
                              const isSelected = level.id === selectedLevelId;
                              return (
                                <button
                                  type="button"
                                  data-project-write="true"
                                  key={level.id}
                                  role="radio"
                                  aria-checked={isSelected}
                                  className={`performance-rating-level ${isSelected ? 'is-selected' : ''}`}
                                  onClick={() =>
                                    setRatingDrafts((current) => ({
                                      ...current,
                                      [criterion.id]: isSelected ? '' : level.id,
                                    }))
                                  }
                                >
                                  <span className="performance-rating-level__name">{level.name}</span>
                                  <span className="performance-rating-level__points">{level.points} puan</span>
                                </button>
                              );
                            })}
                          </div>
                          {selectedLevel && (
                            <div className="performance-rating-observation">
                              <strong>Seçilen düzey tanımı:</strong>
                              <span>
                                {criterion.levelDescriptions.find(
                                  (entry) => entry.levelId === selectedLevel.id,
                                )?.description || selectedLevel.description}
                              </span>
                            </div>
                          )}
                        </div>
                      );
                    })}
                  </div>

                  <div className="performance-rating-footer">
                    <div className="performance-total-card">
                      <div>
                        <span>Geçici toplam (yalnız seçili düzeyler)</span>
                        <strong>
                          {provisionalTotal}
                          <small> / {maxPoints} puan</small>
                        </strong>
                      </div>
                      <div className="performance-total-card__status">
                        {missingCriteria.length > 0 ? (
                          <span className="performance-missing-chip">
                            {rubricCriteria.length - missingCriteria.length}/{rubricCriteria.length} ölçüt seçildi ·{' '}
                            {missingCriteria.map((criterion) => criterion.name || 'Ölçüt').join(', ')} eksik
                          </span>
                        ) : (
                          <span className="performance-ready-chip">
                            <CheckCircle2 size={13} /> Tüm ölçütler seçildi
                          </span>
                        )}
                      </div>
                    </div>

                    <label className="performance-feedback">
                      <span>Geri bildirim (öğrenciye yansıtılır)</span>
                      <textarea
                        rows={3}
                        value={feedback}
                        placeholder="Öğrencinin performansına yönelik gözlemlerinizi yazın."
                        onChange={(event) => setFeedback(event.target.value)}
                      />
                    </label>

                    <div className="performance-panel__actions">
                      <LoadingButton
                        type="button"
                        className="button button--primary"
                        loading={saveMutation.isPending}
                        disabledReason={provisionalTotal === 0 && selectedRatings.length === 0 ? 'En az bir düzey seçin' : undefined}
                        onClick={() => saveMutation.mutate()}
                      >
                        <Save size={15} /> Taslağı kaydet
                      </LoadingButton>
                      <LoadingButton
                        type="button"
                        className="button button--primary button--approve"
                        loading={approveMutation.isPending}
                        disabledReason={
                          !canApprove
                            ? missingCriteria.length > 0
                              ? 'Tüm ölçütler seçilmeden onay verilemez.'
                              : !selectedAssessment
                                ? 'Önce taslağı kaydedin.'
                                : undefined
                            : undefined
                        }
                        onClick={() => approveMutation.mutate()}
                      >
                        <Send size={15} /> Onayla
                      </LoadingButton>
                    </div>

                    <div className="performance-non-rated-actions">
                      <span>Bu öğrenci değerlendirilemeyecekse:</span>
                      <LoadingButton
                        type="button"
                        className="button button--secondary"
                        loading={statusMutation.isPending}
                        onClick={() => statusMutation.mutate('missing')}
                      >
                        <MinusCircle size={14} /> Eksik (teslim etmedi)
                      </LoadingButton>
                      <LoadingButton
                        type="button"
                        className="button button--secondary"
                        loading={statusMutation.isPending}
                        onClick={() => statusMutation.mutate('not_performed')}
                      >
                        <UserX size={14} /> Gösterilmedi
                      </LoadingButton>
                    </div>
                  </div>
                </div>
              )}
            </div>
          </div>
        </>
      )}
    </div>
  );
}

function ApprovedAssessmentView({
  rubric,
  assessment,
  feedback,
}: {
  rubric: PerformanceRubric;
  assessment: PerformanceAssessment | undefined;
  feedback: string;
}) {
  const ratings = assessment?.ratings ?? [];
  const ratingByCriterion = useMemo(
    () => new Map(ratings.map((rating) => [rating.criterionId, rating])),
    [ratings],
  );
  return (
    <div className="performance-approved-view">
      <div className="performance-total-card">
        <div>
          <span>Onaylanan toplam</span>
          <strong>
            {assessment?.provisionalTotal ?? 0}
            <small> / {performanceMaxPoints(rubric)} puan</small>
          </strong>
        </div>
        <div className="performance-total-card__status">
          <span className="performance-ready-chip">
            <BadgeCheck size={13} /> Onaylandı
          </span>
        </div>
      </div>
      {assessment?.approvedAt && (
        <p className="performance-approved-view__meta">
          Onay tarihi: {formatDateTime(assessment.approvedAt)}
        </p>
      )}
      <div className="performance-approved-criteria">
        {rubric.criteria.map((criterion) => {
          const rating = ratingByCriterion.get(criterion.id);
          const level = rubric.levels.find((candidate) => candidate.id === rating?.levelId);
          return (
            <div key={criterion.id} className="performance-approved-criterion">
              <div>
                <strong>{criterion.name || 'Ölçüt'}</strong>
                <span>
                  {level
                    ? `${level.name} · ${level.points} puan`
                    : 'Değerlendirme yok'}
                </span>
              </div>
            </div>
          );
        })}
      </div>
      {(feedback?.trim() || assessment?.feedback) && (
        <div className="performance-feedback-view">
          <strong>Geri bildirim</strong>
          <p>{feedback?.trim() || assessment?.feedback}</p>
        </div>
      )}
    </div>
  );
}

export function PerformanceResultsView({
  projectId,
  activityId,
}: {
  projectId: string;
  activityId: string;
}) {
  const [searchParams] = useSearchParams();
  const requestedClassApplicationId = searchParams.get('classApplicationId') || '';
  const [printReport, setPrintReport] = useState<PerformanceReport | null>(null);

  const activityQuery = useQuery({
    queryKey: ['assessment-activity', projectId, activityId],
    queryFn: () => commands.getPerformanceTask({ projectId, activityId }),
    enabled: !!projectId && !!activityId,
  });
  const activity = activityQuery.data;
  const applications = useMemo(
    () => (activity?.classApplications ?? []).filter((application) => application.status !== 'archived'),
    [activity?.classApplications],
  );
  const classApplicationId = applications.some(
    (application) => application.id === requestedClassApplicationId,
  )
    ? requestedClassApplicationId
    : (applications[0]?.id ?? '');

  const studentsQuery = useQuery({
    queryKey: ['class-application-students', projectId, activityId, classApplicationId],
    queryFn: () =>
      commands.getClassApplicationStudents({
        projectId,
        activityId,
        applicationId: classApplicationId,
      }),
    enabled: !!projectId && !!activityId && !!classApplicationId,
  });
  const students = studentsQuery.data ?? [];
  const assessmentsQuery = useQuery({
    queryKey: ['performance-assessments', projectId, activityId, classApplicationId],
    queryFn: () =>
      commands.listPerformanceAssessments({
        projectId,
        activityId,
        applicationId: classApplicationId,
      }),
    enabled: !!projectId && !!activityId && !!classApplicationId,
  });
  const assessments = assessmentsQuery.data ?? [];
  const assessmentByStudent = useMemo(
    () => new Map(assessments.map((assessment) => [assessment.studentId, assessment])),
    [assessments],
  );
  const rubric = useMemo(
    () => latestPublishedPerformanceRubric(activity?.performanceDetails?.rubricVersions),
    [activity?.performanceDetails?.rubricVersions],
  );
  const maxPoints = performanceMaxPoints(rubric);

  const reportQuery = useQuery({
    queryKey: ['performance-report', projectId, activityId, classApplicationId],
    queryFn: () =>
      commands.getPerformanceReport({
        projectId,
        activityId,
        applicationId: classApplicationId,
      }),
    enabled: !!projectId && !!activityId && !!classApplicationId && !!rubric,
  });
  const report = reportQuery.data;
  const reportError = reportQuery.error as AppError | null;

  useEffect(() => {
    setPrintReport(null);
  }, [classApplicationId]);

  const handleExportCsv = () => {
    if (!report) return;
    downloadTextFile(
      performanceReportCsvFileName(report, report.className),
      buildPerformanceCsv(report),
      'text/csv;charset=utf-8',
    );
  };

  const handlePrintPdf = () => {
    if (!report) return;
    setPrintReport(report);
    requestAnimationFrame(() => requestAnimationFrame(() => window.print()));
  };

  const reportReady = Boolean(report) && !reportQuery.isLoading;

  return (
    <div className="performance-results-view">
      {printReport && <PerformanceReportPrintView report={printReport} />}
      <div className="performance-panel">
        <div className="performance-panel__heading">
          <div>
            <h3>Sonuçlar</h3>
            <p>Öğrenci bazlı durum ve geçici/onaylı toplamlar. Sınıf düzeyinde PDF/Excel raporu üretebilirsiniz.</p>
          </div>
          <CircleAlert size={18} />
        </div>

        {reportError && <ErrorBanner error={reportError} />}

        <div className="performance-results-actions">
          <LoadingButton
            type="button"
            className="button button--secondary"
            loading={reportQuery.isLoading}
            disabledReason={!reportReady ? 'Rapor verisi yüklenemedi.' : undefined}
            onClick={handleExportCsv}
          >
            <FileSpreadsheet size={15} /> Excel (CSV) raporu
          </LoadingButton>
          <LoadingButton
            type="button"
            className="button button--secondary"
            loading={reportQuery.isLoading}
            disabledReason={!reportReady ? 'Rapor verisi yüklenemedi.' : undefined}
            onClick={handlePrintPdf}
          >
            <Printer size={15} /> PDF raporu
          </LoadingButton>
        </div>

        {students.length === 0 ? (
          <p className="assessment-form-help">Bu sınıfta öğrenci bulunmuyor.</p>
        ) : (
          <table className="performance-results-table">
            <thead>
              <tr>
                <th>Öğrenci</th>
                <th>Durum</th>
                <th>Toplam</th>
                <th>Geri bildirim</th>
              </tr>
            </thead>
            <tbody>
              {students.map((student) => {
                const assessment = assessmentByStudent.get(student.id);
                const status = assessment?.status;
                return (
                  <tr key={student.id}>
                    <td>
                      <strong>{studentLabel(student)}</strong>
                      <small>{studentNumber(student) || 'Numara yok'}</small>
                    </td>
                    <td>
                      <span
                        className={`performance-status-badge performance-status-badge--${status ?? 'none'}`}
                      >
                        {statusLabel(status)}
                      </span>
                    </td>
                    <td>
                      {status === 'missing' || status === 'not_performed' ? (
                        <span className="performance-non-rated-cell">—</span>
                      ) : status ? (
                        `${assessment?.provisionalTotal ?? 0}/${maxPoints}`
                      ) : (
                        '—'
                      )}
                    </td>
                    <td className="performance-results-table__feedback">
                      {assessment?.feedback || '—'}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}

function PerformanceReportPrintView({ report }: { report: PerformanceReport }) {
  return createPortal(
    <div className="performance-report-print">
      <header className="performance-report-print__header">
        <h1>Performans Değerlendirme Sonuç Raporu</h1>
        <span>Rapor tarihi: {formatDateTime(report.generatedAt) ?? report.generatedAt}</span>
      </header>

      <section className="performance-report-print__meta">
        <div>
          <span>Görev</span>
          <strong>{report.taskTitle}</strong>
        </div>
        <div>
          <span>Ders</span>
          <strong>{report.courseName}</strong>
        </div>
        <div>
          <span>Sınıf</span>
          <strong>{report.className}</strong>
        </div>
        <div>
          <span>Dönem / Sıra</span>
          <strong>
            {report.gradeLevel}. sınıf · {report.term}. dönem · {report.sequenceNumber}. görev
          </strong>
        </div>
        {report.theme && (
          <div>
            <span>Tema</span>
            <strong>{report.theme}</strong>
          </div>
        )}
        {report.skillArea && (
          <div>
            <span>Beceri alanı</span>
            <strong>{performanceSkillAreaLabels[report.skillArea]}</strong>
          </div>
        )}
        {report.workMode && (
          <div>
            <span>Çalışma biçimi</span>
            <strong>{performanceWorkModeLabels[report.workMode]}</strong>
          </div>
        )}
        <div>
          <span>Rubrik</span>
          <strong>
            {report.rubricName} · sürüm {report.rubricVersion}
          </strong>
        </div>
        <div>
          <span>Öğretmen</span>
          <strong>{report.teacherId ? report.teacherId : 'Belirtilmedi'}</strong>
        </div>
      </section>

      <section className="performance-report-print__summary">
        <span>{report.summary.assessedCount} değerlendirildi</span>
        <span>{report.summary.approvedCount} onaylı</span>
        <span>{report.summary.missingCount} eksik</span>
        <span>{report.summary.notPerformedCount} gösterilmedi</span>
        <span>{report.summary.unratedCount} değerlendirilmedi</span>
      </section>

      <table className="performance-report-print__table">
        <thead>
          <tr>
            <th>No</th>
            <th>Öğrenci</th>
            <th>Durum</th>
            {report.criteria.map((criterion) => (
              <th key={criterion.id}>{criterion.name}</th>
            ))}
            <th>Toplam</th>
            <th>Geri bildirim</th>
          </tr>
        </thead>
        <tbody>
          {report.rows.map((row) => {
            const total = performanceReportRowTotal(row);
            const pointsByCriterion = new Map(
              row.criterionScores.map((score) => [score.criterionId, score]),
            );
            const nonRated = row.status === 'missing' || row.status === 'not_performed';
            return (
              <tr key={row.studentId}>
                <td>{row.studentNumber ?? '—'}</td>
                <td>{row.studentName}</td>
                <td>{performanceReportStatusLabel(row.status)}</td>
                {report.criteria.map((criterion) => {
                  const score = pointsByCriterion.get(criterion.id);
                  return (
                    <td key={criterion.id}>
                      {nonRated || !score?.levelName ? '—' : `${score.levelName} (${score.points} p)`}
                    </td>
                  );
                })}
                <td>{nonRated || total == null ? '—' : `${total}/${report.maxPoints}`}</td>
                <td>{row.feedback?.trim() || '—'}</td>
              </tr>
            );
          })}
        </tbody>
      </table>

      <p className="performance-report-print__legend">
        «Eksik (teslim edilmedi)» ve «Gösterilmedi» durumları sıfır puanla karıştırılmaz; raporda
        boş hücre ve etiketle ayrı gösterilir. Nihai öğretmen kararı baz alınmıştır.
      </p>
    </div>,
    document.body,
  );
}

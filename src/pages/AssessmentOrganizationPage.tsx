import { useCallback, useEffect, useMemo, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Archive, CheckCircle2, ChevronDown, Plus, RefreshCw, X } from 'lucide-react';
import { commands } from '../api/commands';
import type { AppError } from '../api/errors';
import type {
  AssessmentActivity,
  AssessmentStatus,
  AssessmentType,
  SchoolClass,
  SpeakingConfigurationSnapshot,
} from '../api/types';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { LoadingButton } from '../components/common/LoadingButton';
import { ProjectContextState } from '../components/common/ProjectContextState';
import { useProjectContext } from '../state/useProjectContext';
import {
  assessmentTypeLabels,
  formatDurationRange,
} from './assessmentOrganizationUi';
import { resolveNextExamStep } from '../app/examWorkspace';

type SpeakingType = 'prepared' | 'impromptu';

const emptySpeaking = {
  type: 'prepared' as SpeakingType,
  task: '',
  min: '120',
  target: '180',
  max: '240',
};
const EMPTY_CLASSES: SchoolClass[] = [];

function classLabel(schoolClass: SchoolClass): string {
  return schoolClass.displayName || schoolClass.name;
}

function applicationProgress(activity: AssessmentActivity, applicationId: string): string {
  const application = activity.classApplications.find((item) => item.id === applicationId);
  if (!application || application.status === 'archived') return 'Arşivlendi';
  if (activity.assessmentType !== 'speaking') {
    return application.status === 'completed' ? 'Tamamlandı' : 'Başlanmadı';
  }
  const completed = application.speakingAttempts.filter((attempt) => (
    attempt.state === 'approved' || attempt.state === 'teacher_review'
  )).length;
  if (completed === 0) return 'Başlanmadı';
  return `${completed}/${application.studentScopeIds.length || 0} tamamlandı`;
}

export function AssessmentOrganizationPage() {
  const { projectId, projectPath, isResolving } = useProjectContext();
  const queryClient = useQueryClient();
  const [searchParams, setSearchParams] = useSearchParams();
  const [isCreateOpen, setIsCreateOpen] = useState(false);
  const [editingActivityId, setEditingActivityId] = useState<string | null>(null);
  const [isDirty, setIsDirty] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const [filterCourseId, setFilterCourseId] = useState('');
  const [filterTerm, setFilterTerm] = useState('');
  const [filterAssessmentType, setFilterAssessmentType] = useState<AssessmentType | ''>('');
  const [filterStatus, setFilterStatus] = useState<AssessmentStatus | ''>('');
  const [showOtherFilters, setShowOtherFilters] = useState(false);
  const [filterGradeLevel, setFilterGradeLevel] = useState('');

  const [courseKey, setCourseKey] = useState('');
  const [activityTitle, setActivityTitle] = useState('');
  const [term, setTerm] = useState(1);
  const [assessmentType, setAssessmentType] = useState<AssessmentType>('written');
  const [sequenceNumber, setSequenceNumber] = useState(1);
  const [selectedClassIds, setSelectedClassIds] = useState<string[]>([]);
  const [speaking, setSpeaking] = useState(emptySpeaking);
  const [listeningInstruction, setListeningInstruction] = useState('');
  const [listeningDuration, setListeningDuration] = useState('');
  const [listeningPlayCount, setListeningPlayCount] = useState('');

  const classesQuery = useQuery({
    queryKey: ['school-classes', projectId, 'all'],
    queryFn: () => commands.listSchoolClasses({ projectId, includeArchived: true }),
    enabled: !!projectId,
  });
  const projectQuery = useQuery({
    queryKey: ['project-snapshot', projectId],
    queryFn: () => commands.getProjectSnapshot(projectId),
    enabled: !!projectId,
  });
  const assignmentsQuery = useQuery({
    queryKey: ['teaching-assignments', projectId],
    queryFn: () => commands.listTeachingAssignments({ projectId }),
    enabled: !!projectId,
  });
  const activitiesQuery = useQuery({
    queryKey: ['assessment-activities', projectId, filterCourseId, filterGradeLevel, filterTerm, filterAssessmentType, filterStatus],
    queryFn: () => commands.listAssessmentActivities({
      projectId,
      courseId: filterCourseId || undefined,
      gradeLevel: filterGradeLevel ? Number(filterGradeLevel) : undefined,
      term: filterTerm ? Number(filterTerm) : undefined,
      assessmentType: filterAssessmentType || undefined,
      status: filterStatus || undefined,
    }),
    enabled: !!projectId,
  });
  const allActivitiesQuery = useQuery({
    queryKey: ['assessment-activities', projectId, 'all'],
    queryFn: () => commands.listAssessmentActivities({ projectId }),
    enabled: !!projectId,
  });
  const classes = classesQuery.data ?? EMPTY_CLASSES;
  const assignments = useMemo(
    () => (assignmentsQuery.data ?? []).filter((assignment) => assignment.isActive),
    [assignmentsQuery.data],
  );
  const classesById = useMemo(
    () => new Map(classes.map((schoolClass) => [schoolClass.id, schoolClass])),
    [classes],
  );
  const courseOptions = useMemo(() => {
    const options = new Map<string, { key: string; id: string; name: string; year: string }>();
    for (const assignment of assignments) {
      const key = `${assignment.academicYearId}:${assignment.courseId}`;
      if (!options.has(key)) {
        options.set(key, {
          key,
          id: assignment.courseId,
          name: assignment.courseName,
          year: assignment.academicYearId,
        });
      }
    }
    return [...options.values()].sort((left, right) => left.name.localeCompare(right.name, 'tr'));
  }, [assignments]);
  const selectedCourse = courseOptions.find((option) => option.key === courseKey);
  const sequenceQuery = useQuery({
    queryKey: ['assessment-sequence-options', projectId, selectedCourse?.year, selectedCourse?.id, term, assessmentType],
    queryFn: () => commands.getAssessmentSequenceOptions({
      projectId,
      academicYearId: selectedCourse?.year ?? '',
      courseId: selectedCourse?.id ?? '',
      term,
      assessmentType,
    }),
    enabled: !!projectId && !!selectedCourse,
  });
  const availableClasses = useMemo(() => {
    if (!selectedCourse) return [];
    const ids = new Set(assignments
      .filter((assignment) => `${assignment.academicYearId}:${assignment.courseId}` === selectedCourse.key)
      .map((assignment) => assignment.classSectionId));
    return classes
      .filter((schoolClass) => schoolClass.status === 'active' && ids.has(schoolClass.id))
      .sort((left, right) => left.displayOrder - right.displayOrder);
  }, [assignments, classes, selectedCourse]);
  const selectedClasses = availableClasses.filter((schoolClass) => selectedClassIds.includes(schoolClass.id));
  const selectedGradeLevels = [...new Set(selectedClasses.map((schoolClass) => schoolClass.gradeLevel).filter((value): value is number => value !== null && value !== undefined))];
  const derivedGradeLevel = selectedGradeLevels.length === 1 ? selectedGradeLevels[0] : null;
  const sequenceOptions = useMemo(() => {
    const backendOptions = sequenceQuery.data?.options ?? [];
    if (editingActivityId && !backendOptions.includes(sequenceNumber)) return [sequenceNumber, ...backendOptions];
    return backendOptions;
  }, [editingActivityId, sequenceNumber, sequenceQuery.data?.options]);
  const activeAssignmentsExist = assignments.length > 0;

  const closeCreateMode = useCallback(() => {
    if (isDirty && !window.confirm('Kaydedilmemiş değişiklikler var. Form kapatılsın mı?')) return;
    setIsCreateOpen(false);
    resetForm();
  }, [isDirty]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && isCreateOpen) {
        closeCreateMode();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isCreateOpen, isDirty, closeCreateMode]);

  useEffect(() => {
    const queryType = searchParams.get('assessmentType');
    setFilterAssessmentType(queryType === 'written' || queryType === 'listening' || queryType === 'speaking' ? queryType : '');
  }, [searchParams]);

  useEffect(() => {
    if (sequenceOptions.length > 0 && !sequenceOptions.includes(sequenceNumber)) {
      setSequenceNumber(sequenceOptions[0] ?? 1);
    }
  }, [sequenceNumber, sequenceOptions]);

  const refresh = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ['assessment-activities', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['teaching-assignments', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['school-classes', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] }),
    ]);
  };

  const resetForm = () => {
    setEditingActivityId(null);
    setCourseKey('');
    setActivityTitle('');
    setTerm(1);
    setAssessmentType('written');
    setSequenceNumber(1);
    setSelectedClassIds([]);
    setSpeaking(emptySpeaking);
    setListeningInstruction('');
    setListeningDuration('');
    setListeningPlayCount('');
    setIsDirty(false);
  };


  const openCreateMode = () => {
    if (!activeAssignmentsExist) {
      window.alert("Sınav oluşturabilmek için önce Ders Alanı Kurulumunda bir ders–sınıf görevlendirmesi oluşturun.");
      return;
    }
    resetForm();
    setIsCreateOpen(true);
  };

  const openEditMode = (activity: AssessmentActivity) => {
    const matchingCourse = courseOptions.find((option) => option.id === activity.courseId && option.year === activity.academicYearId);
    setEditingActivityId(activity.id);
    setCourseKey(matchingCourse?.key ?? '');
    setActivityTitle(activity.title);
    setTerm(activity.term);
    setAssessmentType(activity.assessmentType);
    setSequenceNumber(activity.sequenceNumber);
    setSelectedClassIds(activity.classApplications.filter((application) => application.status !== 'archived').map((application) => application.schoolClassId));
    if (activity.speakingConfiguration) {
      setSpeaking({
        type: activity.speakingConfiguration.speakingType === 'impromptu' ? 'impromptu' : 'prepared',
        task: activity.speakingConfiguration.taskText,
        min: String(activity.speakingConfiguration.minDurationSeconds),
        target: String(activity.speakingConfiguration.targetDurationSeconds),
        max: String(activity.speakingConfiguration.maxDurationSeconds),
      });
    }
    setIsDirty(false);
    setIsCreateOpen(true);
  };

  const createMutation = useMutation({
    mutationFn: () => {
      if (!selectedCourse || !derivedGradeLevel) throw new Error('Form tamamlanmadı');
      return commands.createAssessmentActivity({
        projectId,
        academicYearId: selectedCourse.year,
        courseId: selectedCourse.id,
        courseName: selectedCourse.name,
        gradeLevel: derivedGradeLevel,
        term,
        assessmentType,
        sequenceNumber,
        schoolClassIds: selectedClassIds,
        title: activityTitle.trim() || `${term}. Dönem ${sequenceNumber}. ${assessmentTypeLabels[assessmentType]}`,
        speakingConfiguration: assessmentType === 'speaking' ? {
          speakingType: speaking.type,
          taskText: speaking.task.trim(),
          targetDurationSeconds: Number(speaking.target),
          minDurationSeconds: Number(speaking.min),
          maxDurationSeconds: Number(speaking.max),
          rubricVersion: speaking.type === 'prepared' ? 'tymm-prepared-speaking-v1' : 'tymm-impromptu-speaking-v1',
          scoringPolicyVersion: 'speaking_scoring_policy_v2',
          cleanupPromptVersion: 'speaking_asr_cleanup_tr_v3',
          evaluationPromptVersion: 'speaking_rubric_evidence_tr_v4',
          rubricSnapshot: {},
        } satisfies SpeakingConfigurationSnapshot : undefined,
        listeningDetails: assessmentType === 'listening' ? {
          instruction: listeningInstruction.trim() || undefined,
          durationSeconds: listeningDuration ? Number(listeningDuration) : undefined,
          playCount: listeningPlayCount ? Number(listeningPlayCount) : undefined,
        } : undefined,
      });
    },
    onMutate: () => { setError(null); setSuccessMessage(null); },
    onSuccess: async (activity) => {
      setSuccessMessage(`${activity.title || assessmentTypeLabels[activity.assessmentType]} oluşturuldu.`);
      setIsCreateOpen(false);
      resetForm();
      await refresh();
    },
    onError: (caught: AppError) => setError(caught),
  });

  const updateMutation = useMutation({
    mutationFn: () => {
      if (!editingActivityId) throw new Error('Düzenlenecek sınav seçilmedi');
      return commands.updateAssessmentActivity({
        projectId,
        activityId: editingActivityId,
        title: activityTitle.trim(),
        speakingConfiguration: assessmentType === 'speaking' ? {
          speakingType: speaking.type,
          taskText: speaking.task.trim(),
          targetDurationSeconds: Number(speaking.target),
          minDurationSeconds: Number(speaking.min),
          maxDurationSeconds: Number(speaking.max),
          rubricVersion: speaking.type === 'prepared' ? 'tymm-prepared-speaking-v1' : 'tymm-impromptu-speaking-v1',
          scoringPolicyVersion: 'speaking_scoring_policy_v2',
          cleanupPromptVersion: 'speaking_asr_cleanup_tr_v3',
          evaluationPromptVersion: 'speaking_rubric_evidence_tr_v4',
          rubricSnapshot: {},
        } : undefined,
      });
    },
    onMutate: () => { setError(null); setSuccessMessage(null); },
    onSuccess: async () => {
      setSuccessMessage('Sınav bilgileri güncellendi.');
      setIsCreateOpen(false);
      resetForm();
      await refresh();
    },
    onError: (caught: AppError) => setError(caught),
  });

  const archiveMutation = useMutation({
    mutationFn: (input: { activityId: string; applicationId: string }) => commands.archiveAssessmentClassApplication({ projectId, ...input }),
    onSuccess: refresh,
    onError: (caught: AppError) => setError(caught),
  });

  if (isResolving) return <ProjectContextState pageLabel="Sınav Organizasyonu" loading projectPath={projectPath} />;
  if (!projectId) return <ProjectContextState pageLabel="Sınav Organizasyonu" projectPath={projectPath} />;

  const filteredActivities = (activitiesQuery.data ?? []).filter((activity) => {
    const needle = search.trim().toLocaleLowerCase('tr-TR');
    if (!needle) return true;
    return [activity.title, activity.courseName, ...activity.classApplications.map((application) => classLabel(classesById.get(application.schoolClassId) ?? { name: '', normalizedName: '', id: '', displayOrder: 0, status: 'active', createdAt: '', updatedAt: '' }))].some((value) => value.toLocaleLowerCase('tr-TR').includes(needle));
  });
  const selectedStudentCount = selectedClasses.reduce((total, schoolClass) => (
    total + (projectQuery.data?.students.filter((student) => {
      const studentClass = student.className?.trim().toLocaleLowerCase('tr-TR');
      return studentClass === schoolClass.name.trim().toLocaleLowerCase('tr-TR')
        || studentClass === schoolClass.normalizedName.trim().toLocaleLowerCase('tr-TR');
    }).length ?? (allActivitiesQuery.data ?? []).flatMap((activity) => activity.classApplications).find((application) => application.schoolClassId === schoolClass.id)?.studentScopeIds.length ?? 0)
  ), 0);
  const missingFields: string[] = [];
  if (!selectedCourse) missingFields.push('Ders');
  if (selectedClasses.length === 0) missingFields.push('En az bir sınıf');
  if (selectedGradeLevels.length > 1) missingFields.push('Aynı sınıf düzeyi');
  if (!editingActivityId && (!sequenceQuery.isSuccess || !sequenceOptions.includes(sequenceNumber))) missingFields.push('Sınav sırası');
  if (assessmentType === 'speaking') {
    if (!speaking.task.trim()) missingFields.push('Konuşma görevi');
    if (!(Number(speaking.min) > 0 && Number(speaking.min) <= Number(speaking.target) && Number(speaking.target) <= Number(speaking.max))) missingFields.push('Geçerli konuşma süreleri');
  }
  const canSubmit = activeAssignmentsExist && missingFields.length === 0 && !createMutation.isPending && !updateMutation.isPending;
  const formSubmitting = createMutation.isPending || updateMutation.isPending;

  const toggleFilterType = (value: AssessmentType | '') => {
    setFilterAssessmentType(value);
    const params = new URLSearchParams(searchParams);
    if (value) params.set('assessmentType', value); else params.delete('assessmentType');
    setSearchParams(params, { replace: true });
  };
  const clearFilters = () => {
    setSearch(''); setFilterCourseId(''); setFilterGradeLevel(''); setFilterTerm(''); setFilterAssessmentType(''); setFilterStatus('');
    const params = new URLSearchParams(searchParams); params.delete('assessmentType'); setSearchParams(params, { replace: true });
  };

  return (
    <div className="assessment-page">
      <header className="assessment-page__header">
        <div><h2>Sınav Organizasyonu</h2><p>Ortak sınavları ve sınıf uygulamalarını yönetin.</p></div>
        <button type="button" className="button button--primary" onClick={openCreateMode} disabled={!activeAssignmentsExist} title={!activeAssignmentsExist ? "Sınav oluşturabilmek için önce Ders Alanı Kurulumunda bir ders–sınıf görevlendirmesi oluşturun." : undefined}><Plus size={17} /> Yeni sınav oluştur</button>
      </header>

      {(error || classesQuery.error || projectQuery.error || assignmentsQuery.error || activitiesQuery.error) && <ErrorBanner error={(error || classesQuery.error || projectQuery.error || assignmentsQuery.error || activitiesQuery.error) as AppError} />}
      {successMessage && <div className="classes-notice" role="status"><CheckCircle2 size={17} />{successMessage}</div>}

      {!activeAssignmentsExist && (
        <section className="assessment-setup-blocker" aria-label="Kurulum eksik">
          <div><strong>Kurulum eksik</strong><p>Sınav oluşturabilmek için önce ders–sınıf görevlendirmelerini tamamlayın.</p></div>
          <Link className="button button--secondary" to={`/project/${encodeURIComponent(projectId)}/classes`}>Kuruluma git</Link>
        </section>
      )}

      <section className="assessment-list-section" aria-labelledby="activity-list-heading">
        <div className="assessment-toolbar">
          <div><h3 id="activity-list-heading">Sınavlar</h3><span>Her ortak sınav, sınıf uygulamalarıyla birlikte tek kartta görünür.</span></div>
          <div className="assessment-toolbar__actions"><button type="button" className="button button--secondary" onClick={() => void refresh()}><RefreshCw size={15} /> Yenile</button></div>
        </div>
        <div className="assessment-filter-toolbar" aria-label="Sınav filtreleri">
          <input aria-label="Sınav ara" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Ara…" />
          {courseOptions.length > 1 && <select aria-label="Ders filtresi" value={filterCourseId} onChange={(event) => setFilterCourseId(event.target.value)}><option value="">Ders</option>{courseOptions.map((option) => <option key={option.key} value={option.id}>{option.name}</option>)}</select>}
          <select aria-label="Dönem filtresi" value={filterTerm} onChange={(event) => setFilterTerm(event.target.value)}><option value="">Dönem</option><option value="1">1. Dönem</option><option value="2">2. Dönem</option></select>
          <select aria-label="Tür filtresi" value={filterAssessmentType} onChange={(event) => toggleFilterType(event.target.value as AssessmentType | '')}><option value="">Tür</option><option value="written">Yazılı</option><option value="listening">Dinleme</option><option value="speaking">Konuşma</option></select>
          <button type="button" className="filter-more-button" aria-expanded={showOtherFilters} onClick={() => setShowOtherFilters((current) => !current)}>Diğer filtreler <ChevronDown size={15} /></button>
          {showOtherFilters && <><select aria-label="Sınıf düzeyi filtresi" value={filterGradeLevel} onChange={(event) => setFilterGradeLevel(event.target.value)}><option value="">Sınıf düzeyi</option>{[9, 10, 11, 12].map((grade) => <option key={grade} value={grade}>{grade}. sınıf</option>)}</select><select aria-label="Durum filtresi" value={filterStatus} onChange={(event) => setFilterStatus(event.target.value as AssessmentStatus | '')}><option value="">Durum</option><option value="draft">Taslak</option><option value="scheduled">Planlandı</option><option value="active">Aktif</option><option value="completed">Tamamlandı</option><option value="archived">Arşivlendi</option></select></>}
          {(search || filterCourseId || filterTerm || filterAssessmentType || filterStatus || filterGradeLevel) && <button type="button" className="filter-clear-button" onClick={clearFilters}>Tüm filtreleri temizle</button>}
        </div>
        {filteredActivities.length === 0 ? (
          <div className="assessment-empty-state">
            <strong>Henüz sınav oluşturulmadı</strong>
            <span>{activeAssignmentsExist ? "Yazılı, dinleme veya konuşma sınavı oluşturabilir ve aynı sınavı birden fazla sınıfa uygulayabilirsiniz." : "Sınav oluşturabilmek için önce Ders Alanı Kurulumunda bir ders–sınıf görevlendirmesi oluşturun."}</span>
            {activeAssignmentsExist ? (
              <button type="button" className="button button--primary" onClick={openCreateMode}>
                + Yeni sınav oluştur
              </button>
            ) : (
              <Link className="button button--primary" to={`/project/${encodeURIComponent(projectId)}/classes`}>
                Kurulumu tamamla
              </Link>
            )}
          </div>
        ) : (
          <div className="assessment-card-grid">
            {filteredActivities.map((activity) => {
              const applications = activity.classApplications.filter((application) => application.status !== 'archived');
              return <article key={activity.id} className="assessment-card">
                <div className="assessment-card__heading"><div><h3>{activity.title || `${activity.term}. Dönem ${activity.sequenceNumber}. ${assessmentTypeLabels[activity.assessmentType]}`}</h3><span>{activity.courseName} · {activity.gradeLevel}. sınıf</span></div><span className="assessment-card__status">{activity.status === 'draft' ? 'Taslak' : activity.status === 'completed' ? 'Tamamlandı' : 'Planlandı'}</span></div>
                {activity.assessmentType === 'speaking' && activity.speakingConfiguration && <p className="assessment-card__meta">{activity.speakingConfiguration.speakingType === 'prepared' ? 'Hazırlıklı' : 'Hazırlıksız'} · {formatDurationRange(activity.speakingConfiguration.minDurationSeconds, activity.speakingConfiguration.maxDurationSeconds)}</p>}
                <p className="assessment-card__meta">{applications.length} sınıf · {applications.reduce((total, application) => total + application.studentScopeIds.length, 0)} öğrenci</p>
                <div className="assessment-card__applications">{applications.length === 0 ? <p className="assessment-card__empty-application">Bu sınava bağlı aktif sınıf uygulaması yok.</p> : applications.map((application) => <div key={application.id}><span>{classesById.get(application.schoolClassId) ? classLabel(classesById.get(application.schoolClassId)!) : 'Sınıf bilgisi yok'}</span><small>{applicationProgress(activity, application.id)}</small>{application.status !== 'archived' && <button type="button" className="icon-button" aria-label="Sınıf uygulamasını arşivle" onClick={() => archiveMutation.mutate({ activityId: activity.id, applicationId: application.id })}><Archive size={15} /></button>}</div>)}</div>
                <div className="assessment-card__actions">{(() => {
                  const nextStep = resolveNextExamStep(activity, projectQuery.data?.workflow);
                  return <Link className="button button--secondary" to={`/project/${encodeURIComponent(projectId)}/activities/${encodeURIComponent(activity.id)}/${nextStep.id}`}>Devam et</Link>;
                })()}<button type="button" className="button button--secondary" onClick={() => openEditMode(activity)}>Düzenle</button></div>
              </article>;
            })}
          </div>
        )}
      </section>

      {isCreateOpen && <div className="assessment-drawer-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) closeCreateMode(); }}><aside className="assessment-drawer" role="dialog" aria-modal="true" aria-labelledby="assessment-create-heading">
        <div className="assessment-drawer__header"><div><h2 id="assessment-create-heading">{editingActivityId ? 'Sınavı düzenle' : 'Yeni sınav oluştur'}</h2><p>Ortak sınav ve sınıf uygulamaları tek işlemde oluşturulur.</p></div><button type="button" className="icon-button" onClick={closeCreateMode} aria-label="Formu kapat"><X size={19} /></button></div>
        {!activeAssignmentsExist && <div className="classes-warning">Önce <Link to={`/project/${encodeURIComponent(projectId)}/classes`}>Ders Alanı Kurulumu</Link> sayfasında aktif ders–sınıf görevlendirmesi oluşturun.</div>}
        <form onSubmit={(event) => { event.preventDefault(); if (!canSubmit) return; if (editingActivityId) updateMutation.mutate(); else createMutation.mutate(); }}>
          <section className="assessment-form-section"><h3>A. Sınav bilgileri</h3><div className="assessment-form-grid"><div className="readonly-field"><span>Eğitim yılı</span><strong>{selectedCourse?.year || 'Ders seçildiğinde gelir'}</strong></div><label><span>Ders</span><select value={courseKey} disabled={!activeAssignmentsExist || !!editingActivityId} onChange={(event) => { setCourseKey(event.target.value); setSelectedClassIds([]); setIsDirty(true); }}><option value="">Ders seçin</option>{courseOptions.map((option) => <option key={option.key} value={option.key}>{option.name}</option>)}</select></label><label><span>Dönem</span><select value={term} disabled={!!editingActivityId} onChange={(event) => { setTerm(Number(event.target.value)); setSelectedClassIds([]); setIsDirty(true); }}><option value={1}>1. Dönem</option><option value={2}>2. Dönem</option></select></label><label><span>Sınav türü</span><select value={assessmentType} disabled={!!editingActivityId} onChange={(event) => { setAssessmentType(event.target.value as AssessmentType); setSequenceNumber(1); setIsDirty(true); }}><option value="written">Yazılı</option><option value="listening">Dinleme</option><option value="speaking">Konuşma</option></select></label><label><span>Sınav sırası</span><select value={sequenceNumber} disabled={!!editingActivityId} onChange={(event) => { setSequenceNumber(Number(event.target.value)); setIsDirty(true); }}>{sequenceOptions.map((value) => <option key={value} value={value}>{value}. sınav</option>)}</select></label><label className="assessment-form-grid__wide"><span>Ortak sınav adı <small>(isteğe bağlı)</small></span><input value={activityTitle} onChange={(event) => { setActivityTitle(event.target.value); setIsDirty(true); }} placeholder={`${term}. Dönem ${sequenceNumber}. ${assessmentTypeLabels[assessmentType]}`} /></label></div></section>
          <section className="assessment-form-section"><h3>B. Uygulanacak sınıflar</h3>{!selectedCourse ? <p className="assessment-form-help">Aktif görevlendirmelerden bir ders seçin.</p> : availableClasses.length === 0 ? <p className="assessment-form-help">Bu derse atanmış aktif sınıf yok. <Link to={`/project/${encodeURIComponent(projectId)}/classes`}>Kuruluma git</Link></p> : <><div className="assessment-class-selection__actions"><button type="button" className="filter-clear-button" onClick={() => { setSelectedClassIds(availableClasses.map((schoolClass) => schoolClass.id)); setIsDirty(true); }}>Tümünü seç</button><button type="button" className="filter-clear-button" onClick={() => { setSelectedClassIds([]); setIsDirty(true); }}>Seçimi temizle</button></div><div className="assessment-class-selection">{availableClasses.map((schoolClass) => <label key={schoolClass.id}><input type="checkbox" checked={selectedClassIds.includes(schoolClass.id)} onChange={(event) => { setSelectedClassIds((current) => event.target.checked ? [...current, schoolClass.id] : current.filter((id) => id !== schoolClass.id)); setIsDirty(true); }} /><span><strong>{classLabel(schoolClass)}</strong><small>{schoolClass.gradeLevel ? `${schoolClass.gradeLevel}. sınıf` : 'Sınıf düzeyi yok'}</small></span></label>)}</div>{selectedGradeLevels.length > 1 && <p className="classes-warning">Farklı sınıf düzeyleri aynı sınava seçilemez. Tek bir düzey seçin.</p>}</>}</section>
          {assessmentType === 'speaking' && <section className="assessment-form-section"><h3>C. Konuşma ayarları</h3><div className="assessment-speaking-config"><label><span>Konuşma türü</span><select value={speaking.type} onChange={(event) => { setSpeaking((current) => ({ ...current, type: event.target.value as SpeakingType })); setIsDirty(true); }}><option value="prepared">Hazırlıklı</option><option value="impromptu">Hazırlıksız</option></select></label><label><span>Minimum süre (sn)</span><input type="number" min={1} value={speaking.min} onChange={(event) => { setSpeaking((current) => ({ ...current, min: event.target.value })); setIsDirty(true); }} /></label><label><span>Önerilen süre (sn)</span><input type="number" min={1} value={speaking.target} onChange={(event) => { setSpeaking((current) => ({ ...current, target: event.target.value })); setIsDirty(true); }} /></label><label><span>Maksimum süre (sn)</span><input type="number" min={1} value={speaking.max} onChange={(event) => { setSpeaking((current) => ({ ...current, max: event.target.value })); setIsDirty(true); }} /></label><label className="assessment-speaking-config__task"><span>Konuşma görevi</span><textarea rows={4} value={speaking.task} onChange={(event) => { setSpeaking((current) => ({ ...current, task: event.target.value })); setIsDirty(true); }} placeholder="Ortak görev metni" /></label></div></section>}
          {assessmentType === 'listening' && <section className="assessment-form-section"><h3>C. Dinleme ayarları</h3><div className="assessment-form-grid"><label><span>Dinleme talimatı</span><textarea rows={3} value={listeningInstruction} onChange={(event) => { setListeningInstruction(event.target.value); setIsDirty(true); }} /></label><label><span>Süre (sn)</span><input type="number" min={1} value={listeningDuration} onChange={(event) => { setListeningDuration(event.target.value); setIsDirty(true); }} /></label><label><span>Dinletme sayısı</span><input type="number" min={1} value={listeningPlayCount} onChange={(event) => { setListeningPlayCount(event.target.value); setIsDirty(true); }} /></label></div></section>}
          {selectedCourse && selectedClasses.length > 0 && (
          <section className="assessment-live-summary"><h3>Canlı özet</h3><strong>{activityTitle.trim() || `${term}. Dönem ${sequenceNumber}. ${assessmentTypeLabels[assessmentType]}`}</strong><span>Ders: {selectedCourse?.name || '—'}</span><span>Sınıflar: {selectedClasses.length ? selectedClasses.map(classLabel).join(', ') : '—'}</span><span>Toplam öğrenci: {selectedStudentCount || '—'}</span>{assessmentType === 'speaking' && <span>Tür: {speaking.type === 'prepared' ? 'Hazırlıklı' : 'Hazırlıksız'} · Süre: {formatDurationRange(Number(speaking.min), Number(speaking.max))}</span>}{missingFields.length > 0 && <div><strong>Eksik:</strong>{missingFields.map((field) => <span key={field}>• {field}</span>)}</div>}</section>
        )}
          <div className="assessment-drawer__actions"><button type="button" className="button button--secondary" onClick={closeCreateMode}>İptal</button><LoadingButton type="submit" className="button button--primary" loading={formSubmitting} disabledReason={!canSubmit ? (missingFields.length ? `Eksik: ${missingFields.join(', ')}` : 'Kurulumu tamamlayın.') : undefined}>{editingActivityId ? 'Değişiklikleri kaydet' : 'Sınavı ve sınıf uygulamalarını oluştur'}</LoadingButton></div>
        </form>
      </aside></div>}
    </div>
  );
}

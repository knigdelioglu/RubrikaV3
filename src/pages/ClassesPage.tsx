import { useEffect, useMemo, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Archive, CheckCircle2, Pencil, RotateCcw, Users } from 'lucide-react';
import { commands } from '../api/commands';
import type { AppError } from '../api/errors';
import type { SchoolClass, TeachingAssignment } from '../api/types';
import { ConfirmationDialog } from '../components/common/ConfirmationDialog';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { LoadingButton } from '../components/common/LoadingButton';
import { ProjectContextState } from '../components/common/ProjectContextState';
import { useProjectContext } from '../state/useProjectContext';
import { ClassStudentRosterPage } from './ClassStudentRosterPage';
import { getClassesSetupTargetId, getClassesTab, setClassesTab } from './classesUi';

type ClassDraft = {
  academicYear: string;
  gradeLevel: string;
  section: string;
};

const emptyDraft: ClassDraft = { academicYear: '', gradeLevel: '', section: '' };

function derivedClassName(draft: ClassDraft): string {
  return [draft.gradeLevel.trim(), draft.section.trim().toLocaleUpperCase('tr-TR')].filter(Boolean).join('-');
}

export function ClassesPage() {
  const { projectId, projectPath, isResolving } = useProjectContext();
  const queryClient = useQueryClient();
  const [searchParams, setSearchParams] = useSearchParams();

  const [draft, setDraft] = useState<ClassDraft>(emptyDraft);
  const [editingClassId, setEditingClassId] = useState<string | null>(null);
  const [classToArchive, setClassToArchive] = useState<SchoolClass | null>(null);

  // Step 1: Course Info state
  const [editingCourse, setEditingCourse] = useState(false);
  const [courseCodeDraft, setCourseCodeDraft] = useState('');
  const [courseNameDraft, setCourseNameDraft] = useState('');
  const [academicYearDraft, setAcademicYearDraft] = useState('2026-2027');

  // Step 3: Batch assignment state
  const [selectedClassIdsForAssign, setSelectedClassIdsForAssign] = useState<string[]>([]);

  const [error, setError] = useState<AppError | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const activeTab = getClassesTab(searchParams);
  const setupParam = searchParams.get('setup');

  const classesQuery = useQuery({
    queryKey: ['school-classes', projectId, 'all'],
    queryFn: () => commands.listSchoolClasses({ projectId, includeArchived: true }),
    enabled: !!projectId,
  });
  const activitiesQuery = useQuery({
    queryKey: ['assessment-activities', projectId, 'course-definitions'],
    queryFn: () => commands.listAssessmentActivities({ projectId }),
    enabled: !!projectId,
  });
  const assignmentsQuery = useQuery({
    queryKey: ['teaching-assignments', projectId],
    queryFn: () => commands.listTeachingAssignments({ projectId, includeInactive: true }),
    enabled: !!projectId,
  });
  const projectQuery = useQuery({
    queryKey: ['project-snapshot', projectId],
    queryFn: () => commands.getProjectSnapshot(projectId),
    enabled: !!projectId,
  });

  const project = projectQuery.data;
  const classes = classesQuery.data ?? [];
  const activeClasses = useMemo(() => (classesQuery.data ?? []).filter((c) => c.status === 'active'), [classesQuery.data]);
  const assignments = assignmentsQuery.data ?? [];
  const activeAssignments = useMemo(() => (assignmentsQuery.data ?? []).filter((a) => a.isActive), [assignmentsQuery.data]);

  // Derived Course Info
  const courseCode = project?.courseId || (assignments[0]?.courseId) || (activitiesQuery.data?.[0]?.courseId) || '';
  const courseName = project?.courseName || (assignments[0]?.courseName) || (activitiesQuery.data?.[0]?.courseName) || '';
  const academicYear = project?.academicYearId || (assignments[0]?.academicYearId) || '2026-2027';

  const hasCourse = Boolean(courseCode && courseName);
  const hasClasses = activeClasses.length > 0;
  const hasAssignments = activeAssignments.length > 0;
  const isSetupComplete = hasCourse && hasClasses && hasAssignments;

  useEffect(() => {
    if (project) {
      if (project.courseId) setCourseCodeDraft(project.courseId);
      if (project.courseName) setCourseNameDraft(project.courseName);
      if (project.academicYearId) setAcademicYearDraft(project.academicYearId);
    }
  }, [project]);

  // Auto scroll effect for ?setup=course | classes | assignments
  useEffect(() => {
    if (activeTab === 'roster') return;
    const targetId = getClassesSetupTargetId(setupParam);
    if (!targetId) return;
    const timeoutId = window.setTimeout(() => {
      document.getElementById(targetId)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }, 150);
    return () => window.clearTimeout(timeoutId);
  }, [activeTab, setupParam]);

  const refresh = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ['school-classes', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['teaching-assignments', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['assessment-activities', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['data-loss-preflight', projectPath] }),
    ]);
  };

  const updateCourseMutation = useMutation({
    mutationFn: () => {
      if (!courseCodeDraft.trim() || !courseNameDraft.trim() || !academicYearDraft.trim()) {
        throw new Error('Ders kodu, ders adı ve eğitim yılı zorunludur.');
      }
      return commands.updateCourseInfo({
        projectId,
        academicYearId: academicYearDraft.trim(),
        courseId: courseCodeDraft.trim().toLowerCase(),
        courseName: courseNameDraft.trim(),
        expectedRevision: project?.storageRevision,
      });
    },
    onMutate: () => { setError(null); setSuccessMessage(null); },
    onSuccess: async () => {
      setSuccessMessage('Ders bilgileri kaydedildi.');
      setEditingCourse(false);
      await refresh();
    },
    onError: (caught: AppError) => setError(caught),
  });

  const saveClassMutation = useMutation({
    mutationFn: async () => {
      const common = {
        projectId,
        name: derivedClassName(draft),
        academicYear: draft.academicYear.trim() || academicYear || undefined,
        gradeLevel: draft.gradeLevel.trim() ? Number(draft.gradeLevel) : undefined,
        section: draft.section.trim() || undefined,
      };
      return editingClassId
        ? commands.updateSchoolClass({ ...common, classId: editingClassId })
        : commands.createSchoolClass(common);
    },
    onMutate: () => { setError(null); setSuccessMessage(null); },
    onSuccess: async (schoolClass) => {
      setSuccessMessage(editingClassId ? `${schoolClass.name} güncellendi.` : `${schoolClass.name} oluşturuldu.`);
      setEditingClassId(null);
      setDraft(emptyDraft);
      await refresh();
    },
    onError: (caught: AppError) => setError(caught),
  });

  const archiveMutation = useMutation({
    mutationFn: (schoolClass: SchoolClass) => commands.archiveSchoolClass({ projectId, classId: schoolClass.id }),
    onSuccess: async (schoolClass) => {
      setClassToArchive(null);
      setSuccessMessage(`${schoolClass.name} arşivlendi.`);
      await refresh();
    },
    onError: (caught: AppError) => { setClassToArchive(null); setError(caught); },
  });

  const restoreMutation = useMutation({
    mutationFn: (schoolClass: SchoolClass) => commands.restoreSchoolClass({ projectId, classId: schoolClass.id }),
    onSuccess: async (schoolClass) => {
      setSuccessMessage(`${schoolClass.name} yeniden etkinleştirildi.`);
      await refresh();
    },
    onError: (caught: AppError) => setError(caught),
  });

  const activeAssignmentClassIds = useMemo(
    () => new Set(activeAssignments.map((a) => a.classSectionId)),
    [activeAssignments],
  );

  const unassignedClassIds = useMemo(
    () => activeClasses.filter((c) => !activeAssignmentClassIds.has(c.id)).map((c) => c.id),
    [activeClasses, activeAssignmentClassIds],
  );

  const unassignedSelectedIds = useMemo(
    () => selectedClassIdsForAssign.filter((id) => unassignedClassIds.includes(id)),
    [selectedClassIdsForAssign, unassignedClassIds],
  );

  const batchAssignMutation = useMutation({
    mutationFn: () => {
      if (!hasCourse) throw new Error('Önce ders bilgileri kaydedilmelidir.');
      if (unassignedSelectedIds.length === 0) throw new Error('Görevlendirilecek yeni sınıf seçin.');
      return commands.batchCreateTeachingAssignments({
        projectId,
        academicYearId: academicYear,
        courseId: courseCode,
        courseName,
        classSectionIds: unassignedSelectedIds,
      });
    },
    onMutate: () => { setError(null); setSuccessMessage(null); },
    onSuccess: async (res) => {
      setSuccessMessage(`${res.length} sınıf ${courseName} dersine görevlendirildi.`);
      setSelectedClassIdsForAssign([]);
      await refresh();
    },
    onError: (caught: AppError) => setError(caught),
  });

  const archiveAssignmentMutation = useMutation({
    mutationFn: (assignment: TeachingAssignment) => commands.archiveTeachingAssignment({ projectId, assignmentId: assignment.id }),
    onSuccess: async (assignment) => {
      setSuccessMessage(`${assignment.courseName} görevlendirmesi arşivlendi.`);
      await refresh();
    },
    onError: (caught: AppError) => setError(caught),
  });

  if (isResolving) return <ProjectContextState pageLabel="Sınıflar" loading projectPath={projectPath} />;
  if (!projectId) return <ProjectContextState pageLabel="Sınıflar" projectPath={projectPath} />;

  const queryError = (classesQuery.error ?? assignmentsQuery.error ?? activitiesQuery.error ?? projectQuery.error) as AppError | null;
  const canSaveClass = draft.gradeLevel.trim().length > 0 && draft.section.trim().length > 0 && !saveClassMutation.isPending;

  const getStudentCount = (cls: SchoolClass) => {
    return project?.students.filter((student) => {
      const sClass = student.className?.trim().toLocaleLowerCase('tr-TR');
      return sClass === cls.name.trim().toLocaleLowerCase('tr-TR') || sClass === cls.normalizedName.trim().toLocaleLowerCase('tr-TR');
    }).length ?? 0;
  };

  const selectedClassesWithZeroStudents = activeClasses.filter(
    (cls) => unassignedSelectedIds.includes(cls.id) && getStudentCount(cls) === 0,
  );

  return (
    <div className="classes-page" style={{ paddingBottom: '3rem' }}>
      <header className="classes-page__header">
        <div>
          <h2>Sınıflar ve Öğrenciler</h2>
          <p>Ders Alanı Kurulumunu, sınıfları, merkezi öğrenci listesini ve ders–sınıf görevlendirmelerini bu alandan yönetin.</p>
        </div>
      </header>

      <div className="exam-package-tabs" role="tablist" style={{ marginBottom: '1.5rem' }}>
        <button
          type="button"
          data-project-write="false"
          className={activeTab === 'classes' ? 'is-active' : ''}
          onClick={() => setSearchParams(setClassesTab(searchParams, 'classes'), { replace: true })}
        >
          Sınıflar ve Görevlendirmeler
        </button>
        <button
          type="button"
          data-project-write="false"
          className={activeTab === 'roster' ? 'is-active' : ''}
          onClick={() => setSearchParams(setClassesTab(searchParams, 'roster'), { replace: true })}
        >
          Merkezi Öğrenci Listesi
        </button>
      </div>

      {(error || queryError) && <ErrorBanner error={(error || queryError)!} onRefresh={refresh} />}
      {successMessage && <div className="classes-notice" role="status"><CheckCircle2 size={17} />{successMessage}</div>}

      {activeTab === 'roster' ? (
        <ClassStudentRosterPage />
      ) : (
        <>
          {/* Top Compact Progress Indicator */}
          <div className="setup-progress-bar" style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))', gap: '1rem', padding: '1rem 1.25rem', background: '#fff', border: '1px solid #e2e8f0', borderRadius: '1rem', marginBottom: '1.5rem' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
              <span style={{ width: 28, height: 28, borderRadius: '50%', background: hasCourse ? '#dcfce7' : '#e0e7ff', color: hasCourse ? '#166534' : '#3730a3', fontWeight: 800, display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: '0.85rem', flexShrink: 0 }}>
                {hasCourse ? '✓' : '1'}
              </span>
              <div>
                <strong style={{ fontSize: '0.9rem', color: '#0f172a', display: 'block' }}>1. Ders bilgileri</strong>
                <small style={{ color: '#64748b', fontSize: '0.75rem' }}>{hasCourse ? `${courseName} (${courseCode.toUpperCase()})` : 'Ders tanımı bekleniyor'}</small>
              </div>
            </div>

            <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
              <span style={{ width: 28, height: 28, borderRadius: '50%', background: hasClasses ? '#dcfce7' : hasCourse ? '#e0e7ff' : '#f1f5f9', color: hasClasses ? '#166534' : hasCourse ? '#3730a3' : '#94a3b8', fontWeight: 800, display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: '0.85rem', flexShrink: 0 }}>
                {hasClasses ? '✓' : '2'}
              </span>
              <div>
                <strong style={{ fontSize: '0.9rem', color: '#0f172a', display: 'block' }}>2. Sınıflar</strong>
                <small style={{ color: '#64748b', fontSize: '0.75rem' }}>{hasClasses ? `${activeClasses.length} aktif sınıf` : 'En az 1 sınıf olmalı'}</small>
              </div>
            </div>

            <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
              <span style={{ width: 28, height: 28, borderRadius: '50%', background: hasAssignments ? '#dcfce7' : (hasCourse && hasClasses) ? '#e0e7ff' : '#f1f5f9', color: hasAssignments ? '#166534' : (hasCourse && hasClasses) ? '#3730a3' : '#94a3b8', fontWeight: 800, display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: '0.85rem', flexShrink: 0 }}>
                {hasAssignments ? '✓' : '3'}
              </span>
              <div>
                <strong style={{ fontSize: '0.9rem', color: '#0f172a', display: 'block' }}>3. Ders–sınıf görevlendirmeleri</strong>
                <small style={{ color: '#64748b', fontSize: '0.75rem' }}>{hasAssignments ? `${activeAssignments.length} sınıf görevlendirildi` : 'Görevlendirme bekleniyor'}</small>
              </div>
            </div>
          </div>

          {/* Step 1: Course Info */}
          <section id="setup-step-course" style={{ padding: '1.25rem', background: '#fff', border: '1px solid #e2e8f0', borderRadius: '1rem', marginBottom: '1.5rem' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: '1rem', marginBottom: '0.75rem' }}>
              <div>
                <h3 style={{ margin: 0, fontSize: '1.1rem', fontWeight: 700, color: '#0f172a' }}>1. Ders Bilgileri</h3>
                <p style={{ margin: '0.25rem 0 0', color: '#64748b', fontSize: '0.85rem' }}>
                  Dersinizin tanımını ve akademik yılını bu alandan belirleyin.
                </p>
              </div>
              {hasCourse && !editingCourse && (
                <button type="button" data-project-write="false" className="button button--secondary" onClick={() => setEditingCourse(true)} style={{ fontSize: '0.8rem' }}>
                  Düzenle
                </button>
              )}
            </div>

            {hasCourse && !editingCourse ? (
              <div style={{ padding: '1rem', background: '#f8fafc', borderRadius: '0.75rem', border: '1px solid #cbd5e1', display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '0.75rem' }}>
                <div>
                  <strong style={{ fontSize: '1.05rem', color: '#0f172a' }}>{courseName} ({courseCode.toUpperCase()})</strong>
                  <span style={{ color: '#64748b', fontSize: '0.85rem', display: 'block', marginTop: '0.2rem' }}>Eğitim yılı: {academicYear}</span>
                </div>
                <span style={{ fontSize: '0.8rem', fontWeight: 700, color: '#166534', background: '#dcfce7', padding: '0.35rem 0.75rem', borderRadius: '9999px' }}>
                  ✓ Kaydedildi
                </span>
              </div>
            ) : (
              <form onSubmit={(e) => { e.preventDefault(); updateCourseMutation.mutate(); }} style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: '1rem', alignItems: 'end', marginTop: '1rem' }}>
                <label style={{ display: 'flex', flexDirection: 'column', gap: '0.35rem', fontSize: '0.85rem', color: '#334155' }}>
                  <span>Ders kodu</span>
                  <input
                    type="text"
                    value={courseCodeDraft}
                    onChange={(e) => setCourseCodeDraft(e.target.value)}
                    placeholder="TDE"
                    required
                    style={{ padding: '0.6rem 0.75rem', borderRadius: '0.5rem', border: '1px solid #cbd5e1' }}
                  />
                </label>

                <label style={{ display: 'flex', flexDirection: 'column', gap: '0.35rem', fontSize: '0.85rem', color: '#334155' }}>
                  <span>Ders adı</span>
                  <input
                    type="text"
                    value={courseNameDraft}
                    onChange={(e) => setCourseNameDraft(e.target.value)}
                    placeholder="Türk Dili ve Edebiyatı"
                    required
                    style={{ padding: '0.6rem 0.75rem', borderRadius: '0.5rem', border: '1px solid #cbd5e1' }}
                  />
                </label>

                <label style={{ display: 'flex', flexDirection: 'column', gap: '0.35rem', fontSize: '0.85rem', color: '#334155' }}>
                  <span>Eğitim yılı</span>
                  <input
                    type="text"
                    value={academicYearDraft}
                    onChange={(e) => setAcademicYearDraft(e.target.value)}
                    placeholder="2026-2027"
                    required
                    style={{ padding: '0.6rem 0.75rem', borderRadius: '0.5rem', border: '1px solid #cbd5e1' }}
                  />
                </label>

                <div style={{ display: 'flex', gap: '0.5rem' }}>
                  {editingCourse && hasCourse && (
                    <button type="button" data-project-write="false" className="button button--secondary" onClick={() => setEditingCourse(false)}>
                      İptal
                    </button>
                  )}
                  <LoadingButton type="submit" className="button button--primary" loading={updateCourseMutation.isPending}>
                    Ders Bilgisini Kaydet
                  </LoadingButton>
                </div>
              </form>
            )}
          </section>

          {/* Step 2: Classes */}
          <section id="setup-step-classes" style={{ marginBottom: '1.5rem' }}>
            <div style={{ marginBottom: '0.75rem' }}>
              <h3 style={{ margin: 0, fontSize: '1.1rem', fontWeight: 700, color: '#0f172a' }}>2. Sınıflar</h3>
              <p style={{ margin: '0.25rem 0 0', color: '#64748b', fontSize: '0.85rem' }}>
                Öğrenci şubelerinizi oluşturun ve yönetin.
              </p>
            </div>

            {!hasCourse ? (
              <div className="classes-warning" style={{ padding: '1rem', background: '#fffbeb', border: '1px solid #fef08a', borderRadius: '0.75rem', color: '#92400e', fontSize: '0.875rem' }}>
                ⚠️ Önce 1. adımdaki ders bilgilerini kaydetmeniz gereklidir.
              </div>
            ) : (
              <>
                <form className="class-editor" onSubmit={(event) => { event.preventDefault(); if (canSaveClass) saveClassMutation.mutate(); }} style={{ marginBottom: '1.25rem' }}>
                  <div className="class-editor__heading">
                    <div><strong>{editingClassId ? 'Sınıfı düzenle' : 'Yeni sınıf'}</strong><span>{derivedClassName(draft) || 'Düzey ve şube seçin'}</span></div>
                    {editingClassId && <button type="button" data-project-write="false" className="button button--secondary" onClick={() => { setEditingClassId(null); setDraft(emptyDraft); }}>İptal</button>}
                  </div>
                  <label><span>Akademik yıl</span><input value={draft.academicYear || academicYear} onChange={(event) => setDraft((current) => ({ ...current, academicYear: event.target.value }))} placeholder="2026-2027" /></label>
                  <label><span>Sınıf düzeyi</span><input type="number" min={1} max={12} value={draft.gradeLevel} onChange={(event) => setDraft((current) => ({ ...current, gradeLevel: event.target.value }))} placeholder="11" /></label>
                  <label><span>Şube</span><input value={draft.section} onChange={(event) => setDraft((current) => ({ ...current, section: event.target.value }))} placeholder="A" /></label>
                  <LoadingButton type="submit" className="button button--primary" loading={saveClassMutation.isPending} disabledReason={!canSaveClass ? 'Sınıf düzeyi ve şube zorunludur.' : undefined}>
                    {editingClassId ? 'Değişiklikleri Kaydet' : 'Sınıf Oluştur'}
                  </LoadingButton>
                </form>

                {classesQuery.isLoading ? (
                  <div className="classes-empty" role="status">Sınıf bilgileri yükleniyor…</div>
                ) : classes.length === 0 ? (
                  <div className="classes-empty"><Users size={28} /><strong>Henüz sınıf yok</strong><span>Önce en az bir sınıf oluşturun.</span></div>
                ) : (
                  <div className="class-card-grid">
                    {classes.map((schoolClass) => {
                      const classAssignments = activeAssignments.filter((assignment) => assignment.classSectionId === schoolClass.id);
                      const studentCount = getStudentCount(schoolClass);

                      return (
                        <article key={schoolClass.id} className={`class-card ${schoolClass.status === "archived" ? "is-archived" : ""}`}>
                          <div className="class-card__heading">
                            <div>
                              <h3>{schoolClass.name}</h3>
                              <span>{schoolClass.status === "active" ? "Aktif sınıf" : "Arşivlenmiş sınıf"}</span>
                            </div>
                            <div className="class-card__actions">
                              <button type="button" data-project-write="false" className="icon-button" aria-label={`${schoolClass.name} sınıfını düzenle`} onClick={() => {
                                setEditingClassId(schoolClass.id);
                                setDraft({
                                  academicYear: schoolClass.academicYear ?? "",
                                  gradeLevel: schoolClass.gradeLevel?.toString() ?? "",
                                  section: schoolClass.section ?? "",
                                });
                              }}><Pencil size={16} /></button>
                              {schoolClass.status === "active" ? (
                                <button type="button" data-project-write="false" className="icon-button" aria-label={`${schoolClass.name} sınıfını arşivle`} onClick={() => setClassToArchive(schoolClass)}><Archive size={16} /></button>
                              ) : (
                                <button type="button" data-project-write="true" className="icon-button" aria-label={`${schoolClass.name} sınıfını yeniden etkinleştir`} onClick={() => restoreMutation.mutate(schoolClass)} disabled={restoreMutation.isPending}><RotateCcw size={16} /></button>
                              )}
                            </div>
                          </div>
                          <div style={{ padding: "0.75rem 0", borderTop: "1px solid #f1f5f9", marginTop: "0.5rem" }}>
                            <div style={{ fontSize: "1.1rem", fontWeight: 800, color: "#0f172a" }}>{studentCount} öğrenci</div>
                            <p style={{ margin: "0.25rem 0 0", fontSize: "0.8rem", color: "#64748b" }}>
                              {classAssignments.length > 0
                                ? `${classAssignments.map((a) => a.courseName).join(", ")} görevlendirmesi aktif`
                                : "Ders görevlendirmesi bulunmuyor"}
                            </p>
                          </div>
                          <div className="class-card__links" style={{ marginTop: "0.5rem" }}>
                            <button type="button" data-project-write="false" className="button button--secondary" style={{ fontSize: "0.8rem", width: "100%", justifyContent: "center" }} onClick={() => setSearchParams(setClassesTab(searchParams, 'roster'), { replace: true })}>
                              Öğrencileri yönet
                            </button>
                          </div>
                        </article>
                      );
                    })}
                  </div>
                )}
              </>
            )}
          </section>

          {/* Step 3: Teaching Assignments */}
          <section id="setup-step-assignments" style={{ padding: '1.25rem', background: '#fff', border: '1px solid #e2e8f0', borderRadius: '1rem', marginBottom: '1.5rem' }}>
            <h3 style={{ margin: 0, fontSize: '1.1rem', fontWeight: 700, color: '#0f172a' }}>3. Ders–Sınıf Görevlendirmeleri</h3>
            <p style={{ margin: '0.25rem 0 1rem', color: '#64748b', fontSize: '0.85rem' }}>
              {hasCourse ? `${courseName} (${academicYear}) dersi için görevlendirilecek sınıfları seçin.` : 'Önce ders bilgileri kaydedilmelidir.'}
            </p>

            {!hasCourse || !hasClasses ? (
              <div className="classes-warning" style={{ padding: '1rem', background: '#fffbeb', border: '1px solid #fef08a', borderRadius: '0.75rem', color: '#92400e', fontSize: '0.875rem' }}>
                ⚠️ Görevlendirme yapabilmek için ders bilgileri ve en az bir aktif sınıf hazır olmalıdır.
              </div>
            ) : (
              <div>
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(220px, 1fr))', gap: '0.75rem', marginBottom: '1rem' }}>
                  {activeClasses.map((cls) => {
                    const isAssigned = activeAssignmentClassIds.has(cls.id);
                    const studentCount = getStudentCount(cls);
                    const isChecked = selectedClassIdsForAssign.includes(cls.id) || isAssigned;

                    return (
                      <label
                        key={cls.id}
                        style={{
                          padding: '0.85rem 1rem',
                          background: isAssigned ? '#f8fafc' : '#fff',
                          border: isChecked ? '2px solid #4f46e5' : '1px solid #cbd5e1',
                          borderRadius: '0.75rem',
                          cursor: isAssigned ? 'default' : 'pointer',
                          display: 'flex',
                          alignItems: 'center',
                          gap: '0.75rem',
                          userSelect: 'none',
                        }}
                      >
                        <input
                          type="checkbox"
                          disabled={isAssigned}
                          checked={isChecked}
                          onChange={(e) => {
                            if (isAssigned) return;
                            setSelectedClassIdsForAssign((current) =>
                              e.target.checked ? [...current, cls.id] : current.filter((id) => id !== cls.id),
                            );
                          }}
                        />
                        <div>
                          <strong style={{ display: 'block', fontSize: '0.95rem', color: '#0f172a' }}>{cls.name}</strong>
                          <small style={{ color: '#64748b', fontSize: '0.75rem' }}>{studentCount} öğrenci</small>
                          {isAssigned && <span style={{ display: 'block', fontSize: '0.72rem', color: '#166534', fontWeight: 700, marginTop: '0.2rem' }}>✓ Görevlendirildi</span>}
                          {studentCount === 0 && !isAssigned && <span style={{ display: 'block', fontSize: '0.72rem', color: '#b45309', marginTop: '0.2rem' }}>⚠️ Sınıfta öğrenci yok</span>}
                        </div>
                      </label>
                    );
                  })}
                </div>

                {selectedClassesWithZeroStudents.length > 0 && (
                  <div style={{ padding: '0.75rem 1rem', background: '#fff7ed', border: '1px solid #fed7aa', borderRadius: '0.5rem', color: '#9a3412', fontSize: '0.8rem', marginBottom: '1rem' }}>
                    ⚠️ Seçilen sınıflarda ({selectedClassesWithZeroStudents.map((c) => c.name).join(', ')}) henüz öğrenci bulunmuyor. Görevlendirme yapılabilir; sınav öncesi öğrenci ekleyebilirsiniz.
                  </div>
                )}

                <LoadingButton
                  type="button"
                  className="button button--primary"
                  loading={batchAssignMutation.isPending}
                  disabledReason={unassignedSelectedIds.length === 0 ? 'Görevlendirilecek yeni sınıf seçin.' : undefined}
                  onClick={() => batchAssignMutation.mutate()}
                >
                  Seçili Sınıfları Görevlendir ({unassignedSelectedIds.length})
                </LoadingButton>

                {activeAssignments.length > 0 && (
                  <div style={{ marginTop: '1.25rem', borderTop: '1px solid #f1f5f9', paddingTop: '1rem' }}>
                    <strong style={{ fontSize: '0.85rem', color: '#475569', display: 'block', marginBottom: '0.5rem' }}>Aktif Görevlendirmeler</strong>
                    <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
                      {activeAssignments.map((a) => (
                        <span key={a.id} style={{ padding: '0.35rem 0.75rem', background: '#f1f5f9', borderRadius: '0.5rem', fontSize: '0.8rem', color: '#1e293b', fontWeight: 600, display: 'inline-flex', alignItems: 'center', gap: '0.5rem' }}>
                          {a.courseName} · {classes.find((c) => c.id === a.classSectionId)?.name || a.classSectionId}
                          <button type="button" data-project-write="true" style={{ border: 'none', background: 'none', cursor: 'pointer', color: '#64748b', padding: 0 }} onClick={() => archiveAssignmentMutation.mutate(a)} aria-label="Görevlendirmeyi arşivle">
                            <Archive size={14} />
                          </button>
                        </span>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            )}
          </section>

          {/* Bottom Completion Card */}
          {isSetupComplete ? (
            <div style={{ padding: '1.5rem', background: '#f0fdf4', border: '1px solid #bbf7d0', borderRadius: '1rem', textAlign: 'center' }}>
              <h3 style={{ margin: 0, fontSize: '1.15rem', color: '#166534', fontWeight: 700 }}>✓ Ders Alanı Kurulumu Tamamlandı</h3>
              <p style={{ margin: '0.35rem 0 1.25rem', color: '#15803d', fontSize: '0.875rem' }}>
                {activeAssignments.length} sınıf {courseName} dersine görevlendirildi.
              </p>
              <Link to={`/project/${encodeURIComponent(projectId)}/activities`} className="button button--primary" style={{ padding: '0.75rem 1.5rem', fontSize: '0.95rem' }}>
                Sınav oluşturmaya geç →
              </Link>
            </div>
          ) : (
            <div style={{ padding: '1.25rem', background: '#f8fafc', border: '1px solid #e2e8f0', borderRadius: '1rem', textAlign: 'center', color: '#64748b', fontSize: '0.875rem' }}>
              Sınav oluşturabilmek için yukarıdaki 3 kurulum adımını (Ders bilgileri, Sınıflar, Görevlendirmeler) tamamlayın.
            </div>
          )}
        </>
      )}

      <ConfirmationDialog
        open={classToArchive !== null}
        title={`${classToArchive?.name ?? "Sınıf"} arşivlensin mi?`}
        description="Sınıf listede pasif görünür. İlişkili kayıtlar silinmez."
        confirmLabel="Sınıfı Arşivle"
        busy={archiveMutation.isPending}
        onCancel={() => setClassToArchive(null)}
        onConfirm={() => { if (classToArchive) archiveMutation.mutate(classToArchive); }}
      />
    </div>
  );
}

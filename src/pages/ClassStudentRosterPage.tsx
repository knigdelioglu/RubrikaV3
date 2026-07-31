import { useEffect, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { CheckCircle2, Pencil, UserPlus, Users } from 'lucide-react';
import { commands } from '../api/commands';
import type { AppError } from '../api/errors';
import type { SchoolClass, Student } from '../api/types';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { LoadingButton } from '../components/common/LoadingButton';
import { ProjectContextState } from '../components/common/ProjectContextState';
import { useProjectContext } from '../state/useProjectContext';

type StudentDraft = {
  displayName: string;
  number: string;
};

const emptyDraft: StudentDraft = { displayName: '', number: '' };
const EMPTY_CLASSES: SchoolClass[] = [];

function studentLabel(student: Student): string {
  return student.displayName?.trim() || student.number?.trim() || 'İsimsiz öğrenci';
}

export function ClassStudentRosterPage() {
  const { projectId, projectPath, isResolving } = useProjectContext();
  const [searchParams, setSearchParams] = useSearchParams();
  const queryClient = useQueryClient();
  const [selectedClassId, setSelectedClassId] = useState(searchParams.get('classId') ?? '');
  const [draft, setDraft] = useState<StudentDraft>(emptyDraft);
  const [editingStudentId, setEditingStudentId] = useState<string | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);

  const classesQuery = useQuery({
    queryKey: ['school-classes', projectId, 'active'],
    queryFn: () => commands.listSchoolClasses({ projectId, includeArchived: false }),
    enabled: !!projectId,
  });
  const classes = classesQuery.data ?? EMPTY_CLASSES;

  useEffect(() => {
    if (selectedClassId && classes.some((schoolClass) => schoolClass.id === selectedClassId)) return;
    const nextClassId = classes[0]?.id ?? '';
    setSelectedClassId(nextClassId);
    const next = new URLSearchParams(searchParams);
    if (nextClassId) next.set('classId', nextClassId); else next.delete('classId');
    setSearchParams(next, { replace: true });
  }, [classes, searchParams, selectedClassId, setSearchParams]);

  const studentsQuery = useQuery({
    queryKey: ['class-students', projectId, selectedClassId],
    queryFn: () => commands.listClassStudents({ projectId, classId: selectedClassId }),
    enabled: !!projectId && !!selectedClassId,
  });

  const refresh = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ['class-students', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['assessment-activities', projectId] }),
    ]);
  };

  const saveMutation = useMutation({
    mutationFn: () => {
      const input = {
        projectId,
        classId: selectedClassId,
        displayName: draft.displayName.trim() || undefined,
        number: draft.number.trim() || undefined,
      };
      return editingStudentId
        ? commands.updateClassStudent({ ...input, studentId: editingStudentId })
        : commands.createClassStudent(input);
    },
    onMutate: () => { setError(null); setSuccessMessage(null); },
    onSuccess: async (student) => {
      setSuccessMessage(`${studentLabel(student)} ${editingStudentId ? 'güncellendi' : 'sınıfa kaydedildi'}.`);
      setDraft(emptyDraft);
      setEditingStudentId(null);
      await refresh();
    },
    onError: (caught: AppError) => setError(caught),
  });

  if (isResolving || classesQuery.isLoading) {
    return <ProjectContextState pageLabel="Sınıf öğrencileri" loading projectPath={projectPath} />;
  }
  if (!projectId) return <ProjectContextState pageLabel="Sınıf öğrencileri" projectPath={projectPath} />;

  const selectedClass = classes.find((schoolClass) => schoolClass.id === selectedClassId);
  const students = studentsQuery.data ?? [];
  const canSave = !!selectedClassId
    && (draft.displayName.trim().length > 0 || draft.number.trim().length > 0)
    && !saveMutation.isPending;
  const queryError = (classesQuery.error ?? studentsQuery.error) as AppError | null;

  const selectClass = (classId: string) => {
    setSelectedClassId(classId);
    const next = new URLSearchParams(searchParams);
    if (classId) next.set('classId', classId); else next.delete('classId');
    setSearchParams(next);
    setEditingStudentId(null);
    setDraft(emptyDraft);
  };

  const editStudent = (student: Student) => {
    setEditingStudentId(student.id);
    setDraft({ displayName: student.displayName ?? '', number: student.number ?? '' });
    setError(null);
    setSuccessMessage(null);
  };

  return (
    <div className="class-student-roster">
      <header className="class-student-roster__header">
        <div>
          <h2>Sınıf Öğrencileri</h2>
          <p>Sınavdan önce öğrencileri sınıfa kaydedin. Yazılı, dinleme ve konuşma sınavları bu ortak listeyi kullanır.</p>
        </div>
        <Link className="button button--secondary" to={`/project/${encodeURIComponent(projectId)}/classes`}>Sınıf kurulumuna dön</Link>
      </header>

      {(error || queryError) && <ErrorBanner error={(error || queryError)!} />}
      {successMessage && <div className="classes-notice" role="status"><CheckCircle2 size={17} />{successMessage}</div>}

      {classes.length === 0 ? (
        <div className="class-student-roster__empty"><Users size={30} /><strong>Önce sınıf oluşturun</strong><span>Öğrenci kaydetmek için en az bir aktif sınıf gerekir.</span><Link className="button button--primary" to={`/project/${encodeURIComponent(projectId)}/classes`}>Sınıf oluştur</Link></div>
      ) : (
        <>
          <section className="class-student-roster__class-picker">
            <label><span>Sınıf</span><select value={selectedClassId} onChange={(event) => selectClass(event.target.value)}>{classes.map((schoolClass: SchoolClass) => <option key={schoolClass.id} value={schoolClass.id}>{schoolClass.displayName || schoolClass.name}</option>)}</select></label>
            <div><strong>{selectedClass?.displayName || selectedClass?.name}</strong><span>{students.length} öğrenci kayıtlı</span></div>
          </section>

          <section className="class-student-roster__editor">
            <div><h3>{editingStudentId ? 'Öğrenciyi düzenle' : 'Sınıfa öğrenci ekle'}</h3><p>Ad soyad veya okul numarasından en az biri zorunludur.</p></div>
            <form onSubmit={(event) => { event.preventDefault(); if (canSave) saveMutation.mutate(); }}>
              <label><span>Ad soyad</span><input value={draft.displayName} onChange={(event) => setDraft((current) => ({ ...current, displayName: event.target.value }))} placeholder="Örn. Ayşe Yılmaz" /></label>
              <label><span>Okul numarası</span><input value={draft.number} onChange={(event) => setDraft((current) => ({ ...current, number: event.target.value }))} placeholder="Örn. 1042" /></label>
              <div className="class-student-roster__editor-actions"><LoadingButton type="submit" className="button button--primary" loading={saveMutation.isPending} disabledReason={!canSave ? 'Ad soyad veya okul numarası girin.' : undefined}>{editingStudentId ? 'Öğrenciyi kaydet' : <><UserPlus size={16} /> Öğrenci ekle</>}</LoadingButton>{editingStudentId && <button type="button" className="button button--secondary" onClick={() => { setEditingStudentId(null); setDraft(emptyDraft); }}>İptal</button>}</div>
            </form>
          </section>

          <section className="class-student-roster__list">
            <div className="class-student-roster__list-heading"><h3>{selectedClass?.displayName || selectedClass?.name} öğrenci listesi</h3><span>{students.length} kayıt</span></div>
            {studentsQuery.isLoading ? <div className="classes-empty">Öğrenciler yükleniyor…</div> : students.length === 0 ? <div className="class-student-roster__empty class-student-roster__empty--compact"><Users size={24} /><span>Bu sınıfta henüz öğrenci yok. İlk öğrenciyi yukarıdaki formdan ekleyin.</span></div> : <div className="class-student-roster__rows">{students.map((student, index) => <div key={student.id} className="class-student-roster__row"><span className="class-student-roster__index">{index + 1}</span><div><strong>{student.displayName || 'Ad soyad girilmedi'}</strong><small>{student.number ? `Okul no: ${student.number}` : 'Okul numarası girilmedi'}</small></div><button type="button" className="icon-button" aria-label={`${studentLabel(student)} öğrencisini düzenle`} onClick={() => editStudent(student)}><Pencil size={16} /></button></div>)}</div>}
          </section>
        </>
      )}
    </div>
  );
}

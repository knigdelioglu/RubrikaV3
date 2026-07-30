import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Archive, CheckCircle2, FilePlus2, Pencil, RotateCcw, Users } from 'lucide-react';
import { commands } from '../api/commands';
import type { AppError } from '../api/errors';
import type { SchoolClass } from '../api/types';
import { ConfirmationDialog } from '../components/common/ConfirmationDialog';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { LoadingButton } from '../components/common/LoadingButton';
import { ProjectContextState } from '../components/common/ProjectContextState';
import { useProjectContext } from '../state/useProjectContext';

type ClassDraft = {
  name: string;
  academicYear: string;
  gradeLevel: string;
  section: string;
};

const emptyDraft: ClassDraft = { name: '', academicYear: '', gradeLevel: '', section: '' };

export function ClassesPage() {
  const { projectId, projectPath, isResolving } = useProjectContext();
  const queryClient = useQueryClient();
  const [draft, setDraft] = useState<ClassDraft>(emptyDraft);
  const [editingClassId, setEditingClassId] = useState<string | null>(null);
  const [classToArchive, setClassToArchive] = useState<SchoolClass | null>(null);
  const [moveTargets, setMoveTargets] = useState<Record<string, string>>({});
  const [error, setError] = useState<AppError | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);

  const classesQuery = useQuery({
    queryKey: ['school-classes', projectId, 'all'],
    queryFn: () => commands.listSchoolClasses({ projectId, includeArchived: true }),
    enabled: !!projectId,
  });
  const overviewQuery = useQuery({
    queryKey: ['school-class-overview', projectId],
    queryFn: () => commands.getSchoolClassOverview(projectId),
    enabled: !!projectId,
  });
  const batchesQuery = useQuery({
    queryKey: ['student-scan-batches', projectId],
    queryFn: () => commands.listStudentScanBatches({ projectId }),
    enabled: !!projectId,
  });

  const refresh = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ['school-classes', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['school-class-overview', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['student-scan-batches', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] }),
    ]);
  };

  const saveMutation = useMutation({
    mutationFn: async () => {
      const common = {
        projectId,
        name: draft.name.trim(),
        academicYear: draft.academicYear.trim() || undefined,
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

  const moveMutation = useMutation({
    mutationFn: ({ batchId, targetClassId }: { batchId: string; targetClassId: string }) => (
      commands.moveStudentScanBatch({ projectId, batchId, targetClassId })
    ),
    onSuccess: async (batch) => {
      setSuccessMessage(`${batch.displayName} seçilen sınıfa taşındı. OCR ve notlandırma kayıtları korundu.`);
      await refresh();
    },
    onError: (caught: AppError) => setError(caught),
  });

  if (isResolving) return <ProjectContextState pageLabel="Sınıflar" loading projectPath={projectPath} />;
  if (!projectId) return <ProjectContextState pageLabel="Sınıflar" projectPath={projectPath} />;

  const classes = classesQuery.data ?? [];
  const overviews = overviewQuery.data?.classes ?? [];
  const batches = batchesQuery.data ?? [];
  const queryError = (classesQuery.error ?? overviewQuery.error ?? batchesQuery.error) as AppError | null;
  const canSave = draft.name.trim().length > 0 && !saveMutation.isPending;

  return (
    <div className="classes-page">
      <header className="classes-page__header">
        <div>
          <h2>Sınıflar</h2>
          <p>Öğrenci PDF paketlerini ait oldukları sınıflarla yönetin.</p>
        </div>
        <Link className="button button--primary classes-page__upload" to={`/project/${encodeURIComponent(projectId)}/exam/documents?documentType=student`}>
          <FilePlus2 size={17} /> Öğrenci PDF Paketi Ekle
        </Link>
      </header>

      {(error || queryError) && <ErrorBanner error={(error || queryError)!} />}
      {successMessage && <div className="classes-notice" role="status"><CheckCircle2 size={17} />{successMessage}</div>}

      <form className="class-editor" onSubmit={(event) => { event.preventDefault(); if (canSave) saveMutation.mutate(); }}>
        <div className="class-editor__heading">
          <div><strong>{editingClassId ? 'Sınıfı düzenle' : 'Yeni sınıf'}</strong><span>Örn. 11-A</span></div>
          {editingClassId && <button type="button" className="button button--secondary" onClick={() => { setEditingClassId(null); setDraft(emptyDraft); }}>İptal</button>}
        </div>
        <label><span>Sınıf adı</span><input value={draft.name} onChange={(event) => setDraft((current) => ({ ...current, name: event.target.value }))} placeholder="11-A" required /></label>
        <label><span>Akademik yıl</span><input value={draft.academicYear} onChange={(event) => setDraft((current) => ({ ...current, academicYear: event.target.value }))} placeholder="2026-2027" /></label>
        <label><span>Sınıf düzeyi</span><input type="number" min={1} max={12} value={draft.gradeLevel} onChange={(event) => setDraft((current) => ({ ...current, gradeLevel: event.target.value }))} placeholder="11" /></label>
        <label><span>Şube</span><input value={draft.section} onChange={(event) => setDraft((current) => ({ ...current, section: event.target.value }))} placeholder="A" /></label>
        <LoadingButton type="submit" className="button button--primary" loading={saveMutation.isPending} disabledReason={!draft.name.trim() ? 'Sınıf adı zorunludur.' : undefined}>
          {editingClassId ? 'Değişiklikleri Kaydet' : 'Sınıf Oluştur'}
        </LoadingButton>
      </form>

      {classesQuery.isLoading || overviewQuery.isLoading ? (
        <div className="classes-empty" role="status">Sınıf bilgileri yükleniyor…</div>
      ) : overviews.length === 0 ? (
        <div className="classes-empty"><Users size={28} /><strong>Henüz sınıf yok</strong><span>İlk sınıfı oluşturduktan sonra öğrenci PDF paketi ekleyebilirsiniz.</span></div>
      ) : (
        <div className="class-card-grid">
          {overviews.map((overview) => {
            const schoolClass = overview.schoolClass;
            const classBatches = batches.filter((batch) => batch.classId === schoolClass.id);
            return (
              <article key={schoolClass.id} className={`class-card ${schoolClass.status === 'archived' ? 'is-archived' : ''}`}>
                <div className="class-card__heading">
                  <div><h3>{schoolClass.name}</h3><span>{schoolClass.status === 'active' ? 'Aktif sınıf' : 'Arşivlenmiş sınıf'}</span></div>
                  <div className="class-card__actions">
                    <button type="button" className="icon-button" aria-label={`${schoolClass.name} sınıfını düzenle`} onClick={() => {
                      setEditingClassId(schoolClass.id);
                      setDraft({
                        name: schoolClass.name,
                        academicYear: schoolClass.academicYear ?? '',
                        gradeLevel: schoolClass.gradeLevel?.toString() ?? '',
                        section: schoolClass.section ?? '',
                      });
                    }}><Pencil size={16} /></button>
                    {schoolClass.status === 'active' ? (
                      <button type="button" className="icon-button" aria-label={`${schoolClass.name} sınıfını arşivle`} onClick={() => setClassToArchive(schoolClass)}><Archive size={16} /></button>
                    ) : (
                      <button type="button" className="icon-button" aria-label={`${schoolClass.name} sınıfını yeniden etkinleştir`} onClick={() => restoreMutation.mutate(schoolClass)} disabled={restoreMutation.isPending}><RotateCcw size={16} /></button>
                    )}
                  </div>
                </div>
                <dl className="class-card__metrics">
                  <div><dt>PDF paketi</dt><dd>{overview.scanBatchCount}</dd></div>
                  <div><dt>Öğrenci</dt><dd>{overview.submissionCount}</dd></div>
                  <div><dt>Kimlik doğrulandı</dt><dd>{overview.identityVerifiedCount}</dd></div>
                  <div><dt>OCR tamamlandı</dt><dd>{overview.ocrCompleteCount}</dd></div>
                  <div><dt>Notlandırıldı</dt><dd>{overview.scoringCompleteCount}</dd></div>
                  <div className={overview.reviewRequiredCount > 0 ? 'has-warning' : ''}><dt>Kontrol gerekli</dt><dd>{overview.reviewRequiredCount}</dd></div>
                </dl>
                <div className="class-card__links">
                  <Link to={`/project/${encodeURIComponent(projectId)}/exam/documents?documentType=student&classId=${encodeURIComponent(schoolClass.id)}`}>Bu sınıfa PDF ekle</Link>
                  <Link to={`/project/${encodeURIComponent(projectId)}/students?tab=grouping&classId=${encodeURIComponent(schoolClass.id)}`}>Sınıf öğrencilerini aç</Link>
                </div>
                {classBatches.length > 0 && (
                  <div className="class-card__batches">
                    {classBatches.map((batch) => (
                      <div key={batch.id}>
                        <span><strong>{batch.displayName}</strong><small>{batch.groupingCompletedAt ? 'Gruplama tamamlandı' : 'Gruplama bekliyor'}</small></span>
                        <label>
                          <span className="sr-only">{batch.displayName} paketini başka sınıfa taşı</span>
                          <select value={moveTargets[batch.id] ?? schoolClass.id} onChange={(event) => setMoveTargets((current) => ({ ...current, [batch.id]: event.target.value }))}>
                            {classes.filter((item) => item.status === 'active').map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
                          </select>
                        </label>
                        <button type="button" className="button button--secondary" disabled={!moveTargets[batch.id] || moveTargets[batch.id] === schoolClass.id || moveMutation.isPending} onClick={() => moveMutation.mutate({ batchId: batch.id, targetClassId: moveTargets[batch.id]! })}>Taşı</button>
                      </div>
                    ))}
                  </div>
                )}
              </article>
            );
          })}
        </div>
      )}

      {(overviewQuery.data?.unassignedBatchCount ?? 0) + (overviewQuery.data?.unassignedSubmissionCount ?? 0) > 0 && (
        <div className="classes-warning">Sınıf ilişkisi belirlenmemiş legacy kayıtlar var. Veriler korunuyor; ilgili PDF paketini sınıfa taşıyarak düzenleyebilirsiniz.</div>
      )}

      <ConfirmationDialog
        open={classToArchive !== null}
        title={`${classToArchive?.name ?? 'Sınıf'} arşivlensin mi?`}
        description="Sınıf listede pasif görünür. İlişkili PDF paketleri, OCR ve notlandırma kayıtları silinmez."
        confirmLabel="Sınıfı Arşivle"
        busy={archiveMutation.isPending}
        onCancel={() => setClassToArchive(null)}
        onConfirm={() => { if (classToArchive) archiveMutation.mutate(classToArchive); }}
      />
    </div>
  );
}

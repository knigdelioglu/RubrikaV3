import { useEffect } from 'react';
import { Navigate, useLocation, useNavigate, useSearchParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { commands } from '../api/commands';
import type { AppError } from '../api/errors';
import type { StudentOperationsTab } from '../app/projectRoutes';
import { projectStudentOperationsPath } from '../app/projectRoutes';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { ProjectContextState } from '../components/common/ProjectContextState';
import { ClassSelector } from '../components/student/ClassSelector';
import { useProjectContext } from '../state/useProjectContext';
import { CropTemplatePage } from './CropTemplatePage';
import { StudentAnswerOcrIssueReviewPage } from './StudentAnswerOcrIssueReviewPage';
import { StudentAnswerOcrPage } from './StudentAnswerOcrPage';
import { StudentGroupingPage } from './StudentGroupingPage';
import { StudentIdentityPage } from './StudentIdentityPage';
import {
  aggregateClassOverview,
  normalizeStudentOperationsTab,
  resolveStudentOperationsSelection,
} from './studentOperations';

const tabs: Array<{ id: StudentOperationsTab; label: string }> = [
  { id: 'grouping', label: 'Gruplama' },
  { id: 'identity', label: 'Kimlik' },
  { id: 'crops', label: 'Crop Şablonları' },
  { id: 'ocr', label: 'Cevap OCR' },
  { id: 'issues', label: 'OCR Sorunları' },
];

export function StudentOperationsWorkspacePage() {
  const { projectId, projectPath, isResolving } = useProjectContext();
  const [searchParams, setSearchParams] = useSearchParams();
  const tab = normalizeStudentOperationsTab(searchParams.get('tab'));

  const projectQuery = useQuery({
    queryKey: ['project-snapshot', projectId],
    queryFn: () => commands.getProjectSnapshot(projectId),
    enabled: !!projectId,
  });
  const classesQuery = useQuery({
    queryKey: ['school-classes', projectId, 'active'],
    queryFn: () => commands.listSchoolClasses({ projectId, includeArchived: false }),
    enabled: !!projectId,
  });
  const batchesQuery = useQuery({
    queryKey: ['student-scan-batches', projectId],
    queryFn: () => commands.listStudentScanBatches({ projectId }),
    enabled: !!projectId,
  });
  const overviewQuery = useQuery({
    queryKey: ['school-class-overview', projectId],
    queryFn: () => commands.getSchoolClassOverview(projectId),
    enabled: !!projectId,
  });

  const classes = classesQuery.data ?? projectQuery.data?.schoolClasses ?? [];
  const batches = batchesQuery.data ?? projectQuery.data?.studentScanBatches ?? [];
  const selection = resolveStudentOperationsSelection(
    classes,
    batches,
    searchParams.get('classId'),
    searchParams.get('batchId'),
  );

  useEffect(() => {
    const requestedTab = searchParams.get('tab');
    const requestedClassId = searchParams.get('classId') ?? '';
    const requestedBatchId = searchParams.get('batchId') ?? '';
    if (requestedTab === tab && requestedClassId === selection.classId && requestedBatchId === selection.batchId) return;
    const next = new URLSearchParams(searchParams);
    next.set('tab', tab);
    if (selection.classId) next.set('classId', selection.classId); else next.delete('classId');
    if (selection.batchId) next.set('batchId', selection.batchId); else next.delete('batchId');
    setSearchParams(next, { replace: true });
  }, [searchParams, selection.batchId, selection.classId, setSearchParams, tab]);

  if (isResolving || projectQuery.isLoading) {
    return <ProjectContextState pageLabel="Öğrenci İşlemleri" loading projectPath={projectPath} />;
  }
  if (!projectId || !projectQuery.data) {
    return <ProjectContextState pageLabel="Öğrenci İşlemleri" projectPath={projectPath} />;
  }

  const queryError = (projectQuery.error ?? classesQuery.error ?? batchesQuery.error ?? overviewQuery.error) as AppError | null;
  const classOverviews = overviewQuery.data?.classes ?? [];
  const visibleOverview = selection.classId
    ? classOverviews.filter((item) => item.schoolClass.id === selection.classId)
    : classOverviews;
  const summary = aggregateClassOverview(visibleOverview);

  const updateSelection = (patch: { tab?: StudentOperationsTab; classId?: string; batchId?: string }) => {
    const next = new URLSearchParams(searchParams);
    if (patch.tab) next.set('tab', patch.tab);
    if (patch.classId !== undefined) {
      if (patch.classId) next.set('classId', patch.classId); else next.delete('classId');
      next.delete('batchId');
    }
    if (patch.batchId !== undefined) {
      if (patch.batchId) {
        const batch = batches.find((item) => item.id === patch.batchId);
        next.set('batchId', patch.batchId);
        if (batch) next.set('classId', batch.classId);
      } else {
        next.delete('batchId');
      }
    }
    setSearchParams(next);
  };

  return (
    <div className="student-operations-workspace">
      <header className="student-operations-workspace__header">
        <div><h2>Öğrenci İşlemleri</h2><p>Sınıf ve PDF paketi bağlamını koruyarak gruplama, kimlik ve OCR adımlarını yönetin.</p></div>
      </header>

      {queryError && <ErrorBanner error={queryError} />}

      <ClassSelector
        idPrefix="student-operations"
        classes={classes}
        batches={batches}
        classId={selection.classId}
        batchId={selection.batchId}
        onClassChange={(classId) => updateSelection({ classId })}
        onBatchChange={(batchId) => updateSelection({ batchId })}
      />

      <dl className="student-operations-summary" aria-label="Seçili sınıf ve paket özeti">
        <div><dt>PDF paketi</dt><dd>{summary.scanBatchCount}</dd></div>
        <div><dt>Öğrenci</dt><dd>{summary.submissionCount}</dd></div>
        <div><dt>Kimlik doğrulandı</dt><dd>{summary.identityVerifiedCount}</dd></div>
        <div><dt>OCR tamamlandı</dt><dd>{summary.ocrCompleteCount}</dd></div>
        <div><dt>Notlandırıldı</dt><dd>{summary.scoringCompleteCount}</dd></div>
        <div className={summary.reviewRequiredCount ? 'has-warning' : ''}><dt>Kontrol gerekli</dt><dd>{summary.reviewRequiredCount}</dd></div>
      </dl>

      <div className="student-operations-tabs" role="tablist" aria-label="Öğrenci işlemleri bölümleri">
        {tabs.map((item) => (
          <button key={item.id} type="button" role="tab" aria-selected={tab === item.id} className={tab === item.id ? 'is-active' : ''} onClick={() => updateSelection({ tab: item.id })}>{item.label}</button>
        ))}
      </div>

      <section className="student-operations-panel" role="tabpanel" aria-label={tabs.find((item) => item.id === tab)?.label}>
        {tab === 'grouping' && <StudentGroupingPage />}
        {tab === 'identity' && <StudentIdentityPage />}
        {tab === 'crops' && <CropTemplatePage />}
        {tab === 'ocr' && <StudentAnswerOcrPage />}
        {tab === 'issues' && <StudentAnswerOcrIssueReviewPage />}
      </section>
    </div>
  );
}

export function StudentOperationsCompatibilityRedirect({ tab }: { tab: StudentOperationsTab }) {
  const { projectId, projectPath, isResolving } = useProjectContext();
  const location = useLocation();
  if (isResolving) return <ProjectContextState pageLabel="Öğrenci İşlemleri" loading projectPath={projectPath} />;
  if (!projectId) return <Navigate to="/projects" replace />;
  return <Redirect projectId={projectId} tab={tab} search={location.search} />;
}

function Redirect({ projectId, tab, search }: { projectId: string; tab: StudentOperationsTab; search: string }) {
  const navigate = useNavigate();
  useEffect(() => {
    navigate(projectStudentOperationsPath(projectId, tab, search), { replace: true });
  }, [navigate, projectId, search, tab]);
  return <ProjectContextState pageLabel="Öğrenci İşlemleri" loading />;
}

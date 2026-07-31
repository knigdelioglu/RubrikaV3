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
import { StudentGroupingPage } from './StudentGroupingPage';
import { StudentIdentityPage } from './StudentIdentityPage';
import {
  aggregateClassOverview,
  normalizeStudentOperationsTab,
  resolveStudentOperationsSelection,
} from './studentOperations';

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
        <div>
          <h2>Öğrenci Kâğıtları</h2>
          <p>Taranmış kâğıtları yükleyin, sınıflara göre ayırın ve sayfaları öğrencilerle eşleştirin.</p>
        </div>
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
        <div><dt>Öğrenci kâğıdı</dt><dd>{summary.submissionCount}</dd></div>
        <div><dt>Kimlik doğrulandı</dt><dd>{summary.identityVerifiedCount}</dd></div>
        <div className={summary.reviewRequiredCount ? 'has-warning' : ''}><dt>Kontrol gerekli</dt><dd>{summary.reviewRequiredCount}</dd></div>
      </dl>

      <div className="student-operations-panel">
        <StudentGroupingPage />
      </div>

      <details className="package-technical-details" style={{ marginTop: '1.5rem' }}>
        <summary style={{ fontWeight: 600, cursor: 'pointer', padding: '0.75rem', background: '#f8fafc', borderRadius: '8px', border: '1px solid #e2e8f0' }}>
          Teknik düzeltme araçları (Kimlik OCR, Crop Şablonu ve İnceleme)
        </summary>
        <div style={{ padding: '1rem 0' }}>
          <div className="student-operations-tabs" role="tablist" style={{ marginBottom: '1rem' }}>
            <button
              type="button"
              className={tab === 'identity' ? 'is-active' : ''}
              onClick={() => updateSelection({ tab: 'identity' })}
            >
              Kimlik Doğrulama
            </button>
            <button
              type="button"
              className={tab === 'crops' ? 'is-active' : ''}
              onClick={() => updateSelection({ tab: 'crops' })}
            >
              Crop Şablonları
            </button>
          </div>

          <div>
            {tab === 'identity' && <StudentIdentityPage />}
            {tab === 'crops' && <CropTemplatePage />}
            {tab !== 'identity' && tab !== 'crops' && <StudentIdentityPage />}
          </div>
        </div>
      </details>
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

import { useMemo, useState } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { Volume2 } from 'lucide-react';
import { commands } from '../api/commands';
import type { SchoolClass } from '../api/types';
import { CanonicalExamWorkspaceHeader } from '../components/workspace/CanonicalExamWorkspaceHeader';
import { getExamStepDefinitions, resolveNextExamStep } from '../app/examWorkspace';
import { ProjectContextState } from '../components/common/ProjectContextState';
import { useProjectContext } from '../state/useProjectContext';

import { ExamPackageWorkspacePage } from './ExamPackageWorkspacePage';
import { StudentOperationsWorkspacePage } from './StudentOperationsWorkspacePage';
import { StudentAnswerOcrPage } from './StudentAnswerOcrPage';
import { ScoringPage } from './ScoringPage';
import { AnalysisPage } from './AnalysisPage';
import { SpeechExamPage } from './SpeechExamPage';
import { PerformanceOrganizationPage } from './PerformanceOrganizationPage';
import { PerformanceResultsView, PerformanceScoringPage } from './PerformanceScoringPage';

const EMPTY_CLASSES: SchoolClass[] = [];

function ListeningContentStepView({
  projectId,
  activityId,
}: {
  projectId: string;
  activityId: string;
}) {
  const { data: activity } = useQuery({
    queryKey: ['assessment-activity', projectId, activityId],
    queryFn: () => commands.getAssessmentActivity({ projectId, activityId }),
    enabled: !!projectId && !!activityId,
  });

  const details = activity?.listeningDetails;

  return (
    <div className="listening-content-step" style={{ padding: '1.5rem', background: '#fff', borderRadius: '12px', border: '1px solid #e2e8f0' }}>
      <header style={{ marginBottom: '1.5rem' }}>
        <h3 style={{ fontSize: '1.1rem', fontWeight: 700, color: '#0f172a', display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
          <Volume2 size={20} style={{ color: '#16a34a' }} /> Dinleme Sınavı İçeriği ve Yönergeler
        </h3>
        <p style={{ color: '#64748b', fontSize: '0.88rem' }}>
          Dinleme ses kaydı, oynatma sayısı, toplam süre ve öğrenciye aktarılacak yönergeleri bu alandan yönetin.
        </p>
      </header>

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))', gap: '1rem', marginBottom: '1.5rem' }}>
        <div style={{ padding: '1rem', background: '#f8fafc', borderRadius: '8px', border: '1px solid #cbd5e1' }}>
          <span style={{ fontSize: '0.8rem', color: '#64748b', fontWeight: 600 }}>Dinleme Kaydı</span>
          <p style={{ fontSize: '0.95rem', fontWeight: 700, color: '#1e293b', margin: '0.3rem 0' }}>
            {details?.audioDocumentId ? 'Yüklendi (Ses Dokümanı)' : 'Henüz ses kaydı yüklenmedi'}
          </p>
        </div>

        <div style={{ padding: '1rem', background: '#f8fafc', borderRadius: '8px', border: '1px solid #cbd5e1' }}>
          <span style={{ fontSize: '0.8rem', color: '#64748b', fontWeight: 600 }}>Oynatma Hakkı</span>
          <p style={{ fontSize: '0.95rem', fontWeight: 700, color: '#1e293b', margin: '0.3rem 0' }}>
            {details?.playCount ? `${details.playCount} defa` : 'Varsayılan (2 defa)'}
          </p>
        </div>

        <div style={{ padding: '1rem', background: '#f8fafc', borderRadius: '8px', border: '1px solid #cbd5e1' }}>
          <span style={{ fontSize: '0.8rem', color: '#64748b', fontWeight: 600 }}>Toplam Süre</span>
          <p style={{ fontSize: '0.95rem', fontWeight: 700, color: '#1e293b', margin: '0.3rem 0' }}>
            {details?.durationSeconds ? `${Math.floor(details.durationSeconds / 60)} dk ${details.durationSeconds % 60} sn` : 'Belirtilmedi'}
          </p>
        </div>
      </div>

      {details?.instruction && (
        <div style={{ padding: '1rem', background: '#f0fdf4', border: '1px solid #bbf7d0', borderRadius: '8px', color: '#166534', fontSize: '0.9rem' }}>
          <strong>Öğretmen Yönergesi:</strong>
          <p style={{ margin: '0.3rem 0 0' }}>{details.instruction}</p>
        </div>
      )}
    </div>
  );
}

export function CanonicalExamWorkspacePage() {
  const { projectId: contextProjectId, projectPath, isResolving } = useProjectContext();
  const params = useParams<{ projectId?: string; assessmentActivityId?: string; activityId?: string; step?: string }>();
  const [searchParams, setSearchParams] = useSearchParams();

  const projectId = params.projectId || contextProjectId;
  const activityId = params.assessmentActivityId || params.activityId || searchParams.get('assessmentActivityId') || '';
  const routeStep = params.step;

  const [selectedClassAppId, setSelectedClassAppId] = useState<string>(
    searchParams.get('classApplicationId') || '',
  );

  const classesQuery = useQuery({
    queryKey: ['school-classes', projectId, 'all'],
    queryFn: () => commands.listSchoolClasses({ projectId, includeArchived: true }),
    enabled: !!projectId,
  });

  const activityQuery = useQuery({
    queryKey: ['assessment-activity', projectId, activityId],
    queryFn: () => commands.getAssessmentActivity({ projectId, activityId }),
    enabled: !!projectId && !!activityId,
  });

  const workflowQuery = useQuery({
    queryKey: ['workflow-snapshot', projectId],
    queryFn: () => commands.getWorkflowSnapshot(projectId),
    enabled: !!projectId,
  });

  const classes = classesQuery.data ?? EMPTY_CLASSES;
  const classesById = useMemo(
    () => new Map(classes.map((cls) => [cls.id, cls])),
    [classes],
  );

  const activity = activityQuery.data;
  const workflow = workflowQuery.data;

  // Stale class state cleanup helper
  const handleSelectClassApp = (newAppId: string) => {
    setSelectedClassAppId(newAppId);
    const newParams = new URLSearchParams(searchParams);
    if (newAppId) {
      newParams.set('classApplicationId', newAppId);
    } else {
      newParams.delete('classApplicationId');
    }
    // Clean up student/attempt search parameters when switching class scope
    newParams.delete('studentId');
    newParams.delete('submissionId');
    setSearchParams(newParams, { replace: true });
  };

  if (isResolving) {
    return <ProjectContextState pageLabel="Sınav Çalışma Alanı" loading projectPath={projectPath} />;
  }

  if (!projectId || !activityId) {
    return <ProjectContextState pageLabel="Sınav Çalışma Alanı" projectPath={projectPath} />;
  }

  if (activityQuery.isLoading || workflowQuery.isLoading) {
    return <div style={{ padding: '2rem', color: '#64748b' }}>Sınav çalışma alanı yükleniyor…</div>;
  }

  if (!activity) {
    return <div style={{ padding: '2rem', color: '#ef4444' }}>Aranan sınav bulunamadı.</div>;
  }

  const validSteps = getExamStepDefinitions(activity.assessmentType);
  const currentStepDef = validSteps.find((s) => s.id === routeStep) || resolveNextExamStep(activity, workflow, selectedClassAppId);
  const activeStepId = currentStepDef.id;

  const renderStepContent = () => {
    switch (activity.assessmentType) {
      case 'written':
        switch (activeStepId) {
          case 'prep':
            return <ExamPackageWorkspacePage />;
          case 'students':
            return <StudentOperationsWorkspacePage />;
          case 'ocr':
            return <StudentAnswerOcrPage />;
          case 'scoring':
            return <ScoringPage />;
          case 'results':
          default:
            return <AnalysisPage kind="written" />;
        }

      case 'listening':
        switch (activeStepId) {
          case 'listening_content':
            return <ListeningContentStepView projectId={projectId} activityId={activityId} />;
          case 'questions':
            return <ExamPackageWorkspacePage />;
          case 'students':
            return <StudentOperationsWorkspacePage />;
          case 'ocr_scoring':
            return <StudentAnswerOcrPage />;
          case 'results':
          default:
            return <AnalysisPage kind="written" />;
        }

      case 'speaking':
        switch (activeStepId) {
          case 'settings':
          case 'students':
          case 'transcript':
          case 'evaluation':
            return <SpeechExamPage />;
          case 'results':
          default:
            return <AnalysisPage kind="speaking" />;
        }

      case 'performance':
        switch (activeStepId) {
          case 'task':
            return <PerformanceOrganizationPage activityId={activityId} />;
          case 'assessment':
            return <PerformanceScoringPage />;
          case 'results':
          default:
            return <PerformanceResultsView projectId={projectId} activityId={activityId} />;
        }

      default:
        return <ExamPackageWorkspacePage />;
    }
  };

  return (
    <div className="canonical-exam-workspace-page">
      <CanonicalExamWorkspaceHeader
        projectId={projectId}
        activity={activity}
        workflowSnapshot={workflow}
        classesById={classesById}
        activeStepId={activeStepId}
        selectedClassApplicationId={selectedClassAppId}
        onSelectClassApplicationId={handleSelectClassApp}
      />
      <div className="canonical-exam-workspace-page__content">
        {renderStepContent()}
      </div>
    </div>
  );
}

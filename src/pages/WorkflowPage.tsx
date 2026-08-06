import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { commands } from '../api/commands';
import type { AppError } from '../api/errors';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { ProjectContextState } from '../components/common/ProjectContextState';
import { WorkflowPanel } from '../components/workflow/WorkflowPanel';
import { tauriClient } from '../api/tauriClient';
import { useProjectContext } from '../state/useProjectContext';

export function WorkflowPage() {
  const { projectId, projectPath, isResolving } = useProjectContext();
  const queryClient = useQueryClient();

  const { data: project, error: projectError, isLoading: projectLoading, refetch: refetchProject } = useQuery({
    queryKey: ['project-snapshot', projectId],
    queryFn: () => commands.getProjectSnapshot(projectId),
    enabled: !!projectId,
  });

  const { data: workflow, error: workflowError, isLoading: workflowLoading, refetch: refetchWorkflow } = useQuery({
    queryKey: ['workflow-snapshot', projectId],
    queryFn: () => commands.getWorkflowSnapshot(projectId),
    enabled: !!projectId,
  });

  const { data: jobs = [] } = useQuery({
    queryKey: ['jobs', projectId],
    queryFn: () => commands.listJobs(projectId),
    enabled: !!projectId,
  });

  useEffect(() => {
    if (!projectId) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void tauriClient.listenToJobEvents(() => {
      if (cancelled) return;
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['workflow-snapshot', projectId] });
      queryClient.invalidateQueries({ queryKey: ['jobs', projectId] });
    }).then((cleanup) => {
      unlisten = cleanup;
      if (cancelled) {
        cleanup();
      }
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [projectId, queryClient]);

  const error = projectError || workflowError;

  const refresh = async () => {
    await Promise.allSettled([refetchProject(), refetchWorkflow()]);
  };

  if (isResolving) {
    return <ProjectContextState pageLabel="İş akışı" loading projectPath={projectPath} />;
  }

  if (!projectId) {
    return <ProjectContextState pageLabel="İş akışı" projectPath={projectPath} />;
  }

  return (
    <div style={{ padding: '2rem', height: '100%', boxSizing: 'border-box' }}>
      {error && (
        <ErrorBanner
          error={error as unknown as AppError}
          onRefresh={() => void refresh()}
          showTechnicalDetails
        />
      )}

      {(projectLoading || workflowLoading) && <div>Yükleniyor...</div>}

      {!projectLoading && !workflowLoading && !error && (
        <WorkflowPanel workflow={workflow} project={project} jobs={jobs} />
      )}
    </div>
  );
}

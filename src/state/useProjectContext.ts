import { useLocation, useParams, useSearchParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { commands } from '../api/commands';
import { getLastProjectId, getLastProjectPath, resolveProjectIdFromProjects } from './projectSession';
import { getProjectIdFromPathname } from '../app/projectRoutes';

export type ProjectContextSnapshot = {
  projectId: string;
  projectPath: string;
  isResolving: boolean;
};

export function useProjectContext(): ProjectContextSnapshot {
  const params = useParams<{ projectId?: string }>();
  const location = useLocation();
  const [searchParams] = useSearchParams();
  const routeProjectId = params.projectId?.trim() || getProjectIdFromPathname(location.pathname);
  const queryProjectId = searchParams.get('projectId')?.trim() || '';
  const queryProjectPath = searchParams.get('projectPath')?.trim() || '';
  const lastProjectId = getLastProjectId();
  const lastProjectPath = getLastProjectPath();

  const directProjectId = routeProjectId || queryProjectId || lastProjectId;
  const projectPathCandidate = queryProjectPath || lastProjectPath;
  const needsLookup = !directProjectId && !!projectPathCandidate;

  const { data: projectsResult, isLoading: projectsLoading } = useQuery({
    queryKey: ['projects'],
    queryFn: commands.listProjects,
    enabled: needsLookup,
  });

  const resolvedProjectId = directProjectId || resolveProjectIdFromProjects(projectsResult?.projects ?? [], projectPathCandidate);
  const resolvedProjectPath =
    queryProjectPath ||
    lastProjectPath ||
    projectsResult?.projects.find((project) => project.id === resolvedProjectId)?.path ||
    '';

  return {
    projectId: resolvedProjectId,
    projectPath: resolvedProjectPath,
    isResolving: needsLookup && projectsLoading,
  };
}

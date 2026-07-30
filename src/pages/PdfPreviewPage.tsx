import { Navigate, useLocation } from 'react-router-dom';
import { ProjectContextState } from '../components/common/ProjectContextState';
import { projectDocumentsPath } from '../app/projectRoutes';
import { useProjectContext } from '../state/useProjectContext';

export function PdfPreviewPage() {
  const { projectId, projectPath, isResolving } = useProjectContext();
  const location = useLocation();

  if (isResolving) {
    return <ProjectContextState pageLabel="Belgeler" loading projectPath={projectPath} />;
  }
  if (!projectId) {
    return <ProjectContextState pageLabel="Belgeler" projectPath={projectPath} />;
  }

  return <Navigate to={projectDocumentsPath(projectId, location.search)} replace />;
}

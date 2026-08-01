import { useEffect, useRef, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { commands } from '../api/commands';
import { projectOverviewPath } from './projectRoutes';
import {
  getLastProjectId,
  getLastProjectPath,
  selectStartupProject,
  setActiveProject,
} from '../state/projectSession';

export function StartupRedirect() {
  const navigate = useNavigate();
  const startedRef = useRef(false);
  const [message, setMessage] = useState('Son yazılı sınav projesi açılıyor…');
  const projectsQuery = useQuery({
    queryKey: ['projects'],
    queryFn: commands.listProjects,
  });

  useEffect(() => {
    if (startedRef.current || !projectsQuery.data) return;
    startedRef.current = true;
    const projects = projectsQuery.data.projects;
    if (projects.length === 0) {
      navigate('/projects/new', { replace: true });
      return;
    }
    const lastProjectId = getLastProjectId();
    const lastProjectPath = getLastProjectPath();
    const target = selectStartupProject(projects, lastProjectId, lastProjectPath);
    if (!target) {
      navigate('/projects/new', { replace: true });
      return;
    }
    void commands
      .openProject({ projectPath: target.path })
      .then((result) => {
        setActiveProject(result.project.id, result.projectPath);
        navigate(projectOverviewPath(result.project.id), { replace: true });
      })
      .catch((error: unknown) => {
        const reason =
          typeof error === 'object' && error !== null && 'safeMessage' in error
            ? String((error as { safeMessage: unknown }).safeMessage)
            : 'Proje açılamadı.';
        setMessage(reason);
      });
  }, [navigate, projectsQuery.data]);

  if (projectsQuery.error) {
    return <div style={{ padding: '2rem' }}>Projeler okunamadı. Yeni bir proje oluşturabilirsiniz.</div>;
  }
  return <div style={{ padding: '2rem', color: '#64748b' }}>{message}</div>;
}

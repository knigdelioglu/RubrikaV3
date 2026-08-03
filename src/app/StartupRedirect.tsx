import { useCallback, useEffect, useRef, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { commands } from '../api/commands';
import { isProjectMigrationRequiredError, type AppError } from '../api/errors';
import type { ProjectListItem } from '../api/types';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { projectOverviewPath } from './projectRoutes';
import {
  getLastProjectId,
  getLastProjectPath,
  selectStartupProject,
  setActiveProject,
} from '../state/projectSession';

function toStartupError(error: unknown): AppError {
  if (typeof error === 'object' && error !== null) {
    const candidate = error as Partial<AppError>;
    if (typeof candidate.code === 'string' && typeof candidate.safeMessage === 'string') {
      return {
        code: candidate.code as AppError['code'],
        safeMessage: candidate.safeMessage,
        recoveryAction: typeof candidate.recoveryAction === 'string' ? candidate.recoveryAction : undefined,
        correlationId: typeof candidate.correlationId === 'string' ? candidate.correlationId : 'unknown',
        retryable: candidate.retryable === true,
        detailsAvailable: candidate.detailsAvailable === true,
      };
    }
  }

  return {
    code: 'UNKNOWN_ERROR',
    safeMessage: 'Proje açılamadı.',
    recoveryAction: 'Tekrar deneyin veya yeni bir proje oluşturun.',
    correlationId: 'unknown',
    retryable: true,
    detailsAvailable: false,
  };
}

export function StartupRedirect() {
  const navigate = useNavigate();
  const startedRef = useRef(false);
  const [message, setMessage] = useState('Son yazılı sınav projesi açılıyor…');
  const [selectedProject, setSelectedProject] = useState<ProjectListItem | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [isMigrating, setIsMigrating] = useState(false);
  const projectsQuery = useQuery({
    queryKey: ['projects'],
    queryFn: commands.listProjects,
  });

  const openProject = useCallback(async (project: ProjectListItem) => {
    setSelectedProject(project);
    setError(null);
    setMessage(`${project.name} açılıyor…`);
    try {
      const result = await commands.openProject({ projectPath: project.path });
      setActiveProject(result.project.id, result.projectPath);
      navigate(projectOverviewPath(result.project.id), { replace: true });
    } catch (openError: unknown) {
      setError(toStartupError(openError));
      setMessage('Proje açılışı için bir işlem gerekiyor.');
    }
  }, [navigate]);

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
    void openProject(target);
  }, [navigate, openProject, projectsQuery.data]);

  const migrateProject = async () => {
    if (!selectedProject || isMigrating) return;
    setIsMigrating(true);
    setError(null);
    setMessage('Önce doğrulanmış yedek oluşturuluyor…');
    try {
      const result = await commands.migrateProjectWithVerifiedBackup(selectedProject.path);
      setActiveProject(result.project.id, result.projectPath);
      navigate(projectOverviewPath(result.project.id), { replace: true });
    } catch (migrationError: unknown) {
      setError(toStartupError(migrationError));
      setMessage('Proje güncellenemedi.');
    } finally {
      setIsMigrating(false);
    }
  };

  if (projectsQuery.error) {
    return (
      <div className="startup-page">
        <section className="startup-card" aria-labelledby="startup-error-title">
          <p className="startup-card__eyebrow">Rubrika V3</p>
          <h1 id="startup-error-title">Projeler okunamadı</h1>
          <p>Yeni bir proje oluşturabilir veya uygulamayı yeniden başlatıp tekrar deneyebilirsiniz.</p>
          <div className="startup-card__actions">
            <button type="button" className="button button--primary" onClick={() => navigate('/projects/new')}>
              Yeni proje oluştur
            </button>
          </div>
        </section>
      </div>
    );
  }

  if (error) {
    const migrationRequired = isProjectMigrationRequiredError(error);
    return (
      <div className="startup-page">
        <section className="startup-card" aria-labelledby="startup-action-title">
          <p className="startup-card__eyebrow">Rubrika V3</p>
          <h1 id="startup-action-title">
            {migrationRequired ? 'Proje güncellemesi gerekiyor' : 'Proje açılamadı'}
          </h1>
          <p>
            {migrationRequired
              ? 'Devam etmek için proje yeni veri biçimine geçirilecek. İşlemden önce bağımsız doğrulanmış yedek alınır.'
              : 'Açılış tamamlanamadı. Aşağıdaki işlemlerden biriyle devam edebilirsiniz.'}
          </p>
          <div className="startup-card__error">
            <ErrorBanner error={error} showTechnicalDetails />
          </div>
          <div className="startup-card__actions">
            {migrationRequired && selectedProject && (
              <button
                type="button"
                className="button button--primary"
                onClick={() => void migrateProject()}
                disabled={isMigrating}
              >
                {isMigrating ? 'Yedek oluşturuluyor…' : 'Güvenli yedeği al ve projeyi aç'}
              </button>
            )}
            {selectedProject && (
              <button
                type="button"
                className="button button--secondary"
                onClick={() => void openProject(selectedProject)}
                disabled={isMigrating}
              >
                Yeniden dene
              </button>
            )}
            <button type="button" className="button button--secondary" onClick={() => navigate('/projects/new')} disabled={isMigrating}>
              Yeni proje oluştur
            </button>
          </div>
        </section>
      </div>
    );
  }

  return (
    <div className="startup-page" role="status" aria-live="polite">
      <section className="startup-card startup-card--loading" aria-labelledby="startup-loading-title">
        <p className="startup-card__eyebrow">Rubrika V3</p>
        <h1 id="startup-loading-title">{message}</h1>
        <p>Proje durumu doğrulanıyor…</p>
      </section>
    </div>
  );
}

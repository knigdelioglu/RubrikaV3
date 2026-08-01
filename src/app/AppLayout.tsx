import { useEffect, useState, type ReactNode } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link, useLocation, useNavigate } from 'react-router-dom';
import { BarChart3, Bell, BriefcaseBusiness, ChevronDown, ClipboardList, FolderOpen, Home, Menu, Settings, Users, X } from 'lucide-react';
import { commands } from '../api/commands';
import { tauriClient } from '../api/tauriClient';
import { useProjectContext } from '../state/useProjectContext';
import { getActiveJobs, getFailedJobs, getPartialJobs, getJobCenterButtonLabel, getJobLabel, getJobProgressPercent } from './globalJobs';
import { getProjectArea, getProjectIdFromPathname, projectNavigation } from './projectRoutes';
import { shouldShowProjectNavigation } from './assessmentMode';
import type { JobSnapshot, ProjectListItem } from '../api/types';
import { setActiveProject } from '../state/projectSession';
import { projectOverviewPath } from './projectRoutes';

const navIcons: Record<string, ReactNode> = {
  overview: <Home size={18} aria-hidden="true" />,
  activities: <ClipboardList size={18} aria-hidden="true" />,
  classes: <Users size={18} aria-hidden="true" />,
  analysis: <BarChart3 size={18} aria-hidden="true" />,
  settings: <Settings size={18} aria-hidden="true" />,
};

function ProjectSwitcher({ projectId }: { projectId: string }) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const projectsQuery = useQuery({
    queryKey: ['projects'],
    queryFn: commands.listProjects,
  });
  const openMutation = useMutation({
    mutationFn: async (project: ProjectListItem) => {
      const result = await commands.openProject({ projectPath: project.path });
      setActiveProject(result.project.id, result.projectPath);
      return result;
    },
    onSuccess: async (result) => {
      await queryClient.invalidateQueries();
      navigate(projectOverviewPath(result.project.id));
    },
  });

  return (
    <label className="project-switcher" title="Ders alanını değiştir">
      <FolderOpen size={16} aria-hidden="true" />
      <span className="sr-only">Ders alanı seç</span>
      <select
        value={projectId}
        disabled={openMutation.isPending}
        onChange={(event) => {
          const project = projectsQuery.data?.projects.find((item) => item.id === event.target.value);
          if (project && project.id !== projectId) openMutation.mutate(project);
        }}
      >
        {projectsQuery.data?.projects.map((project) => (
          <option key={project.id} value={project.id}>{project.name}</option>
        ))}
      </select>
    </label>
  );
}

function GlobalJobCenter({ projectId }: { projectId: string }) {
  const [open, setOpen] = useState(false);
  const queryClient = useQueryClient();
  const { data: jobs = [] } = useQuery({
    queryKey: ['jobs', projectId],
    queryFn: () => commands.listJobs(projectId),
    enabled: !!projectId,
    refetchInterval: (query) => {
      const hasActiveJob = query.state.data?.some((job) => job.status === 'queued' || job.status === 'running');
      return hasActiveJob ? 1000 : false;
    },
  });

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void tauriClient.listenToJobEvents(() => {
      if (!cancelled) queryClient.invalidateQueries({ queryKey: ['jobs', projectId] });
    }).then((cleanup) => {
      unlisten = cleanup;
      if (cancelled) cleanup();
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [projectId, queryClient]);

  const activeJobs = getActiveJobs(jobs);
  const failedJobs = getFailedJobs(jobs);
  const partialJobs = getPartialJobs(jobs);
  const visibleJobs = [...activeJobs, ...partialJobs, ...failedJobs.slice(0, 3)];
  const buttonLabel = getJobCenterButtonLabel(jobs);

  const handleCancel = async (jobId: string) => {
    try {
      await commands.cancelJob(jobId);
      queryClient.invalidateQueries({ queryKey: ['jobs', projectId] });
    } catch {
      // Error handled by commands error handler
    }
  };

  const handleRetry = async (jobId: string) => {
    try {
      await commands.retryJob(jobId);
      queryClient.invalidateQueries({ queryKey: ['jobs', projectId] });
    } catch {
      // Error handled by commands error handler
    }
  };

  const renderStatusBadge = (job: JobSnapshot) => {
    if (job.cancellationRequested) {
      return <span className="job-status-badge cancel-req">İptal ediliyor...</span>;
    }
    switch (job.status) {
      case 'queued':
        return <span className="job-status-badge">Sırada</span>;
      case 'running':
        return <span className="job-status-badge active">% {getJobProgressPercent(job)}</span>;
      case 'partial':
        return <span className="job-status-badge partial">Kısmi</span>;
      case 'failed':
        return <span className="job-status-badge failed">Başarısız</span>;
      case 'cancelled':
        return <span className="job-status-badge cancelled">İptal edildi</span>;
      case 'interrupted':
        return <span className="job-status-badge interrupted">Yarıda kaldı</span>;
      default:
        return null;
    }
  };

  return (
    <div className="job-center">
      <button
        type="button"
        className={`project-header__job-button ${failedJobs.length ? 'has-error' : ''}`}
        onClick={() => setOpen((current) => !current)}
        aria-expanded={open}
        aria-controls="global-job-panel"
      >
        <BriefcaseBusiness size={17} aria-hidden="true" />
        <span>{buttonLabel}</span>
        <ChevronDown size={15} aria-hidden="true" />
      </button>
      {open && (
        <section id="global-job-panel" className="job-center__panel" aria-label="İşlem merkezi">
          <div className="job-center__heading">
            <div>
              <strong>İşlem merkezi</strong>
              <span>Arka plandaki işlemler bu sayfadan bağımsız devam eder.</span>
            </div>
            <button type="button" className="icon-button" onClick={() => setOpen(false)} aria-label="İşlem merkezini kapat">
              <X size={18} />
            </button>
          </div>
          {visibleJobs.length === 0 ? (
            <p className="job-center__empty">Şu anda devam eden veya kontrol bekleyen işlem yok.</p>
          ) : (
            <div className="job-center__list">
              {visibleJobs.map((job) => {
                const percent = getJobProgressPercent(job);
                const isCancellable = (job.status === 'queued' || job.status === 'running') && job.cancellable !== false;
                const isRetryable = job.status === 'failed' || job.status === 'cancelled' || job.status === 'interrupted';
                return (
                  <article key={job.id} className="job-center__item">
                    <div className="job-center__item-title">
                      <strong>{job.displayLabel || getJobLabel(job.kind)}</strong>
                      {renderStatusBadge(job)}
                    </div>
                    {job.status === 'failed' || job.status === 'interrupted' || job.status === 'cancelled' ? (
                      <p className="job-center__error">{job.error?.safeMessage ?? job.lastMessage ?? (job.status === 'cancelled' ? 'İşlem iptal edildi.' : 'İşlem tamamlanamadı.')}</p>
                    ) : (
                      <>
                        <div className="job-center__track" aria-label={`İlerleme: yüzde ${percent}`}>
                          <span style={{ width: `${percent}%` }} />
                        </div>
                        <p>{job.cancellationRequested ? 'İşlem güvenli bir durma noktasında sonlandırılıyor...' : (job.progress.message || 'İşlem devam ediyor.')}</p>
                      </>
                    )}
                    <div className="job-center__item-actions">
                      {isCancellable && !job.cancellationRequested && (
                        <button
                          type="button"
                          className="button button--secondary button--small"
                          onClick={() => handleCancel(job.id)}
                        >
                          İptal et
                        </button>
                      )}
                      {isRetryable && (
                        <button
                          type="button"
                          className="button button--secondary button--small"
                          onClick={() => handleRetry(job.id)}
                        >
                          Yeniden Dene
                        </button>
                      )}
                    </div>
                  </article>
                );
              })}
            </div>
          )}
        </section>
      )}
    </div>
  );
}

export function AppLayout({ children }: { children: ReactNode }) {
  const location = useLocation();
  const { projectId: contextProjectId } = useProjectContext();
  const routeProjectId = getProjectIdFromPathname(location.pathname);
  const projectId = routeProjectId || contextProjectId;
  const [navigationOpen, setNavigationOpen] = useState(false);
  const isGlobalPage = location.pathname === '/'
    || location.pathname === '/projects'
    || location.pathname === '/project-create'
    || location.pathname === '/projects/new';

  const { data: project } = useQuery({
    queryKey: ['project-snapshot', projectId],
    queryFn: () => commands.getProjectSnapshot(projectId),
    enabled: !isGlobalPage && !!projectId,
  });

  if (isGlobalPage) return <>{children}</>;

  const activeArea = getProjectArea(location.pathname);
  const showProjectNavigation = shouldShowProjectNavigation(location.pathname);
  const updatedAt = project?.updatedAt
    ? new Intl.DateTimeFormat('tr-TR', { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(project.updatedAt))
    : null;

  return (
    <div className="app-shell">
      {showProjectNavigation && <aside className={`project-navigation ${navigationOpen ? 'is-open' : ''}`} aria-label="Ders alanı menüsü">
        <div className="project-navigation__brand">
          <Link to="/projects">Rubrika<span>V3</span></Link>
          <button type="button" className="icon-button project-navigation__close" onClick={() => setNavigationOpen(false)} aria-label="Menüyü kapat">
            <X size={20} />
          </button>
        </div>
        {projectId ? (
          <nav className="project-navigation__items">
            {projectNavigation.map((item) => {
              const active = activeArea === item.area;
              return (
                <Link
                  key={item.area}
                  to={item.path(projectId)}
                  className={`project-navigation__item ${active ? 'is-active' : ''} ${item.area === 'settings' ? 'is-settings' : ''}`}
                  onClick={() => setNavigationOpen(false)}
                  aria-current={active ? 'page' : undefined}
                >
                  <span className="project-navigation__icon">{navIcons[item.area] ?? <Settings size={18} />}</span>
                  <span>
                    <strong>{item.label}</strong>
                    <small>{item.description}</small>
                  </span>
                </Link>
              );
            })}
          </nav>
        ) : (
          <p className="project-navigation__empty">Menüyü görmek için bir ders alanı açın.</p>
        )}
      </aside>}

      {showProjectNavigation && navigationOpen && <button type="button" className="navigation-scrim" onClick={() => setNavigationOpen(false)} aria-label="Menüyü kapat" />}

      <div className="project-workspace">
        <header className="project-header">
          <div className="project-header__identity">
            {showProjectNavigation && <button type="button" className="icon-button project-header__menu" onClick={() => setNavigationOpen(true)} aria-label="Ders alanı menüsünü aç">
              <Menu size={21} />
            </button>}
            <div>
              <h1>{project?.name ?? 'Ders alanı yükleniyor…'}</h1>
              <p>
                {updatedAt ? `Son güncelleme ${updatedAt}` : 'Ders Alanı'}
              </p>
            </div>
          </div>
          <div className="project-header__actions">
            {projectId && <ProjectSwitcher projectId={projectId} />}
            {projectId && <GlobalJobCenter projectId={projectId} />}
            <button type="button" className="icon-button" aria-label="Bildirimler" title="Bildirimler">
              <Bell size={19} />
            </button>
                      </div>
        </header>
        <main className="project-content">{children}</main>
      </div>
    </div>
  );
}

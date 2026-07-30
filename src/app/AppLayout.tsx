import { useEffect, useState, type ReactNode } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link, useLocation, useNavigate } from 'react-router-dom';
import { Bell, BriefcaseBusiness, ChevronDown, FolderOpen, Menu, Settings, X } from 'lucide-react';
import { commands } from '../api/commands';
import { tauriClient } from '../api/tauriClient';
import { useProjectContext } from '../state/useProjectContext';
import { getActiveJobs, getFailedJobs, getJobCenterButtonLabel, getJobLabel, getJobProgressPercent } from './globalJobs';
import { getProjectArea, getProjectIdFromPathname, projectNavigation } from './projectRoutes';
import { AssessmentModeSelector } from '../components/common/AssessmentModeSelector';
import { shouldShowProjectNavigation } from './assessmentMode';
import type { ProjectListItem } from '../api/types';
import { setActiveProject } from '../state/projectSession';
import { projectOverviewPath } from './projectRoutes';

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
    <label className="project-switcher" title="Yazılı sınav projesini değiştir">
      <FolderOpen size={16} aria-hidden="true" />
      <span className="sr-only">Proje seç</span>
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
  const visibleJobs = [...activeJobs, ...failedJobs.slice(0, 3)];
  const buttonLabel = getJobCenterButtonLabel(jobs);

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
                return (
                  <article key={job.id} className="job-center__item">
                    <div className="job-center__item-title">
                      <strong>{getJobLabel(job.kind)}</strong>
                      <span>{job.status === 'failed' ? 'Başarısız' : `%${percent}`}</span>
                    </div>
                    {job.status === 'failed' ? (
                      <p className="job-center__error">{job.error?.message ?? 'İşlem tamamlanamadı.'}</p>
                    ) : (
                      <>
                        <div className="job-center__track" aria-label={`İlerleme: yüzde ${percent}`}>
                          <span style={{ width: `${percent}%` }} />
                        </div>
                        <p>{job.progress.message || 'İşlem devam ediyor.'}</p>
                      </>
                    )}
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
  const { data: workflow } = useQuery({
    queryKey: ['workflow-snapshot', projectId],
    queryFn: () => commands.getWorkflowSnapshot(projectId),
    enabled: !isGlobalPage && !!projectId,
  });

  if (isGlobalPage) return <>{children}</>;

  const activeArea = getProjectArea(location.pathname);
  const showProjectNavigation = shouldShowProjectNavigation(location.pathname);
  const updatedAt = project?.updatedAt
    ? new Intl.DateTimeFormat('tr-TR', { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(project.updatedAt))
    : null;

  return (
    <div className={`app-shell ${showProjectNavigation ? '' : 'app-shell--speaking'}`}>
      {showProjectNavigation && <aside className={`project-navigation ${navigationOpen ? 'is-open' : ''}`} aria-label="Proje bölümleri">
        <div className="project-navigation__brand">
          <Link to="/projects">Rubrika<span>V3</span></Link>
          <button type="button" className="icon-button project-navigation__close" onClick={() => setNavigationOpen(false)} aria-label="Menüyü kapat">
            <X size={20} />
          </button>
        </div>
        {projectId ? (
          <nav className="project-navigation__items">
            {projectNavigation.map((item, index) => {
              const active = activeArea === item.area;
              return (
                <Link
                  key={item.area}
                  to={item.path(projectId)}
                  className={`project-navigation__item ${active ? 'is-active' : ''} ${item.area === 'settings' ? 'is-settings' : ''}`}
                  onClick={() => setNavigationOpen(false)}
                  aria-current={active ? 'page' : undefined}
                >
                  <span className="project-navigation__number">{item.area === 'settings' ? <Settings size={16} /> : index + 1}</span>
                  <span>
                    <strong>{item.label}</strong>
                    <small>{item.description}</small>
                  </span>
                </Link>
              );
            })}
          </nav>
        ) : (
          <p className="project-navigation__empty">Proje menüsünü görmek için bir proje açın.</p>
        )}
      </aside>}

      {showProjectNavigation && navigationOpen && <button type="button" className="navigation-scrim" onClick={() => setNavigationOpen(false)} aria-label="Menüyü kapat" />}

      <div className="project-workspace">
        <header className="project-header">
          <div className="project-header__identity">
            {showProjectNavigation && <button type="button" className="icon-button project-header__menu" onClick={() => setNavigationOpen(true)} aria-label="Proje menüsünü aç">
              <Menu size={21} />
            </button>}
            <div>
              <h1>{project?.name ?? 'Proje yükleniyor…'}</h1>
              <p>
                {project ? `${project.students?.length ?? 0} öğrenci · ${project.questions?.length ?? 0} soru` : 'Proje bilgileri hazırlanıyor'}
                {updatedAt ? ` · Son güncelleme ${updatedAt}` : ''}
              </p>
            </div>
          </div>
          <div className="project-header__actions">
            <AssessmentModeSelector />
            {projectId && <ProjectSwitcher projectId={projectId} />}
            {projectId && <GlobalJobCenter projectId={projectId} />}
            <button type="button" className="icon-button" aria-label="Bildirimler" title="Bildirimler">
              <Bell size={19} />
            </button>
            {workflow && <span className="project-header__stage">{workflow.currentStageLabel}</span>}
          </div>
        </header>
        <main className="project-content">{children}</main>
      </div>
    </div>
  );
}

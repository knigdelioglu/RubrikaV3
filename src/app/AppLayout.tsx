import { useEffect, useRef, useState, type ReactNode } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link, useLocation, useNavigate } from 'react-router-dom';
import { BarChart3, Bell, BriefcaseBusiness, ChevronDown, ClipboardList, FolderOpen, Home, Menu, Settings, Users, X } from 'lucide-react';
import { commands } from '../api/commands';
import { tauriClient } from '../api/tauriClient';
import { useProjectContext } from '../state/useProjectContext';
import { getActiveJobs, getFailedJobs, getPartialJobs, getJobCenterButtonLabel, getJobLabel, getJobProgressPercent } from './globalJobs';
import { getProjectArea, getProjectIdFromPathname, projectNavigation } from './projectRoutes';
import { shouldShowProjectNavigation } from './assessmentMode';
import type { AssessmentActivity, DataLossPreflightReport, JobSnapshot, ProjectListItem } from '../api/types';
import { setActiveProject } from '../state/projectSession';
import { projectOverviewPath } from './projectRoutes';
import { isProjectWriteBlocked } from './projectSafety';
import { formatDateTime } from '../utils/formatting';
import {
  formatAssessmentOption,
  getAssessmentActivityIdFromLocation,
  getProjectSwitcherContextLabel,
  projectActivityPath,
} from './projectSwitcher';

const navIcons: Record<string, ReactNode> = {
  overview: <Home size={18} aria-hidden="true" />,
  activities: <ClipboardList size={18} aria-hidden="true" />,
  classes: <Users size={18} aria-hidden="true" />,
  analysis: <BarChart3 size={18} aria-hidden="true" />,
  settings: <Settings size={18} aria-hidden="true" />,
};

function ProjectSwitcher({
  projectId,
  projectName,
  activities,
  activitiesLoading,
}: {
  projectId: string;
  projectName?: string;
  activities: AssessmentActivity[];
  activitiesLoading: boolean;
}) {
  const switcherRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const location = useLocation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const activeActivityId = getAssessmentActivityIdFromLocation(location.pathname, location.search);
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

  useEffect(() => {
    if (!open) return undefined;

    const handlePointerDown = (event: PointerEvent) => {
      if (event.target instanceof Node && !switcherRef.current?.contains(event.target)) setOpen(false);
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false);
    };
    document.addEventListener('pointerdown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('pointerdown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [open]);

  const selectedProjectName = projectName ?? projectsQuery.data?.projects.find((project) => project.id === projectId)?.name ?? 'Ders alanı';
  const contextLabel = getProjectSwitcherContextLabel(activities, activeActivityId, activitiesLoading);

  const selectActivity = (activity: AssessmentActivity) => {
    setOpen(false);
    navigate(projectActivityPath(projectId, activity));
  };

  return (
    <div ref={switcherRef} className="project-switcher">
      <button
        type="button"
        className="project-switcher__trigger"
        onClick={() => setOpen((current) => !current)}
        disabled={openMutation.isPending}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls="project-switcher-panel"
        title={`${selectedProjectName} · ${contextLabel}`}
      >
        <FolderOpen size={16} aria-hidden="true" />
        <span className="project-switcher__trigger-text">
          <strong>{selectedProjectName}</strong>
          <small>{contextLabel}</small>
        </span>
        <ChevronDown size={15} aria-hidden="true" />
      </button>
      {open && (
        <section id="project-switcher-panel" className="project-switcher__panel" role="dialog" aria-label="Ders alanı ve sınav seçimi">
          <div className="project-switcher__heading">
            <div>
              <strong>Ders alanı ve sınav seç</strong>
              <span>Dönem ve sınav sırası burada açıkça görünür.</span>
            </div>
            <button type="button" className="icon-button" onClick={() => setOpen(false)} aria-label="Seçiciyi kapat">
              <X size={18} />
            </button>
          </div>

          <div className="project-switcher__section">
            <span className="project-switcher__section-label">Ders alanları</span>
            <div className="project-switcher__list">
              {(projectsQuery.data?.projects ?? []).map((project) => (
                <button
                  key={project.id}
                  type="button"
                  className={`project-switcher__item ${project.id === projectId ? 'is-selected' : ''}`}
                  onClick={() => {
                    if (project.id === projectId) {
                      setOpen(false);
                      return;
                    }
                    openMutation.mutate(project);
                  }}
                  disabled={openMutation.isPending}
                >
                  <span>
                    <strong>{project.name}</strong>
                    <small>{project.id === projectId ? 'Açık ders alanı' : 'Ders alanını aç'}</small>
                  </span>
                  {project.id === projectId && <span className="project-switcher__check" aria-label="Seçili">✓</span>}
                </button>
              ))}
              {projectsQuery.isLoading && <p className="project-switcher__empty">Ders alanları yükleniyor…</p>}
              {!projectsQuery.isLoading && (projectsQuery.data?.projects.length ?? 0) === 0 && (
                <p className="project-switcher__empty">Açılabilir başka ders alanı yok.</p>
              )}
            </div>
          </div>

          <div className="project-switcher__section">
            <span className="project-switcher__section-label">Bu ders alanındaki sınavlar</span>
            <div className="project-switcher__list">
              {activitiesLoading && <p className="project-switcher__empty">Sınavlar yükleniyor…</p>}
              {!activitiesLoading && activities.map((activity) => (
                <button
                  key={activity.id}
                  type="button"
                  className={`project-switcher__item ${activity.id === activeActivityId ? 'is-selected' : ''}`}
                  onClick={() => selectActivity(activity)}
                >
                  <span>
                    <strong>{formatAssessmentOption(activity)}</strong>
                    <small>{activity.courseName || activity.title || 'Sınav'}</small>
                  </span>
                  {activity.id === activeActivityId && <span className="project-switcher__check" aria-label="Seçili">✓</span>}
                </button>
              ))}
              {!activitiesLoading && activities.length === 0 && (
                <p className="project-switcher__empty">Bu ders alanında henüz sınav oluşturulmadı.</p>
              )}
            </div>
          </div>

          {openMutation.isError && <p className="project-switcher__error" role="alert">Ders alanı açılamadı. Lütfen tekrar deneyin.</p>}
        </section>
      )}
    </div>
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

function preflightReasonLabel(reason: string): string {
  const labels: Record<string, string> = {
    'verified backup yok': 'Bağımsız doğrulanmış yedek bulunamadı.',
    'failed/unverified backup var': 'Doğrulanamayan bir yedek bulundu.',
    'unknown orphan var': 'Ne olduğu doğrulanamayan artık dosyalar bulundu.',
    'missing referenced artifact var': 'Kayıtlı bir dosya başvurusu eksik.',
    'pending migration var': 'Proje için açıkça onaylanmış göç gerekiyor.',
    'incomplete transaction var': 'Tamamlanmamış bir kayıt işlemi bulundu.',
    'ambiguous transaction var': 'Son kayıt işleminin sonucu kesinleştirilemedi.',
    'audit chain geçersiz': 'İşlem geçmişi doğrulanamadı.',
    'audit/project revision divergence var': 'Proje ve işlem geçmişi aynı revision’da değil.',
    'active audit/project revision divergence var': 'Aktif işlem geçmişi mevcut proje durumu ile eşleşmiyor.',
    'active audit chain invalid': 'Aktif işlem geçmişi güvenli yazma için doğrulanamadı.',
    'verified backup restore doğrulanmadı': 'Yedek alındı; restore eşitliği henüz doğrulanmadı.',
    'process-kill proof failure': 'Ani işlem sonlandırma dayanıklılık kanıtı tamamlanmadı.',
    'disk fault proof failure': 'Disk/izin arızası dayanıklılık kanıtı tamamlanmadı.',
    'destructive race proof failure': 'Eşzamanlı işlem yarış kanıtı tamamlanmadı.',
    'source byte manifest changed': 'Kaynak dosya bütünü doğrulama sırasında değişti.',
    'Kaynak byte manifesti işlem boyunca değişti.': 'Kaynak dosya bütünü doğrulama sırasında değişti.',
    'speaking metadata/audio mismatch var': 'Ses kaydı ile konuşma kaydı eşleşmiyor.',
    'read-only hash guarantee doğrulanmadı': 'Okuma ön kontrolü sırasında dosya bütünü doğrulanamadı.',
    'full validation marker yok': 'Tam doğrulama süiti henüz yeşil olarak işaretlenmedi.',
    'symlink bulundu': 'Proje içinde güvenli olmayan sembolik bağ bulundu.',
    'unsafe import staging var': 'Yarım kalmış içe aktarma bulundu.',
    'unsafe restore staging var': 'Yarım kalmış geri yükleme bulundu.',
    'ikinci writer aktif': 'Proje başka bir yazıcı işlem tarafından kullanılıyor.',
  };
  return labels[reason] ?? 'Veri güvenliği ön koşulu sağlanmadı.';
}

function ProjectSafetyBanner({ report, loading, failed }: {
  report?: DataLossPreflightReport;
  loading: boolean;
  failed: boolean;
}) {
  if (loading) {
    return <div className="project-safety-banner project-safety-banner--pending" role="status">Veri güvenliği ön kontrolü yapılıyor…</div>;
  }
  if (failed || !report) {
    return <div className="project-safety-banner project-safety-banner--blocked" role="alert">
      <strong>Yazma işlemleri koruma amacıyla bekletiliyor.</strong>
      <span>Veri güvenliği ön kontrolü alınamadı. Taslaklarınız korunur; doğrulama tamamlanmadan kaydetme işlemi yapılmaz.</span>
    </div>;
  }
  if (report.initializationWriteAllowed) {
    return <div className="project-safety-banner project-safety-banner--warning" role="status">
      <strong>Yeni ders alanı ilk kuruluma hazır.</strong>
      <span>Bu alan henüz sınav, belge veya öğrenci verisi içermiyor; temel kurulum işlemlerine izin verildi. İlk veri eklendikten sonra normal veri güvenliği kontrolleri uygulanır.</span>
    </div>;
  }
  if (report.decision === 'DO_NOT_OPEN_FOR_WRITING') {
    return <div className="project-safety-banner project-safety-banner--blocked" role="alert">
      <strong>Bu ders alanı yazmaya güvenli açılmadı.</strong>
      <span>Önce doğrulanmış bağımsız yedek alın ve aşağıdaki koşulları giderin. Açık form taslakları silinmez.</span>
      <span>
        {report.verifiedBackupRestoreStatus === 'PASS'
          ? 'Doğrulanmış yedek restore eşitliği: hazır.'
          : 'Doğrulanmış yedek restore eşitliği: doğrulama bekliyor.'}
        {report.verifiedBackupPath ? ' Yedek: ' + report.verifiedBackupPath : ''}
      </span>
      <ul>
        {report.blockers.slice(0, 4).map((reason) => <li key={reason}>{preflightReasonLabel(reason)}</li>)}
      </ul>
    </div>;
  }
  if (report.decision === 'SAFE_TO_OPEN_WITH_BACKUP') {
    return <div className="project-safety-banner project-safety-banner--warning" role="status">
      <strong>Yazma öncesi yedek öneriliyor.</strong>
      <span>Ön kontrol tamamlandı; güvenli çalışma için doğrulanmış yedeği güncel tutun.</span>
    </div>;
  }
  return <div className="project-safety-banner project-safety-banner--safe" role="status"><strong>Veri güvenliği ön kontrolü başarılı.</strong></div>;
}

export function AppLayout({ children }: { children: ReactNode }) {
  const location = useLocation();
  const { projectId: contextProjectId, projectPath } = useProjectContext();
  const routeProjectId = getProjectIdFromPathname(location.pathname);
  const projectId = routeProjectId || contextProjectId;
  const [navigationOpen, setNavigationOpen] = useState(false);
  const isGlobalPage = location.pathname === '/'
    || location.pathname === '/projects'
    || location.pathname === '/project-create'
    || location.pathname === '/projects/new';

  const { data: project, isLoading: projectLoading } = useQuery({
    queryKey: ['project-snapshot', projectId],
    queryFn: () => commands.getProjectSnapshot(projectId),
    enabled: !isGlobalPage && !!projectId,
  });

  const preflightQuery = useQuery({
    queryKey: ['data-loss-preflight', projectPath],
    queryFn: () => commands.getDataLossPreflight(projectPath),
    enabled: !isGlobalPage && !!projectPath,
    staleTime: 5_000,
  });

  if (isGlobalPage) return <>{children}</>;

  const activeArea = getProjectArea(location.pathname);
  const showProjectNavigation = shouldShowProjectNavigation(location.pathname);
  const updatedAt = formatDateTime(project?.updatedAt);

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
            {projectId && (
              <ProjectSwitcher
                projectId={projectId}
                projectName={project?.name}
                activities={project?.assessmentActivities ?? []}
                activitiesLoading={projectLoading}
              />
            )}
            {projectId && <GlobalJobCenter projectId={projectId} />}
            <button type="button" className="icon-button" aria-label="Bildirimler" title="Bildirimler">
              <Bell size={19} />
            </button>
                      </div>
        </header>
        <main
          className="project-content"
          onClickCapture={(event) => {
            const target = event.target;
            if (!(target instanceof Element)) return;
            const button = target.closest('button');
            const blocked = isProjectWriteBlocked(preflightQuery.data, {
              isLoading: preflightQuery.isLoading,
              isError: preflightQuery.isError,
            });
            const isProjectWrite = button
              && button.getAttribute('data-project-write') !== 'false';
            if (isProjectWrite && blocked) {
              event.preventDefault();
              event.stopPropagation();
            }
          }}
        >
          {showProjectNavigation && <ProjectSafetyBanner report={preflightQuery.data} loading={preflightQuery.isLoading} failed={preflightQuery.isError} />}
          {children}
        </main>
      </div>
    </div>
  );
}

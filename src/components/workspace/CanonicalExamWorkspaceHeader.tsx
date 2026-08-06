import { useState, type ReactNode } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import {
  AlertCircle,
  CheckCircle2,
  ChevronRight,
  Clock,
  Filter,
  Lock,
  Mic2,
  FileText,
  Volume2,
} from 'lucide-react';
import type {
  AssessmentActivity,
  PerformanceStatus,
  SchoolClass,
  WorkflowSnapshot,
} from '../../api/types';
import {
  deriveExamStepStatuses,
  getCanonicalWorkspaceStepPath,
  type ExamStepState,
  type ExamStepStatus,
} from '../../app/examWorkspace';
import { assessmentTypeLabels } from '../../pages/assessmentOrganizationUi';

type CanonicalExamWorkspaceHeaderProps = {
  projectId: string;
  activity: AssessmentActivity;
  workflowSnapshot?: WorkflowSnapshot | null;
  performanceStatus?: PerformanceStatus | null;
  classesById: Map<string, SchoolClass>;
  activeStepId: string;
  selectedClassApplicationId: string;
  onSelectClassApplicationId: (id: string) => void;
};

const statusIcons: Record<ExamStepStatus, ReactNode> = {
  not_started: <Clock size={15} aria-hidden="true" />,
  ready: <ChevronRight size={15} aria-hidden="true" />,
  in_progress: <Clock size={15} aria-hidden="true" />,
  needs_review: <AlertCircle size={15} aria-hidden="true" />,
  completed: <CheckCircle2 size={15} aria-hidden="true" />,
  blocked: <Lock size={15} aria-hidden="true" />,
};

function activityStatusLabel(status: string): string {
  switch (status) {
    case 'draft':
      return 'Taslak';
    case 'completed':
      return 'Tamamlandı';
    case 'archived':
      return 'Arşivlendi';
    case 'scheduled':
      return 'Planlandı';
    case 'active':
    default:
      return 'Aktif';
  }
}

export function CanonicalExamWorkspaceHeader({
  projectId,
  activity,
  workflowSnapshot,
  performanceStatus,
  classesById,
  activeStepId,
  selectedClassApplicationId,
  onSelectClassApplicationId,
}: CanonicalExamWorkspaceHeaderProps) {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const [blockedAlert, setBlockedAlert] = useState<string | null>(null);

  const stepStates = deriveExamStepStatuses(
    activity,
    workflowSnapshot,
    selectedClassApplicationId,
    performanceStatus,
  );

  const applications = activity.classApplications.filter(
    (app) => app.status !== 'archived',
  );

  const linkedClassNames = applications
    .map((app) => {
      const cls = classesById.get(app.schoolClassId);
      return cls ? cls.displayName || cls.name : 'Sınıf';
    })
    .join(', ');

  const title =
    activity.title?.trim() ||
    `${activity.term}. Dönem ${activity.sequenceNumber}. ${assessmentTypeLabels[activity.assessmentType]}`;

  const typeBadgeIcon =
    activity.assessmentType === 'speaking' ? (
      <Mic2 size={15} aria-hidden="true" />
    ) : activity.assessmentType === 'listening' ? (
      <Volume2 size={15} aria-hidden="true" />
    ) : (
      <FileText size={15} aria-hidden="true" />
    );

  const handleStepClick = (stepState: ExamStepState) => {
    if (stepState.status === 'blocked') {
      setBlockedAlert(
        stepState.blockerMessage ||
          'Bu adıma geçebilmek için lütfen önceki adımları tamamlayın.',
      );
      return;
    }
    setBlockedAlert(null);
    const destination = getCanonicalWorkspaceStepPath(
      projectId,
      activity.id,
      stepState.definition.id,
      searchParams.toString(),
    );
    navigate(destination);
  };

  return (
    <header className="canonical-exam-workspace-header" aria-label="Sınav Çalışma Alanı">
      <div className="canonical-exam-workspace-header__top">
        <div className="canonical-exam-workspace-header__meta">
          <div className="canonical-exam-workspace-header__title-row">
            <span
              className={`canonical-exam-workspace-header__type-badge canonical-exam-workspace-header__type-badge--${activity.assessmentType}`}
            >
              {typeBadgeIcon}
              <span>{assessmentTypeLabels[activity.assessmentType]}</span>
            </span>
            <h2>{title}</h2>
            <span className="canonical-exam-workspace-header__status-badge">
              {activityStatusLabel(activity.status)}
            </span>
          </div>

          <div className="canonical-exam-workspace-header__subtext">
            <span>
              {activity.gradeLevel}. Sınıf · {activity.term}. Dönem · {activity.sequenceNumber}. Sınav
            </span>
            <span className="dot-separator">•</span>
            <span>{activity.courseName}</span>
            {linkedClassNames && (
              <>
                <span className="dot-separator">•</span>
                <span className="canonical-exam-workspace-header__classes-summary">
                  Bağlı Sınıflar: <strong>{linkedClassNames}</strong>
                </span>
              </>
            )}
          </div>
        </div>

        {applications.length > 0 && (
          <div className="canonical-exam-workspace-header__class-filter">
            <label htmlFor="workspace-class-filter" className="sr-only">
              Sınıf Filtresi
            </label>
            <Filter size={15} aria-hidden="true" className="canonical-exam-workspace-header__filter-icon" />
            <select
              id="workspace-class-filter"
              value={selectedClassApplicationId}
              onChange={(e) => onSelectClassApplicationId(e.target.value)}
              title="Sınıf filtresi (Yalnızca bu sınava ait sınıfları gösterir)"
            >
              <option value="">Tüm Sınıflar ({applications.length})</option>
              {applications.map((app) => {
                const cls = classesById.get(app.schoolClassId);
                const label = cls ? cls.displayName || cls.name : 'Sınıf';
                return (
                  <option key={app.id} value={app.id}>
                    {label} ({app.studentScopeIds.length} öğrenci)
                  </option>
                );
              })}
            </select>
          </div>
        )}
      </div>

      {blockedAlert && (
        <div className="canonical-exam-workspace-header__blocker-banner" role="alert">
          <Lock size={16} aria-hidden="true" />
          <span>{blockedAlert}</span>
          <button
            type="button"
            data-project-write="false"
            className="filter-clear-button"
            onClick={() => setBlockedAlert(null)}
            aria-label="Engeli kapat"
          >
            Kapat
          </button>
        </div>
      )}

      <nav
        className="canonical-exam-workspace-header__step-bar"
        aria-label="Sınav Adımları Navigasyonu"
      >
        <ol className="canonical-exam-workspace-header__step-list">
          {stepStates.map((stepState) => {
            const isActive = activeStepId === stepState.definition.id;
            return (
              <li key={stepState.definition.id} className="canonical-exam-workspace-header__step-item">
                <button
                  type="button"
                  data-project-write="false"
                  onClick={() => handleStepClick(stepState)}
                  className={`canonical-exam-workspace-header__step-button ${
                    isActive ? 'is-active' : ''
                  } is-status-${stepState.status}`}
                  aria-current={isActive ? 'step' : undefined}
                  aria-disabled={stepState.status === 'blocked'}
                  title={`${stepState.definition.index}. ${stepState.definition.label} - Durum: ${stepState.statusLabel}`}
                >
                  <span className="canonical-exam-workspace-header__step-index">
                    {stepState.definition.index}
                  </span>
                  <span className="canonical-exam-workspace-header__step-label">
                    {stepState.definition.label}
                  </span>
                  <span className="canonical-exam-workspace-header__step-status-badge">
                    {statusIcons[stepState.status]}
                    <span className="canonical-exam-workspace-header__step-status-text">
                      {stepState.statusLabel}
                    </span>
                  </span>
                </button>
              </li>
            );
          })}
        </ol>
      </nav>
    </header>
  );
}

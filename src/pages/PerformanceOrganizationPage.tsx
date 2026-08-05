import { useEffect, useMemo, useRef, useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  AlertTriangle,
  CalendarDays,
  CheckCircle2,
  ClipboardCheck,
  History,
  Lock,
  Plus,
  RefreshCw,
  Save,
  Send,
  Users,
  X,
} from 'lucide-react';
import { commands } from '../api/commands';
import type { AppError } from '../api/errors';
import type {
  LevelDescription,
  PerformanceCriterion,
  PerformanceDetails,
  PerformanceLevel,
  PerformanceRubric,
  PerformanceSkillArea,
  PerformanceWorkMode,
  SchoolClass,
  TeachingAssignment,
} from '../api/types';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { LoadingButton } from '../components/common/LoadingButton';
import { ProjectContextState } from '../components/common/ProjectContextState';
import { useProjectContext } from '../state/useProjectContext';
import { formatDate, formatDateTime } from '../utils/formatting';
import {
  PERFORMANCE_EVIDENCE_TYPES,
  PERFORMANCE_TEMPLATES,
  emptyPerformanceDetails,
  performanceLevelTemplates,
  performancePublishedVersions,
  performanceRubricDraft,
  performanceSkillAreaLabels,
  performanceSkillAreaOptions,
  performanceTemplateToRubric,
  performanceWorkModeLabels,
  validatePerformanceRubric,
} from './performanceOrganizationUi';

type PerformanceOrganizationPageProps = {
  activityId?: string;
  createOnly?: boolean;
};

const EMPTY_CLASSES: SchoolClass[] = [];

function classLabel(schoolClass: SchoolClass): string {
  return schoolClass.displayName || schoolClass.name;
}

function freshRubric(title: string): PerformanceRubric {
  return {
    id: crypto.randomUUID(),
    name: `${title.trim() || 'Performans Görevi'} Rubrik`,
    version: 0,
    criteria: [],
    levels: performanceLevelTemplates(5),
    createdAt: '',
  };
}

function RubricTemplateCatalog({
  selectedSkillArea,
  onApply,
}: {
  selectedSkillArea: PerformanceSkillArea;
  onApply: (templateId: string, rubric: PerformanceRubric) => void;
}) {
  const [activeArea, setActiveArea] = useState<'Metin Tahlili' | 'Edebiyat Atölyesi'>('Metin Tahlili');
  const templates = PERFORMANCE_TEMPLATES.filter(
    (template) => template.learningArea === activeArea,
  );
  return (
    <div className="performance-template-catalog">
      <div className="performance-template-catalog__head">
        <span className="performance-rubric-editor__label">
          Hazır TDE şablonları <small>(9. sınıf pilot · salt-okunur katalog)</small>
        </span>
        <div className="performance-template-catalog__tabs" role="tablist" aria-label="Öğrenme alanı">
          {(['Metin Tahlili', 'Edebiyat Atölyesi'] as const).map((area) => (
            <button
              key={area}
              type="button"
              data-project-write="false"
              role="tab"
              aria-selected={activeArea === area}
              className={activeArea === area ? 'is-active' : ''}
              onClick={() => setActiveArea(area)}
            >
              {area}
            </button>
          ))}
        </div>
      </div>
      <div className="performance-template-catalog__grid">
        {templates.map((template) => {
          const matchesSkill = template.skillArea === selectedSkillArea;
          return (
            <button
              key={template.id}
              type="button"
              data-project-write="false"
              className="performance-template-card"
              onClick={() => onApply(template.id, performanceTemplateToRubric(template))}
            >
              <strong>{template.title}</strong>
              <span>{template.description}</span>
              <small>
                {template.criteria.length} ölçüt · 5 düzey ·{' '}
                {performanceSkillAreaLabels[template.skillArea]}
                {!matchesSkill ? ' · seçili beceri alanına otomatik geçilir' : ''}
              </small>
            </button>
          );
        })}
      </div>
      <p className="performance-template-catalog__note">
        Şablon seçildiğinde rubrik taslağına (sürüm 0) yüklenir; zümre kararına göre düzenleyip
        yayınlayabilirsiniz. Şablonlar öğretmen kararını kısıtlamaz.
      </p>
    </div>
  );
}

function reconcileLevelDescriptions(
  criterion: PerformanceCriterion,
  levels: PerformanceLevel[],
): LevelDescription[] {
  const byLevelId = new Map(
    criterion.levelDescriptions.map((entry) => [entry.levelId, entry.description]),
  );
  return levels.map((level) => ({
    levelId: level.id,
    description: byLevelId.get(level.id) ?? '',
  }));
}

function rebuildLevels(draft: PerformanceRubric, levelCount: 3 | 5): PerformanceRubric {
  if (draft.levels.length === levelCount) return draft;
  const levels = performanceLevelTemplates(levelCount);
  return {
    ...draft,
    levels,
    criteria: draft.criteria.map((criterion) => ({
      ...criterion,
      levelDescriptions: reconcileLevelDescriptions(criterion, levels),
    })),
  };
}

function TaskDetailsFields({
  details,
  onChange,
}: {
  details: PerformanceDetails;
  onChange: (details: PerformanceDetails) => void;
}) {
  return (
    <>
      <div className="assessment-form-grid">
        <label className="assessment-form-grid__wide">
          <span>Tema</span>
          <input
            value={details.theme}
            placeholder="Örn: Doğa ve insan"
            onChange={(event) => onChange({ ...details, theme: event.target.value })}
          />
        </label>
        <label>
          <span>Beceri alanı</span>
          <select
            value={details.skillArea}
            onChange={(event) =>
              onChange({ ...details, skillArea: event.target.value as PerformanceSkillArea })
            }
          >
            {performanceSkillAreaOptions.map((area) => (
              <option key={area} value={area}>
                {performanceSkillAreaLabels[area]}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>Çalışma biçimi</span>
          <select
            value={details.workMode}
            onChange={(event) =>
              onChange({ ...details, workMode: event.target.value as PerformanceWorkMode })
            }
          >
            {(['individual', 'group'] as const).map((mode) => (
              <option key={mode} value={mode}>
                {performanceWorkModeLabels[mode]}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>Teslim tarihi <small>(isteğe bağlı)</small></span>
          <input
            type="date"
            value={details.dueDate ?? ''}
            onChange={(event) => onChange({ ...details, dueDate: event.target.value || null })}
          />
        </label>
        <label className="assessment-form-grid__wide">
          <span>Öğrenme çıktıları <small>(her satır bir çıktı)</small></span>
          <textarea
            rows={3}
            value={details.learningOutcomes.join('\n')}
            placeholder={'Örn:\nMetindeki ana düşünceyi belirler.\nSözlü anlatımında sesini etkili kullanır.'}
            onChange={(event) =>
              onChange({
                ...details,
                learningOutcomes: event.target.value
                  .split('\n')
                  .map((line) => line.trim())
                  .filter((line) => line.length > 0),
              })
            }
          />
        </label>
        <label className="assessment-form-grid__wide">
          <span>Görev yönergesi</span>
          <textarea
            rows={4}
            value={details.taskInstruction}
            placeholder="Öğrencilere aktarılacak ortak görev yönergesini yazın."
            onChange={(event) => onChange({ ...details, taskInstruction: event.target.value })}
          />
        </label>
        <div className="assessment-form-grid__wide performance-evidence-types">
          <span className="performance-evidence-types__label">Kanıt türleri</span>
          <div className="performance-evidence-types__options">
            {PERFORMANCE_EVIDENCE_TYPES.map((evidenceType) => {
              const selected = details.evidenceTypes.includes(evidenceType);
              return (
                <label
                  key={evidenceType}
                  className={selected ? 'is-selected' : ''}
                >
                  <input
                    type="checkbox"
                    checked={selected}
                    onChange={(event) =>
                      onChange({
                        ...details,
                        evidenceTypes: event.target.checked
                          ? [...details.evidenceTypes, evidenceType]
                          : details.evidenceTypes.filter((item) => item !== evidenceType),
                      })
                    }
                  />
                  <span>{evidenceType}</span>
                </label>
              );
            })}
          </div>
        </div>
      </div>
    </>
  );
}

function RubricDraftEditor({
  draft,
  onChange,
  disabled,
}: {
  draft: PerformanceRubric;
  onChange: (draft: PerformanceRubric) => void;
  disabled: boolean;
}) {
  const updateCriterion = (index: number, next: PerformanceCriterion) => {
    onChange({
      ...draft,
      criteria: draft.criteria.map((criterion, candidateIndex) =>
        candidateIndex === index ? next : criterion,
      ),
    });
  };

  const addCriterion = () => {
    if (draft.criteria.length >= 6) return;
    onChange({
      ...draft,
      criteria: [
        ...draft.criteria,
        {
          id: crypto.randomUUID(),
          name: '',
          description: '',
          levelDescriptions: draft.levels.map((level) => ({ levelId: level.id, description: '' })),
        },
      ],
    });
  };

  const removeCriterion = (index: number) => {
    if (draft.criteria.length <= 3) return;
    onChange({
      ...draft,
      criteria: draft.criteria.filter((_, candidateIndex) => candidateIndex !== index),
    });
  };

  const updateLevel = (index: number, next: PerformanceLevel) => {
    const levels = draft.levels.map((level, candidateIndex) =>
      candidateIndex === index ? next : level,
    );
    onChange({ ...draft, levels });
  };

  return (
    <div className="performance-rubric-editor">
      <div className="assessment-form-grid">
        <label className="assessment-form-grid__wide">
          <span>Rubrik adı</span>
          <input
            value={draft.name}
            disabled={disabled}
            placeholder="Örn: Yazılı Ürün Rubriği"
            onChange={(event) => onChange({ ...draft, name: event.target.value })}
          />
        </label>
        <label>
          <span>Düzey sayısı</span>
          <select
            value={draft.levels.length === 3 ? '3' : '5'}
            disabled={disabled}
            onChange={(event) =>
              onChange(rebuildLevels(draft, event.target.value === '3' ? 3 : 5))
            }
          >
            <option value="5">5 düzey</option>
            <option value="3">3 düzey</option>
          </select>
        </label>
        <div className="assessment-form-grid__wide" />
      </div>

      <div className="performance-rubric-levels">
        <span className="performance-rubric-editor__label">Düzeyler</span>
        <div className="performance-rubric-levels__grid">
          {draft.levels.map((level, index) => (
            <div key={level.id} className="performance-rubric-level-card">
              <label>
                <span>Ad</span>
                <input
                  value={level.name}
                  disabled={disabled}
                  onChange={(event) =>
                    updateLevel(index, { ...level, name: event.target.value })
                  }
                />
              </label>
              <label>
                <span>Puan</span>
                <input
                  type="number"
                  min={0}
                  value={level.points}
                  disabled={disabled}
                  onChange={(event) =>
                    updateLevel(index, { ...level, points: Number(event.target.value) || 0 })
                  }
                />
              </label>
              <label className="performance-rubric-level-card__description">
                <span>Düzey tanımı</span>
                <textarea
                  rows={2}
                  value={level.description}
                  disabled={disabled}
                  onChange={(event) =>
                    updateLevel(index, { ...level, description: event.target.value })
                  }
                />
              </label>
            </div>
          ))}
        </div>
      </div>

      <div className="performance-rubric-criteria">
        <div className="performance-rubric-criteria__toolbar">
          <span className="performance-rubric-editor__label">
            Ölçütler ({draft.criteria.length}/6)
          </span>
          <button
            type="button"
            data-project-write="false"
            className="filter-clear-button"
            disabled={disabled || draft.criteria.length >= 6}
            onClick={addCriterion}
          >
            <Plus size={15} /> Ölçüt ekle
          </button>
        </div>
        {draft.criteria.length === 0 && (
          <p className="assessment-form-help">Henüz ölçüt eklenmedi. En az 3 ölçüt ekleyin.</p>
        )}
        <div className="performance-rubric-matrix">
          <table>
            <thead>
              <tr>
                <th className="performance-rubric-matrix__criterion-head">Ölçüt</th>
                {draft.levels.map((level) => (
                  <th key={level.id}>
                    {level.name}
                    <small>{level.points} puan</small>
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {draft.criteria.map((criterion, criterionIndex) => (
                <tr key={criterion.id}>
                  <td className="performance-rubric-matrix__criterion">
                    <input
                      value={criterion.name}
                      disabled={disabled}
                      placeholder="Ölçüt adı"
                      onChange={(event) =>
                        updateCriterion(criterionIndex, { ...criterion, name: event.target.value })
                      }
                    />
                    <textarea
                      rows={2}
                      value={criterion.description}
                      disabled={disabled}
                      placeholder="Ölçüt açıklaması"
                      onChange={(event) =>
                        updateCriterion(criterionIndex, {
                          ...criterion,
                          description: event.target.value,
                        })
                      }
                    />
                    <button
                      type="button"
                      data-project-write="false"
                      className="icon-button performance-rubric-matrix__remove"
                      aria-label="Ölçüdü sil"
                      disabled={disabled || draft.criteria.length <= 3}
                      onClick={() => removeCriterion(criterionIndex)}
                    >
                      <X size={14} />
                    </button>
                  </td>
                  {draft.levels.map((level) => {
                    const entry = criterion.levelDescriptions.find(
                      (candidate) => candidate.levelId === level.id,
                    );
                    return (
                      <td key={level.id}>
                        <textarea
                          rows={3}
                          value={entry?.description ?? ''}
                          disabled={disabled}
                          placeholder="Gözlenebilir tanım"
                          onChange={(event) =>
                            updateCriterion(criterionIndex, {
                              ...criterion,
                              levelDescriptions: criterion.levelDescriptions
                                .filter((candidate) => candidate.levelId !== level.id)
                                .concat({ levelId: level.id, description: event.target.value }),
                            })
                          }
                        />
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

function PublishedRubricView({ rubric }: { rubric: PerformanceRubric }) {
  return (
    <article className="performance-rubric-version">
      <div className="performance-rubric-version__head">
        <div>
          <strong>{rubric.name}</strong>
          <span>Yayınlanan sürüm {rubric.version}</span>
        </div>
        <span className="performance-status-badge performance-status-badge--published">
          Yayınlandı
        </span>
      </div>
      <p className="performance-rubric-version__meta">
        {formatDateTime(rubric.createdAt)} · {rubric.levels.length} düzey ·{' '}
        {rubric.criteria.length} ölçüt
      </p>
      <div className="performance-rubric-matrix performance-rubric-matrix--readonly">
        <table>
          <thead>
            <tr>
              <th className="performance-rubric-matrix__criterion-head">Ölçüt</th>
              {rubric.levels.map((level) => (
                <th key={level.id}>
                  {level.name}
                  <small>{level.points} puan</small>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rubric.criteria.map((criterion) => (
              <tr key={criterion.id}>
                <td className="performance-rubric-matrix__criterion">
                  <strong>{criterion.name}</strong>
                  <span>{criterion.description}</span>
                </td>
                {rubric.levels.map((level) => {
                  const entry = criterion.levelDescriptions.find(
                    (candidate) => candidate.levelId === level.id,
                  );
                  return <td key={level.id}>{entry?.description ?? '—'}</td>;
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </article>
  );
}

export function PerformanceOrganizationPage({
  activityId,
  createOnly = false,
}: PerformanceOrganizationPageProps) {
  const { projectId, projectPath, isResolving } = useProjectContext();
  const routeParams = useParams<{ performanceActivityId?: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const resolvedActivityId = activityId || routeParams.performanceActivityId || '';

  // List-mode filters
  const [filterCourseId, setFilterCourseId] = useState('');
  const [filterTerm, setFilterTerm] = useState('');
  const [filterClassId, setFilterClassId] = useState('');

  const isEditor = Boolean(resolvedActivityId) || createOnly;

  const classesQuery = useQuery({
    queryKey: ['school-classes', projectId, 'all'],
    queryFn: () => commands.listSchoolClasses({ projectId, includeArchived: true }),
    enabled: !!projectId,
  });
  const assignmentsQuery = useQuery({
    queryKey: ['teaching-assignments', projectId],
    queryFn: () => commands.listTeachingAssignments({ projectId }),
    enabled: !!projectId,
  });
  const classes = classesQuery.data ?? EMPTY_CLASSES;
  const classesById = useMemo(
    () => new Map(classes.map((schoolClass) => [schoolClass.id, schoolClass])),
    [classes],
  );
  const assignments = useMemo(
    () => (assignmentsQuery.data ?? []).filter((assignment) => assignment.isActive),
    [assignmentsQuery.data],
  );
  const courseOptions = useMemo(() => {
    const options = new Map<string, { key: string; id: string; name: string; year: string }>();
    for (const assignment of assignments) {
      const key = `${assignment.academicYearId}:${assignment.courseId}`;
      if (!options.has(key)) {
        options.set(key, {
          key,
          id: assignment.courseId,
          name: assignment.courseName,
          year: assignment.academicYearId,
        });
      }
    }
    return [...options.values()].sort((left, right) =>
      left.name.localeCompare(right.name, 'tr'),
    );
  }, [assignments]);

  const tasksQuery = useQuery({
    queryKey: [
      'performance-tasks',
      projectId,
      filterCourseId,
      filterTerm,
      filterClassId,
    ],
    queryFn: () =>
      commands.listPerformanceTasks({
        projectId,
        courseId: filterCourseId || undefined,
        term: filterTerm ? Number(filterTerm) : undefined,
        schoolClassId: filterClassId || undefined,
      }),
    enabled: !!projectId && !isEditor,
  });
  const tasks = tasksQuery.data ?? [];

  const refreshProjectData = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ['performance-tasks', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['assessment-activity', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['performance-rubric-history', projectId] }),
      queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] }),
    ]);
  };

  if (isResolving) {
    return <ProjectContextState pageLabel="Performans Görevleri" loading projectPath={projectPath} />;
  }
  if (!projectId) {
    return <ProjectContextState pageLabel="Performans Görevleri" projectPath={projectPath} />;
  }

  if (createOnly) {
    return (
      <PerformanceCreateEditor
        projectId={projectId}
        classes={classes}
        assignments={assignments}
        courseOptions={courseOptions}
      />
    );
  }

  if (activityId || resolvedActivityId) {
    return (
      <PerformanceEditEditor
        projectId={projectId}
        activityId={resolvedActivityId}
        classesById={classesById}
        refreshProjectData={refreshProjectData}
      />
    );
  }

  return (
    <div className="performance-page">
      <header className="performance-page__header">
        <div>
          <h2>Performans Görevleri</h2>
          <p>Tema, öğrenme çıktıları ve rubrik ile görevleri organize edin.</p>
        </div>
        <div className="performance-page__header-actions">
          <button
            type="button"
            data-project-write="false"
            className="button button--secondary"
            onClick={() => void refreshProjectData()}
          >
            <RefreshCw size={15} /> Yenile
          </button>
          <button
            type="button"
            data-project-write="false"
            className="button button--primary"
            onClick={() => navigate(`/project/${encodeURIComponent(projectId)}/performance/new`)}
          >
            <Plus size={17} /> Yeni görev oluştur
          </button>
        </div>
      </header>

      {assignments.length === 0 && (
        <section className="assessment-setup-blocker" aria-label="Kurulum eksik">
          <div>
            <strong>Kurulum eksik</strong>
            <p>Performans görevi oluşturabilmek için önce ders–sınıf görevlendirmelerini tamamlayın.</p>
          </div>
          <Link className="button button--secondary" to={`/project/${encodeURIComponent(projectId)}/classes`}>
            Kuruluma git
          </Link>
        </section>
      )}

      <section className="assessment-list-section" aria-labelledby="performance-task-list-heading">
        <div className="assessment-toolbar">
          <div>
            <h3 id="performance-task-list-heading">Görevler</h3>
            <span>Her performans görevi, rubrik sürümleriyle birlikte tek kartta görünür.</span>
          </div>
        </div>
        <div className="assessment-filter-toolbar" aria-label="Performans görevi filtreleri">
          {courseOptions.length > 1 && (
            <select
              aria-label="Ders filtresi"
              value={filterCourseId}
              onChange={(event) => setFilterCourseId(event.target.value)}
            >
              <option value="">Ders</option>
              {courseOptions.map((option) => (
                <option key={option.key} value={option.id}>
                  {option.name}
                </option>
              ))}
            </select>
          )}
          <select
            aria-label="Dönem filtresi"
            value={filterTerm}
            onChange={(event) => setFilterTerm(event.target.value)}
          >
            <option value="">Dönem</option>
            <option value="1">1. Dönem</option>
            <option value="2">2. Dönem</option>
          </select>
          <select
            aria-label="Sınıf filtresi"
            value={filterClassId}
            onChange={(event) => setFilterClassId(event.target.value)}
          >
            <option value="">Sınıf</option>
            {classes
              .filter((schoolClass) => schoolClass.status === 'active')
              .map((schoolClass) => (
                <option key={schoolClass.id} value={schoolClass.id}>
                  {classLabel(schoolClass)}
                </option>
              ))}
          </select>
          {(filterCourseId || filterTerm || filterClassId) && (
            <button
              type="button"
              data-project-write="false"
              className="filter-clear-button"
              onClick={() => {
                setFilterCourseId('');
                setFilterTerm('');
                setFilterClassId('');
              }}
            >
              Filtreleri temizle
            </button>
          )}
        </div>

        {tasksQuery.isLoading ? (
          <div className="assessment-empty-state">
            <strong>Görevler yükleniyor…</strong>
          </div>
        ) : tasks.length === 0 ? (
          <div className="assessment-empty-state">
            <strong>Henüz performans görevi oluşturulmadı</strong>
            <span>
              Tema, öğrenme çıktıları ve gözlenebilir rubrik tanımlarıyla bir performans görevi
              oluşturabilirsiniz.
            </span>
            <button
              type="button"
              data-project-write="false"
              className="button button--primary"
              onClick={() => navigate(`/project/${encodeURIComponent(projectId)}/performance/new`)}
            >
              + Yeni görev oluştur
            </button>
          </div>
        ) : (
          <div className="assessment-card-grid">
            {tasks.map((task) => {
              const applications = task.classApplications.filter(
                (application) => application.status !== 'archived',
              );
              const versions = task.performanceDetails?.rubricVersions ?? [];
              const publishedVersions = performancePublishedVersions(versions);
              const draft = performanceRubricDraft(versions);
              const publishedLabel =
                publishedVersions.length > 0
                  ? `Rubrik v${publishedVersions[publishedVersions.length - 1]?.version ?? ''} yayınlandı`
                  : draft
                    ? 'Rubrik taslağı hazır'
                    : 'Rubrik yok';
              return (
                <article key={task.id} className="assessment-card">
                  <div className="assessment-card__heading">
                    <div>
                      <h3>{task.title || `${task.term}. Dönem ${task.sequenceNumber}. Performans`}</h3>
                      <span>
                        {task.courseName} · {task.gradeLevel}. sınıf · {task.term}. dönem ·{' '}
                        {task.sequenceNumber}. görev
                      </span>
                    </div>
                    <span className="assessment-card__status">Performans</span>
                  </div>
                  {task.performanceDetails?.theme && (
                    <p className="assessment-card__meta">
                      Tema: {task.performanceDetails.theme}
                    </p>
                  )}
                  <p className="assessment-card__meta">
                    {applications.length} sınıf ·{' '}
                    {applications.reduce((total, application) => total + application.studentScopeIds.length, 0)}{' '}
                    öğrenci
                  </p>
                  <div className="assessment-card__applications">
                    <div>
                      <span>{publishedLabel}</span>
                    </div>
                  </div>
                  <div className="assessment-card__actions">
                    <Link
                      className="button button--secondary"
                      to={`/project/${encodeURIComponent(projectId)}/activities/${encodeURIComponent(task.id)}/task`}
                    >
                      Görev ve Rubrik
                    </Link>
                    <Link
                      className="button button--secondary"
                      to={`/project/${encodeURIComponent(projectId)}/activities/${encodeURIComponent(task.id)}/assessment`}
                    >
                      Değerlendir
                    </Link>
                    <Link
                      className="button button--secondary"
                      to={`/project/${encodeURIComponent(projectId)}/performance/${encodeURIComponent(task.id)}`}
                    >
                      Düzenle
                    </Link>
                  </div>
                </article>
              );
            })}
          </div>
        )}
      </section>
    </div>
  );
}

function PerformanceCreateEditor({
  projectId,
  classes,
  assignments,
  courseOptions,
}: {
  projectId: string;
  classes: SchoolClass[];
  assignments: TeachingAssignment[];
  courseOptions: { key: string; id: string; name: string; year: string }[];
}) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [error, setError] = useState<AppError | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);

  const [courseKey, setCourseKey] = useState('');
  const [term, setTerm] = useState(1);
  const [sequenceNumber, setSequenceNumber] = useState(1);
  const [selectedClassIds, setSelectedClassIds] = useState<string[]>([]);
  const [title, setTitle] = useState('');
  const [details, setDetails] = useState<PerformanceDetails>(() => ({
    ...emptyPerformanceDetails(),
    rubricVersions: [],
  }));
  const [draft, setDraft] = useState<PerformanceRubric>(() => freshRubric(''));

  const selectedCourse = courseOptions.find((option) => option.key === courseKey);
  const sequenceQuery = useQuery({
    queryKey: [
      'assessment-sequence-options',
      projectId,
      selectedCourse?.year,
      selectedCourse?.id,
      term,
      'performance',
    ],
    queryFn: () =>
      commands.getAssessmentSequenceOptions({
        projectId,
        academicYearId: selectedCourse?.year ?? '',
        courseId: selectedCourse?.id ?? '',
        term,
        assessmentType: 'performance',
      }),
    enabled: !!projectId && !!selectedCourse,
  });
  const availableClasses = useMemo(() => {
    if (!selectedCourse) return [];
    const ids = new Set(
      assignments
        .filter((assignment) => `${assignment.academicYearId}:${assignment.courseId}` === selectedCourse.key)
        .map((assignment) => assignment.classSectionId),
    );
    return classes
      .filter((schoolClass) => schoolClass.status === 'active' && ids.has(schoolClass.id))
      .sort((left, right) => left.displayOrder - right.displayOrder);
  }, [assignments, classes, selectedCourse]);
  const selectedClasses = availableClasses.filter((schoolClass) =>
    selectedClassIds.includes(schoolClass.id),
  );
  const selectedGradeLevels = [
    ...new Set(
      selectedClasses
        .map((schoolClass) => schoolClass.gradeLevel)
        .filter((value): value is number => value !== null && value !== undefined),
    ),
  ];
  const derivedGradeLevel = selectedGradeLevels.length === 1 ? selectedGradeLevels[0] : null;
  const sequenceOptions = useMemo(() => sequenceQuery.data?.options ?? [1], [sequenceQuery.data]);

  useEffect(() => {
    if (sequenceOptions.length > 0 && !sequenceOptions.includes(sequenceNumber)) {
      setSequenceNumber(sequenceOptions[0] ?? 1);
    }
  }, [sequenceNumber, sequenceOptions]);

  const rubricIssues = useMemo(() => validatePerformanceRubric(draft), [draft]);
  const missingFields: string[] = [];
  if (!selectedCourse) missingFields.push('Ders');
  if (selectedClasses.length === 0) missingFields.push('En az bir sınıf');
  if (selectedGradeLevels.length > 1) missingFields.push('Aynı sınıf düzeyi');
  if (!sequenceQuery.isSuccess || !sequenceOptions.includes(sequenceNumber)) {
    missingFields.push('Görev sırası');
  }
  if (!title.trim() && !details.theme.trim()) missingFields.push('Tema veya görev adı');
  if (rubricIssues.length > 0) missingFields.push('Geçerli rubrik');

  const createMutation = useMutation({
    mutationFn: () => {
      if (!selectedCourse || !derivedGradeLevel) throw new Error('Form tamamlanmadı');
      return commands.createPerformanceTask({
        projectId,
        academicYearId: selectedCourse.year,
        courseId: selectedCourse.id,
        courseName: selectedCourse.name,
        gradeLevel: derivedGradeLevel,
        term,
        sequenceNumber,
        schoolClassIds: selectedClassIds,
        title: title.trim(),
        performanceDetails: {
          theme: details.theme,
          learningOutcomes: details.learningOutcomes,
          skillArea: details.skillArea,
          taskInstruction: details.taskInstruction,
          workMode: details.workMode,
          dueDate: details.dueDate,
          evidenceTypes: details.evidenceTypes,
        },
        initialRubric: draft,
      });
    },
    onMutate: () => {
      setError(null);
      setSuccessMessage(null);
    },
    onSuccess: async (activity) => {
      setSuccessMessage('Performans görevi oluşturuldu.');
      await queryClient.invalidateQueries({ queryKey: ['performance-tasks', projectId] });
      await queryClient.invalidateQueries({ queryKey: ['project-snapshot', projectId] });
      navigate(
        `/project/${encodeURIComponent(projectId)}/activities/${encodeURIComponent(activity.id)}/task`,
      );
    },
    onError: (caught: AppError) => setError(caught),
  });
  const canSubmit = missingFields.length === 0 && !createMutation.isPending;

  return (
    <div className="performance-page performance-editor-layout">
      <header className="performance-page__header">
        <div>
          <h2>Yeni Performans Görevi</h2>
          <p>Görev bilgilerini ve rubrik taslağını birlikte oluşturun.</p>
        </div>
        <Link
          className="button button--secondary"
          to={`/project/${encodeURIComponent(projectId)}/performance`}
        >
          Görev listesine dön
        </Link>
      </header>

      {error && <ErrorBanner error={error} />}
      {successMessage && (
        <div className="classes-notice" role="status">
          <CheckCircle2 size={17} />
          {successMessage}
        </div>
      )}

      <div className="performance-editor-grid">
        <div className="performance-editor-main">
          <section className="performance-panel">
            <div className="performance-panel__heading">
              <span className="performance-panel__index">01</span>
              <div>
                <h3>Görev bilgileri</h3>
                <p>Ders, dönem, sınıflar ve tema bilgilerini tanımlayın.</p>
              </div>
            </div>
            {assignments.length === 0 && (
              <p className="classes-warning">
                Görev oluşturabilmek için önce{' '}
                <Link to={`/project/${encodeURIComponent(projectId)}/classes`}>
                  Ders Alanı Kurulumu
                </Link>{' '}
                sayfasında aktif ders–sınıf görevlendirmesi oluşturun.
              </p>
            )}
            <div className="assessment-form-grid">
              <label>
                <span>Ders</span>
                <select
                  value={courseKey}
                  disabled={assignments.length === 0}
                  onChange={(event) => {
                    setCourseKey(event.target.value);
                    setSelectedClassIds([]);
                  }}
                >
                  <option value="">Ders seçin</option>
                  {courseOptions.map((option) => (
                    <option key={option.key} value={option.key}>
                      {option.name}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                <span>Dönem</span>
                <select
                  value={term}
                  onChange={(event) => {
                    setTerm(Number(event.target.value));
                    setSelectedClassIds([]);
                  }}
                >
                  <option value={1}>1. Dönem</option>
                  <option value={2}>2. Dönem</option>
                </select>
              </label>
              <label>
                <span>Görev sırası</span>
                <select
                  value={sequenceNumber}
                  onChange={(event) => setSequenceNumber(Number(event.target.value))}
                >
                  {sequenceOptions.map((value) => (
                    <option key={value} value={value}>
                      {value}. görev
                    </option>
                  ))}
                </select>
              </label>
              <label className="assessment-form-grid__wide">
                <span>Görev adı <small>(isteğe bağlı)</small></span>
                <input
                  value={title}
                  placeholder={`${term}. Dönem ${sequenceNumber}. Performans Görevi`}
                  onChange={(event) => setTitle(event.target.value)}
                />
              </label>
            </div>
            <div className="performance-panel__section">
              <h3>Uygulanacak sınıflar</h3>
              {!selectedCourse ? (
                <p className="assessment-form-help">Aktif görevlendirmelerden bir ders seçin.</p>
              ) : availableClasses.length === 0 ? (
                <p className="assessment-form-help">
                  Bu derse atanmış aktif sınıf yok.{' '}
                  <Link to={`/project/${encodeURIComponent(projectId)}/classes`}>Kuruluma git</Link>
                </p>
              ) : (
                <>
                  <div className="assessment-class-selection__actions">
                    <button
                      type="button"
                      data-project-write="false"
                      className="filter-clear-button"
                      onClick={() => setSelectedClassIds(availableClasses.map((schoolClass) => schoolClass.id))}
                    >
                      Tümünü seç
                    </button>
                    <button
                      type="button"
                      data-project-write="false"
                      className="filter-clear-button"
                      onClick={() => setSelectedClassIds([])}
                    >
                      Seçimi temizle
                    </button>
                  </div>
                  <div className="assessment-class-selection">
                    {availableClasses.map((schoolClass) => (
                      <label key={schoolClass.id}>
                        <input
                          type="checkbox"
                          checked={selectedClassIds.includes(schoolClass.id)}
                          onChange={(event) =>
                            setSelectedClassIds((current) =>
                              event.target.checked
                                ? [...current, schoolClass.id]
                                : current.filter((id) => id !== schoolClass.id),
                            )
                          }
                        />
                        <span>
                          <strong>{classLabel(schoolClass)}</strong>
                          <small>
                            {schoolClass.gradeLevel ? `${schoolClass.gradeLevel}. sınıf` : 'Sınıf düzeyi yok'}
                          </small>
                        </span>
                      </label>
                    ))}
                  </div>
                  {selectedGradeLevels.length > 1 && (
                    <p className="classes-warning">
                      Farklı sınıf düzeyleri aynı göreve seçilemez. Tek bir düzey seçin.
                    </p>
                  )}
                </>
              )}
            </div>
            <div className="performance-panel__section">
              <h3>Performans ayrıntıları</h3>
              <TaskDetailsFields details={details} onChange={setDetails} />
            </div>
          </section>

          <section className="performance-panel">
            <div className="performance-panel__heading">
              <span className="performance-panel__index">02</span>
              <div>
                <h3>Rubrik taslağı</h3>
                <p>
                  3-6 ölçüt, 3 veya 5 düzey; her düzey için gözlenebilir tanım. Yayınladığınızda
                  rubrik yeni bir sürüm olarak kaydedilir.
                </p>
              </div>
            </div>
            <RubricTemplateCatalog
              selectedSkillArea={details.skillArea}
              onApply={(templateId, rubric) => {
                const template = PERFORMANCE_TEMPLATES.find((candidate) => candidate.id === templateId);
                setDraft(rubric);
                if (template) setDetails((current) => ({ ...current, skillArea: template.skillArea }));
                setError(null);
                setSuccessMessage(null);
              }}
            />
            <RubricDraftEditor draft={draft} onChange={setDraft} disabled={false} />
            {rubricIssues.length > 0 && (
              <div className="performance-warnings">
                <AlertTriangle size={16} />
                <ul>
                  {rubricIssues.map((issue, index) => (
                    <li key={index}>{issue.message}</li>
                  ))}
                </ul>
              </div>
            )}
          </section>
        </div>

        <aside className="performance-sticky-side">
          <div className="performance-summary-card">
            <h3>Görev Özeti</h3>
            <div className="performance-summary-list">
              <div>
                <span>Ad</span>
                <strong>{title.trim() || `${term}. Dönem ${sequenceNumber}. Performans Görevi`}</strong>
              </div>
              <div>
                <span>Ders</span>
                <strong>{selectedCourse?.name || '—'}</strong>
              </div>
              <div>
                <span>Dönem / Sıra</span>
                <strong>{term}. Dönem · {sequenceNumber}. görev</strong>
              </div>
              <div>
                <span>Sınıflar</span>
                <strong>{selectedClasses.length ? selectedClasses.map(classLabel).join(', ') : '—'}</strong>
              </div>
              <div>
                <span>Rubrik</span>
                <strong>
                  {draft.criteria.length}/{draft.levels.length} ölçüt / düzey
                </strong>
              </div>
            </div>
            {missingFields.length > 0 ? (
              <div className="performance-checklist">
                <strong>Eksik:</strong>
                <ul>
                  {missingFields.map((field) => (
                    <li key={field}>• {field}</li>
                  ))}
                </ul>
              </div>
            ) : (
              <div className="performance-notice">
                <CheckCircle2 size={15} /> Görev oluşturulmaya hazır.
              </div>
            )}
            <LoadingButton
              type="button"
              className="button button--primary"
              loading={createMutation.isPending}
              disabledReason={
                !canSubmit
                  ? missingFields.length
                    ? `Eksik: ${missingFields.join(', ')}`
                    : undefined
                  : undefined
              }
              onClick={() => {
                if (canSubmit) createMutation.mutate();
              }}
            >
              <Send size={15} /> Görevi oluştur
            </LoadingButton>
          </div>
        </aside>
      </div>
    </div>
  );
}

function PerformanceEditEditor({
  projectId,
  activityId,
  classesById,
  refreshProjectData,
}: {
  projectId: string;
  activityId: string;
  classesById: Map<string, SchoolClass>;
  refreshProjectData: () => Promise<void>;
}) {
  const [error, setError] = useState<AppError | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const [title, setTitle] = useState('');
  const [details, setDetails] = useState<PerformanceDetails>(() => ({
    ...emptyPerformanceDetails(),
    rubricVersions: [],
  }));
  const [draft, setDraft] = useState<PerformanceRubric>(() => freshRubric(''));
  const initializedRef = useRef(false);

  const activityQuery = useQuery({
    queryKey: ['assessment-activity', projectId, activityId],
    queryFn: () => commands.getPerformanceTask({ projectId, activityId }),
    enabled: !!projectId && !!activityId,
  });
  const activity = activityQuery.data;

  useEffect(() => {
    if (!activity || initializedRef.current) return;
    initializedRef.current = true;
    const versions = activity.performanceDetails?.rubricVersions ?? [];
    setTitle(activity.title);
    setDetails(
      activity.performanceDetails
        ? {
            theme: activity.performanceDetails.theme,
            learningOutcomes: activity.performanceDetails.learningOutcomes,
            skillArea: activity.performanceDetails.skillArea,
            taskInstruction: activity.performanceDetails.taskInstruction,
            workMode: activity.performanceDetails.workMode,
            dueDate: activity.performanceDetails.dueDate,
            evidenceTypes: activity.performanceDetails.evidenceTypes,
            rubricVersions: [],
          }
        : { ...emptyPerformanceDetails(), rubricVersions: [] },
    );
    setDraft(performanceRubricDraft(versions) ?? freshRubric(activity.title));
  }, [activity]);

  const publishedVersions = useMemo(
    () => performancePublishedVersions(activity?.performanceDetails?.rubricVersions),
    [activity?.performanceDetails?.rubricVersions],
  );
  const approvedCount = useMemo(
    () =>
      (activity?.classApplications ?? []).reduce(
        (sum, application) =>
          sum +
          (application.performanceAssessments ?? []).filter(
            (assessment) => assessment.status === 'approved',
          ).length,
        0,
      ),
    [activity],
  );
  const rubricLocked = approvedCount > 0;
  const rubricIssues = useMemo(() => validatePerformanceRubric(draft), [draft]);
  const applications = useMemo(
    () => (activity?.classApplications ?? []).filter((application) => application.status !== 'archived'),
    [activity],
  );

  const updateDetailsMutation = useMutation({
    mutationFn: () =>
      commands.updatePerformanceTask({
        projectId,
        activityId,
        title: title.trim() || undefined,
        performanceDetails: {
          theme: details.theme,
          learningOutcomes: details.learningOutcomes,
          skillArea: details.skillArea,
          taskInstruction: details.taskInstruction,
          workMode: details.workMode,
          dueDate: details.dueDate,
          evidenceTypes: details.evidenceTypes,
        },
      }),
    onMutate: () => {
      setError(null);
      setSuccessMessage(null);
    },
    onSuccess: async () => {
      setSuccessMessage('Görev bilgileri güncellendi.');
      await refreshProjectData();
    },
    onError: (caught: AppError) => setError(caught),
  });

  const publishMutation = useMutation({
    mutationFn: () =>
      commands.publishPerformanceRubric({
        projectId,
        activityId,
        rubric: draft,
      }),
    onMutate: () => {
      setError(null);
      setSuccessMessage(null);
    },
    onSuccess: async (published) => {
      setSuccessMessage(`Rubrik yayınlandı (sürüm ${published.version}).`);
      await refreshProjectData();
    },
    onError: (caught: AppError) => setError(caught),
  });

  const publishDisabledReason = rubricLocked
    ? 'Onaylı değerlendirmeler bulunduğundan rubrik kilitli; yeni sürüm yayınlanamaz.'
    : rubricIssues.length > 0
      ? 'Rubrik doğrulama hataları var.'
      : undefined;

  if (activityQuery.isLoading) {
    return (
      <div className="performance-page">
        <ProjectContextState pageLabel="Performans Görevi" loading projectPath="" />
      </div>
    );
  }
  if (!activity) {
    return (
      <div className="performance-page">
        <ErrorBanner error={error ?? ({ code: 'UNKNOWN_ERROR', safeMessage: 'Performans görevi bulunamadı.', correlationId: 'unknown', retryable: false, detailsAvailable: false } as AppError)} />
        <Link className="button button--secondary" to={`/project/${encodeURIComponent(projectId)}/performance`}>
          Görev listesine dön
        </Link>
      </div>
    );
  }

  return (
    <div className="performance-page performance-editor-layout">
      <header className="performance-page__header">
        <div>
          <h2>{title.trim() || `${activity.term}. Dönem ${activity.sequenceNumber}. Performans`}</h2>
          <p>
            {activity.courseName} · {activity.gradeLevel}. sınıf · {activity.term}. dönem ·{' '}
            {activity.sequenceNumber}. görev
          </p>
        </div>
        <div className="performance-page__header-actions">
          <Link
            className="button button--secondary"
            to={`/project/${encodeURIComponent(projectId)}/activities/${encodeURIComponent(activityId)}/assessment`}
          >
            <ClipboardCheck size={15} /> Değerlendirmeye geç
          </Link>
          <Link
            className="button button--secondary"
            to={`/project/${encodeURIComponent(projectId)}/performance`}
          >
            Görev listesine dön
          </Link>
        </div>
      </header>

      {(error || activityQuery.error) && (
        <ErrorBanner error={(error || activityQuery.error) as AppError} />
      )}
      {successMessage && (
        <div className="classes-notice" role="status">
          <CheckCircle2 size={17} />
          {successMessage}
        </div>
      )}

      <div className="performance-editor-grid">
        <div className="performance-editor-main">
          <section className="performance-panel">
            <div className="performance-panel__heading">
              <span className="performance-panel__index">01</span>
              <div>
                <h3>Görev bilgileri</h3>
                <p>Tema, öğrenme çıktıları ve görev yönergesini düzenleyin.</p>
              </div>
            </div>
            <TaskDetailsFields details={details} onChange={setDetails} />
            <div className="performance-panel__actions">
              <LoadingButton
                type="button"
                className="button button--primary"
                loading={updateDetailsMutation.isPending}
                onClick={() => updateDetailsMutation.mutate()}
              >
                <Save size={15} /> Görev bilgilerini kaydet
              </LoadingButton>
            </div>
          </section>

          <section className="performance-panel">
            <div className="performance-panel__heading">
              <span className="performance-panel__index">02</span>
              <div>
                <h3>Bağlı sınıflar</h3>
                <p>Görevin uygulandığı sınıf uygulamaları ve öğrenci sayıları.</p>
              </div>
              <Users size={18} />
            </div>
            {applications.length === 0 ? (
              <p className="assessment-form-help">Bu göreve bağlı aktif sınıf uygulaması yok.</p>
            ) : (
              <div className="performance-class-list">
                {applications.map((application) => {
                  const schoolClass = classesById.get(application.schoolClassId);
                  return (
                    <div key={application.id}>
                      <strong>{schoolClass ? classLabel(schoolClass) : 'Sınıf bilgisi yok'}</strong>
                      <span>{application.studentScopeIds.length} öğrenci</span>
                    </div>
                  );
                })}
              </div>
            )}
          </section>

          <section className="performance-panel">
            <div className="performance-panel__heading">
              <span className="performance-panel__index">03</span>
              <div>
                <h3>Rubrik</h3>
                <p>
                  Taslak (sürüm 0) düzenlenebilir; yayınlama yeni bir sürüm üretir. Yayınlanmış
                  sürümler yalnız görüntülenir.
                </p>
              </div>
              <History size={18} />
            </div>

            {rubricLocked && (
              <div className="performance-warnings performance-warnings--lock">
                <Lock size={16} />
                <span>
                  Bu görevde onaylanmış değerlendirmeler bulunuyor. Rubrik kilitlidir; yeni sürüm
                  yayınlanamaz ve değerlendirme sürümleri değişmez.
                </span>
              </div>
            )}

            <div className="performance-rubric-history">
              <span className="performance-rubric-editor__label">Yayınlanmış sürümler</span>
              {publishedVersions.length === 0 ? (
                <p className="assessment-form-help">Henüz yayınlanmış rubrik sürümü yok.</p>
              ) : (
                [...publishedVersions]
                  .sort((left, right) => right.version - left.version)
                  .map((rubric) => <PublishedRubricView key={`${rubric.id}-${rubric.version}`} rubric={rubric} />)
              )}
            </div>

            <div className="performance-rubric-draft">
              <span className="performance-rubric-editor__label">
                Taslak (sürüm 0) {rubricLocked ? '· yalnız görüntüleme' : '· düzenlenebilir'}
              </span>
              <RubricDraftEditor
                draft={draft}
                onChange={setDraft}
                disabled={rubricLocked}
              />
              {rubricIssues.length > 0 && (
                <div className="performance-warnings">
                  <AlertTriangle size={16} />
                  <ul>
                    {rubricIssues.map((issue, index) => (
                      <li key={index}>{issue.message}</li>
                    ))}
                  </ul>
                </div>
              )}
              <div className="performance-panel__actions">
                <LoadingButton
                  type="button"
                  className="button button--primary"
                  loading={publishMutation.isPending}
                  disabledReason={publishDisabledReason}
                  onClick={() => publishMutation.mutate()}
                >
                  <Send size={15} /> Rubriği yayınla (yeni sürüm)
                </LoadingButton>
              </div>
            </div>
          </section>
        </div>

        <aside className="performance-sticky-side">
          <div className="performance-summary-card">
            <h3>Görev Özeti</h3>
            <div className="performance-summary-list">
              <div>
                <span>Ad</span>
                <strong>{title.trim() || `${activity.term}. Dönem ${activity.sequenceNumber}. Performans`}</strong>
              </div>
              <div>
                <span>Beceri alanı</span>
                <strong>{performanceSkillAreaLabels[details.skillArea]}</strong>
              </div>
              <div>
                <span>Çalışma biçimi</span>
                <strong>{performanceWorkModeLabels[details.workMode]}</strong>
              </div>
              <div>
                <span>Sınıflar</span>
                <strong>{applications.length} sınıf</strong>
              </div>
              <div>
                <span>Rubrik</span>
                <strong>
                  {publishedVersions.length > 0
                    ? `${publishedVersions.length} sürüm yayınlandı`
                    : 'Taslak'}
                </strong>
              </div>
              <div>
                <span>Onaylanan değerlendirme</span>
                <strong>{approvedCount}</strong>
              </div>
            </div>
            <div className="performance-notice">
              <CalendarDays size={14} />
              {details.dueDate
                ? `Teslim tarihi: ${formatDate(details.dueDate)}`
                : 'Teslim tarihi belirtilmedi'}
            </div>
          </div>
        </aside>
      </div>
    </div>
  );
}

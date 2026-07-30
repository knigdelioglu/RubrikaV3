import type {
  ProjectSnapshot,
  Question,
  RubricQuestionSnapshot,
  RubricValidationReport,
  WorkflowSnapshot,
} from '../api/types';
import type { RubricDraft } from '../components/rubric/RubricQuestionCard';
import type { ExamPackageTab } from '../app/projectRoutes';

export type ExamPackageQuestionItem = {
  id: string;
  number: number;
  maxScore: number;
  questionStatus: Question['questionText']['status'];
  rubricStatus: Question['rubric']['status'];
  needsReview: boolean;
  questionLabel: string;
  rubricLabel: string;
};

export type ExamPackageWorkspaceSummary = {
  totalQuestions: number;
  readyQuestionCount: number;
  readyRubricCount: number;
  invalidQuestionCount: number;
  invalidRubricCount: number;
  totalScore: number;
  blockerCount: number;
  freezeReady: boolean;
  frozen: boolean;
  invalidated: boolean;
};

const QUESTION_READY = new Set<Question['questionText']['status']>(['confirmed', 'edited']);
const RUBRIC_INVALID = new Set<Question['rubric']['status']>(['missing', 'invalid', 'legacy']);

export function normalizeExamPackageTab(rawTab: string | null): ExamPackageTab {
  return rawTab === 'rubric' || rawTab === 'freeze' ? rawTab : 'question';
}

export function resolveSelectedQuestionId(
  questions: Pick<Question, 'id' | 'number'>[],
  requestedQuestionId: string | null,
): string | null {
  const ordered = [...questions].sort((left, right) => left.number - right.number);
  if (requestedQuestionId && ordered.some((question) => question.id === requestedQuestionId)) {
    return requestedQuestionId;
  }
  return ordered[0]?.id ?? null;
}

export function buildExamPackageQuestionItems(
  questions: Question[],
  rubricItems: RubricQuestionSnapshot[],
): ExamPackageQuestionItem[] {
  const validationByQuestionId = new Map(
    rubricItems.map((item) => [item.question.id, item.validation] as const),
  );
  return [...questions]
    .sort((left, right) => left.number - right.number)
    .map((question) => {
      const validation = validationByQuestionId.get(question.id);
      const questionReady = QUESTION_READY.has(question.questionText.status);
      const rubricReady = question.rubric.status === 'confirmed';
      const needsReview = !questionReady
        || !rubricReady
        || question.questionText.warnings.length > 0
        || !!validation?.issues.length
        || !!validation?.warnings.length;
      return {
        id: question.id,
        number: question.number,
        maxScore: question.rubric.maxScore ?? question.maxScore,
        questionStatus: question.questionText.status,
        rubricStatus: question.rubric.status,
        needsReview,
        questionLabel: questionReady ? 'Soru metni hazır' : 'Soru metni kontrol bekliyor',
        rubricLabel: rubricReady
          ? 'Rubrik hazır'
          : RUBRIC_INVALID.has(question.rubric.status)
            ? 'Rubrik eksik veya geçersiz'
            : 'Rubrik kontrol bekliyor',
      };
    });
}

export function buildExamPackageWorkspaceSummary(
  project: Pick<ProjectSnapshot, 'questions' | 'examPackageFreeze'>,
  workflow: Pick<WorkflowSnapshot, 'blockingReasons' | 'summary'>,
  validation: Pick<RubricValidationReport, 'blockingQuestions'> | null,
): ExamPackageWorkspaceSummary {
  const readyQuestionCount = project.questions.filter((question) => QUESTION_READY.has(question.questionText.status)).length;
  const readyRubricCount = project.questions.filter((question) => question.rubric.status === 'confirmed').length;
  const invalidQuestionCount = project.questions.length - readyQuestionCount;
  const invalidRubricCount = project.questions.filter((question) => RUBRIC_INVALID.has(question.rubric.status)).length;
  const blockerQuestions = new Set(validation?.blockingQuestions ?? []);
  const frozen = project.examPackageFreeze?.freezeStatus === 'frozen';
  const invalidated = project.examPackageFreeze?.freezeStatus === 'invalidated';
  return {
    totalQuestions: project.questions.length,
    readyQuestionCount,
    readyRubricCount,
    invalidQuestionCount,
    invalidRubricCount,
    totalScore: project.questions.reduce(
      (total, question) => total + (question.rubric.maxScore ?? question.maxScore),
      0,
    ),
    blockerCount: new Set([
      ...workflow.blockingReasons.map(String),
      ...[...blockerQuestions].map((number) => `question:${number}`),
    ]).size,
    freezeReady: workflow.summary.readiness.examPackageFreeze,
    frozen,
    invalidated,
  };
}

export function rubricDraftFromItem(item: RubricQuestionSnapshot): RubricDraft {
  return {
    answerType: item.question.answerType,
    maxScore: item.question.rubric.maxScore?.toString() ?? '',
    expectedAnswer: item.question.rubric.expectedAnswer ?? '',
    criteria: item.question.rubric.criteria.length > 0
      ? item.question.rubric.criteria.map((criterion) => ({
          id: criterion.id,
          label: criterion.label,
          description: criterion.description,
          points: criterion.points.toString(),
        }))
      : [],
    partialCreditHints: item.question.rubric.partialCreditHints.join('\n'),
    zeroScoreConditions: item.question.rubric.zeroScoreConditions.join('\n'),
    commonMistakes: item.question.rubric.commonMistakes.join('\n'),
  };
}

export function splitRubricLines(value: string): string[] {
  return value.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
}

export function mergePersistedDrafts<T>(
  current: Record<string, T>,
  persisted: ReadonlyArray<readonly [string, T]>,
  dirtyIds: ReadonlySet<string>,
): Record<string, T> {
  const next = { ...current };
  for (const [id, value] of persisted) {
    if (!dirtyIds.has(id)) next[id] = value;
  }
  return next;
}

export function createSingleFlightAction<TArgs extends unknown[], TResult>(
  action: (...args: TArgs) => Promise<TResult>,
): (...args: TArgs) => Promise<TResult | undefined> {
  let active: Promise<TResult> | null = null;
  return (...args: TArgs) => {
    if (active) return active.then(() => undefined);
    active = action(...args);
    return active.finally(() => {
      active = null;
    });
  };
}

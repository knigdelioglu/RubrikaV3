import type {
  AssessmentActivity,
  AssessmentType,
  WorkflowSnapshot,
} from '../api/types';

export type ExamStepStatus =
  | 'not_started'
  | 'ready'
  | 'in_progress'
  | 'needs_review'
  | 'completed'
  | 'blocked';

export type ExamStepDefinition = {
  id: string;
  index: number;
  label: string;
  description: string;
};

export type ExamStepState = {
  definition: ExamStepDefinition;
  status: ExamStepStatus;
  statusLabel: string;
  blockerMessage?: string;
};

export const WRITTEN_EXAM_STEPS: ExamStepDefinition[] = [
  { id: 'prep', index: 1, label: 'Hazırlık', description: 'Sınav dokümanları, soru metinleri ve rubrikler' },
  { id: 'students', index: 2, label: 'Öğrenci Kâğıtları', description: 'Kâğıt yükleme, gruplama, kimlik eşleme ve kırpma' },
  { id: 'ocr', index: 3, label: 'OCR ve Kontrol', description: 'Yapay zekâ cevap okuma ve öğretmen incelemesi' },
  { id: 'scoring', index: 4, label: 'Puanlama', description: 'Otomatik notlandırma ve kâğıt bazlı değerlendirme' },
  { id: 'results', index: 5, label: 'Sonuçlar', description: 'Sınav raporları, grafikler ve istatistikler' },
];

export const LISTENING_EXAM_STEPS: ExamStepDefinition[] = [
  { id: 'listening_content', index: 1, label: 'Dinleme İçeriği', description: 'Dinleme kaydı, dinletme sayısı, süre ve yönergeler' },
  { id: 'questions', index: 2, label: 'Sorular ve Rubrikler', description: 'Soru metinleri ve puanlama rubrikleri' },
  { id: 'students', index: 3, label: 'Öğrenci Kâğıtları', description: 'Kâğıt yükleme, gruplama ve kimlik eşleme' },
  { id: 'ocr_scoring', index: 4, label: 'OCR ve Puanlama', description: 'Cevap OCR’ı ve otomatik notlandırma' },
  { id: 'results', index: 5, label: 'Sonuçlar', description: 'Dinleme sınavı başarı raporları' },
];

export const SPEAKING_EXAM_STEPS: ExamStepDefinition[] = [
  { id: 'settings', index: 1, label: 'Sınav Ayarları', description: 'Konuşma görevi, hedef süreler ve rubrik seçimi' },
  { id: 'students', index: 2, label: 'Öğrenciler', description: 'Sınıf uygulaması ve öğrenci listesi' },
  { id: 'transcript', index: 3, label: 'Kayıt ve Transkript', description: 'Ses kaydı alma, Whisper transkripti ve Gemma düzenleme' },
  { id: 'evaluation', index: 4, label: 'Değerlendirme', description: 'Ölçüt puanlaması ve öğretmen yıldız değerlendirmesi' },
  { id: 'results', index: 5, label: 'Sonuçlar', description: 'Konuşma performansı analizleri ve raporları' },
];

export const EXAM_STEP_STATUS_LABELS: Record<ExamStepStatus, string> = {
  not_started: 'Başlamadı',
  ready: 'Hazır',
  in_progress: 'Devam ediyor',
  needs_review: 'Kontrol gerekiyor',
  completed: 'Tamamlandı',
  blocked: 'Engelli',
};

export function getExamStepDefinitions(type: AssessmentType): ExamStepDefinition[] {
  switch (type) {
    case 'listening':
      return LISTENING_EXAM_STEPS;
    case 'speaking':
      return SPEAKING_EXAM_STEPS;
    case 'written':
    default:
      return WRITTEN_EXAM_STEPS;
  }
}

export function deriveExamStepStatuses(
  activity: AssessmentActivity,
  workflowSnapshot?: WorkflowSnapshot | null,
  classApplicationId?: string | null,
): ExamStepState[] {
  const type = activity.assessmentType;
  const definitions = getExamStepDefinitions(type);

  if (type === 'speaking') {
    return deriveSpeakingStepStatuses(activity, definitions, classApplicationId);
  }

  if (type === 'listening') {
    return deriveListeningStepStatuses(activity, definitions, workflowSnapshot);
  }

  return deriveWrittenStepStatuses(activity, definitions, workflowSnapshot);
}

function deriveWrittenStepStatuses(
  _activity: AssessmentActivity,
  definitions: ExamStepDefinition[],
  workflowSnapshot?: WorkflowSnapshot | null,
): ExamStepState[] {
  const readiness = workflowSnapshot?.summary.readiness;
  const stage = workflowSnapshot?.currentStage;

  const isPrepComplete = readiness?.examPackageFreeze ?? (stage === 'qep_frozen' || stage === 'scoring_ready' || stage === 'scoring_running' || stage === 'scoring_done' || stage === 'analysis_ready');
  const isStudentIntakeComplete = readiness?.studentIntake ?? (stage === 'ocr_ready' || stage === 'ocr_running' || stage === 'review_required' || stage === 'student_answer_ocr_running' || stage === 'student_answer_ocr_review_needed' || stage === 'student_answer_ocr_ready_for_scoring' || stage === 'scoring_ready' || stage === 'scoring_running' || stage === 'scoring_done' || stage === 'analysis_ready');
  const isOcrComplete = stage === 'student_answer_ocr_ready_for_scoring' || stage === 'scoring_ready' || stage === 'scoring_running' || stage === 'scoring_done' || stage === 'analysis_ready';
  const isScoringComplete = stage === 'scoring_done' || stage === 'analysis_ready';

  return definitions.map((def) => {
    switch (def.id) {
      case 'prep': {
        if (isPrepComplete) {
          return { definition: def, status: 'completed', statusLabel: EXAM_STEP_STATUS_LABELS.completed };
        }
        if (stage === 'question_text_extraction_running' || stage === 'exam_package_build_running') {
          return { definition: def, status: 'in_progress', statusLabel: EXAM_STEP_STATUS_LABELS.in_progress };
        }
        if (stage === 'exam_package_review_needed' || stage === 'rubric_imported_needs_review') {
          return { definition: def, status: 'needs_review', statusLabel: EXAM_STEP_STATUS_LABELS.needs_review };
        }
        return { definition: def, status: 'ready', statusLabel: EXAM_STEP_STATUS_LABELS.ready };
      }

      case 'students': {
        if (!isPrepComplete) {
          return {
            definition: def,
            status: 'blocked',
            statusLabel: EXAM_STEP_STATUS_LABELS.blocked,
            blockerMessage: 'Soru ve rubrik paketi dondurulmadan öğrenci kâğıtları işlenemez.',
          };
        }
        if (isStudentIntakeComplete) {
          return { definition: def, status: 'completed', statusLabel: EXAM_STEP_STATUS_LABELS.completed };
        }
        if (stage === 'student_scans_missing' || stage === 'documents_missing') {
          return { definition: def, status: 'not_started', statusLabel: EXAM_STEP_STATUS_LABELS.not_started };
        }
        return { definition: def, status: 'ready', statusLabel: EXAM_STEP_STATUS_LABELS.ready };
      }

      case 'ocr': {
        if (!isStudentIntakeComplete) {
          return {
            definition: def,
            status: 'blocked',
            statusLabel: EXAM_STEP_STATUS_LABELS.blocked,
            blockerMessage: 'Öğrenci kâğıtları ve eşlemeleri tamamlanmadan OCR başlatılamaz.',
          };
        }
        if (isOcrComplete) {
          return { definition: def, status: 'completed', statusLabel: EXAM_STEP_STATUS_LABELS.completed };
        }
        if (stage === 'ocr_running' || stage === 'student_answer_ocr_running') {
          return { definition: def, status: 'in_progress', statusLabel: EXAM_STEP_STATUS_LABELS.in_progress };
        }
        if (stage === 'review_required' || stage === 'student_answer_ocr_review_needed') {
          return { definition: def, status: 'needs_review', statusLabel: EXAM_STEP_STATUS_LABELS.needs_review };
        }
        return { definition: def, status: 'ready', statusLabel: EXAM_STEP_STATUS_LABELS.ready };
      }

      case 'scoring': {
        if (!isOcrComplete) {
          return {
            definition: def,
            status: 'blocked',
            statusLabel: EXAM_STEP_STATUS_LABELS.blocked,
            blockerMessage: 'Tüm öğrenci cevap OCR okumaları onaylanmadan puanlama çalıştırılamaz.',
          };
        }
        if (isScoringComplete) {
          return { definition: def, status: 'completed', statusLabel: EXAM_STEP_STATUS_LABELS.completed };
        }
        if (stage === 'scoring_running') {
          return { definition: def, status: 'in_progress', statusLabel: EXAM_STEP_STATUS_LABELS.in_progress };
        }
        return { definition: def, status: 'ready', statusLabel: EXAM_STEP_STATUS_LABELS.ready };
      }

      case 'results': {
        if (isScoringComplete) {
          return { definition: def, status: 'completed', statusLabel: EXAM_STEP_STATUS_LABELS.completed };
        }
        if (stage === 'scoring_ready' || isOcrComplete) {
          return { definition: def, status: 'ready', statusLabel: EXAM_STEP_STATUS_LABELS.ready };
        }
        return {
          definition: def,
          status: 'blocked',
          statusLabel: EXAM_STEP_STATUS_LABELS.blocked,
          blockerMessage: 'Sonuçlar ve raporlar puanlama tamamlandıktan sonra oluşur.',
        };
      }

      default:
        return { definition: def, status: 'not_started', statusLabel: EXAM_STEP_STATUS_LABELS.not_started };
    }
  });
}

function deriveListeningStepStatuses(
  activity: AssessmentActivity,
  definitions: ExamStepDefinition[],
  workflowSnapshot?: WorkflowSnapshot | null,
): ExamStepState[] {
  const details = activity.listeningDetails;
  const isContentReady = !!(details?.audioDocumentId || details?.instruction);
  const readiness = workflowSnapshot?.summary.readiness;
  const stage = workflowSnapshot?.currentStage;

  const isQuestionsReady = readiness?.examPackageFreeze ?? (stage === 'qep_frozen' || stage === 'scoring_ready' || stage === 'scoring_running' || stage === 'scoring_done' || stage === 'analysis_ready');
  const isStudentIntakeComplete = readiness?.studentIntake ?? (stage === 'ocr_ready' || stage === 'ocr_running' || stage === 'review_required' || stage === 'student_answer_ocr_running' || stage === 'student_answer_ocr_review_needed' || stage === 'student_answer_ocr_ready_for_scoring' || stage === 'scoring_ready' || stage === 'scoring_running' || stage === 'scoring_done' || stage === 'analysis_ready');
  const isScoringComplete = stage === 'scoring_done' || stage === 'analysis_ready';

  return definitions.map((def) => {
    switch (def.id) {
      case 'listening_content': {
        if (isContentReady) {
          return { definition: def, status: 'completed', statusLabel: EXAM_STEP_STATUS_LABELS.completed };
        }
        return { definition: def, status: 'ready', statusLabel: EXAM_STEP_STATUS_LABELS.ready };
      }

      case 'questions': {
        if (isQuestionsReady) {
          return { definition: def, status: 'completed', statusLabel: EXAM_STEP_STATUS_LABELS.completed };
        }
        return { definition: def, status: 'ready', statusLabel: EXAM_STEP_STATUS_LABELS.ready };
      }

      case 'students': {
        if (!isQuestionsReady) {
          return {
            definition: def,
            status: 'blocked',
            statusLabel: EXAM_STEP_STATUS_LABELS.blocked,
            blockerMessage: 'Dinleme soruları ve rubrikleri dondurulmadan kâğıt işlenemez.',
          };
        }
        if (isStudentIntakeComplete) {
          return { definition: def, status: 'completed', statusLabel: EXAM_STEP_STATUS_LABELS.completed };
        }
        return { definition: def, status: 'ready', statusLabel: EXAM_STEP_STATUS_LABELS.ready };
      }

      case 'ocr_scoring': {
        if (!isStudentIntakeComplete) {
          return {
            definition: def,
            status: 'blocked',
            statusLabel: EXAM_STEP_STATUS_LABELS.blocked,
            blockerMessage: 'Öğrenci kâğıtları yüklenmeden OCR ve puanlama yapılamaz.',
          };
        }
        if (isScoringComplete) {
          return { definition: def, status: 'completed', statusLabel: EXAM_STEP_STATUS_LABELS.completed };
        }
        if (stage === 'ocr_running' || stage === 'scoring_running') {
          return { definition: def, status: 'in_progress', statusLabel: EXAM_STEP_STATUS_LABELS.in_progress };
        }
        if (stage === 'review_required' || stage === 'student_answer_ocr_review_needed') {
          return { definition: def, status: 'needs_review', statusLabel: EXAM_STEP_STATUS_LABELS.needs_review };
        }
        return { definition: def, status: 'ready', statusLabel: EXAM_STEP_STATUS_LABELS.ready };
      }

      case 'results': {
        if (isScoringComplete) {
          return { definition: def, status: 'completed', statusLabel: EXAM_STEP_STATUS_LABELS.completed };
        }
        return {
          definition: def,
          status: 'blocked',
          statusLabel: EXAM_STEP_STATUS_LABELS.blocked,
          blockerMessage: 'Sonuçlar dinleme sınavı puanlandıktan sonra gösterilir.',
        };
      }

      default:
        return { definition: def, status: 'not_started', statusLabel: EXAM_STEP_STATUS_LABELS.not_started };
    }
  });
}

function deriveSpeakingStepStatuses(
  activity: AssessmentActivity,
  definitions: ExamStepDefinition[],
  classApplicationId?: string | null,
): ExamStepState[] {
  const config = activity.speakingConfiguration;
  const isConfigured = !!(config && config.taskText.trim());
  const applications = activity.classApplications.filter((app) => app.status !== 'archived');
  const targetApplication = classApplicationId
    ? applications.find((app) => app.id === classApplicationId)
    : applications[0];

  const attempts = targetApplication?.speakingAttempts ?? applications.flatMap((app) => app.speakingAttempts);
  const totalStudents = targetApplication
    ? targetApplication.studentScopeIds.length
    : applications.reduce((sum, app) => sum + app.studentScopeIds.length, 0);

  const completedAttempts = attempts.filter(
    (attempt) => attempt.state === 'approved' || attempt.state === 'teacher_review',
  ).length;
  const reviewNeededAttempts = attempts.filter((attempt) => attempt.state === 'teacher_review').length;
  const approvedAttempts = attempts.filter((attempt) => attempt.state === 'approved').length;

  return definitions.map((def) => {
    switch (def.id) {
      case 'settings': {
        if (isConfigured) {
          return { definition: def, status: 'completed', statusLabel: EXAM_STEP_STATUS_LABELS.completed };
        }
        return { definition: def, status: 'ready', statusLabel: EXAM_STEP_STATUS_LABELS.ready };
      }

      case 'students': {
        if (!isConfigured) {
          return {
            definition: def,
            status: 'blocked',
            statusLabel: EXAM_STEP_STATUS_LABELS.blocked,
            blockerMessage: 'Konuşma görevi ve ayarları kaydedilmeden öğrenciler seçilemez.',
          };
        }
        if (applications.length > 0) {
          return { definition: def, status: 'completed', statusLabel: EXAM_STEP_STATUS_LABELS.completed };
        }
        return { definition: def, status: 'ready', statusLabel: EXAM_STEP_STATUS_LABELS.ready };
      }

      case 'transcript': {
        if (!isConfigured || applications.length === 0) {
          return {
            definition: def,
            status: 'blocked',
            statusLabel: EXAM_STEP_STATUS_LABELS.blocked,
            blockerMessage: 'Sınav ayarları ve sınıf uygulaması tamamlanmadan kayıt alınamaz.',
          };
        }
        if (completedAttempts > 0 && completedAttempts >= totalStudents && totalStudents > 0) {
          return { definition: def, status: 'completed', statusLabel: EXAM_STEP_STATUS_LABELS.completed };
        }
        if (completedAttempts > 0) {
          return { definition: def, status: 'in_progress', statusLabel: EXAM_STEP_STATUS_LABELS.in_progress };
        }
        return { definition: def, status: 'ready', statusLabel: EXAM_STEP_STATUS_LABELS.ready };
      }

      case 'evaluation': {
        if (completedAttempts === 0) {
          return {
            definition: def,
            status: 'blocked',
            statusLabel: EXAM_STEP_STATUS_LABELS.blocked,
            blockerMessage: 'Değerlendirme için önce en az bir öğrenci konuşma kaydı ve transkripti alınmalıdır.',
          };
        }
        if (reviewNeededAttempts > 0) {
          return { definition: def, status: 'needs_review', statusLabel: EXAM_STEP_STATUS_LABELS.needs_review };
        }
        if (approvedAttempts > 0 && approvedAttempts >= totalStudents && totalStudents > 0) {
          return { definition: def, status: 'completed', statusLabel: EXAM_STEP_STATUS_LABELS.completed };
        }
        return { definition: def, status: 'ready', statusLabel: EXAM_STEP_STATUS_LABELS.ready };
      }

      case 'results': {
        if (approvedAttempts > 0 || completedAttempts > 0) {
          return { definition: def, status: 'ready', statusLabel: EXAM_STEP_STATUS_LABELS.ready };
        }
        return {
          definition: def,
          status: 'blocked',
          statusLabel: EXAM_STEP_STATUS_LABELS.blocked,
          blockerMessage: 'Konuşma sınavı sonuçları değerlendirme tamamlandıktan sonra izlenebilir.',
        };
      }

      default:
        return { definition: def, status: 'not_started', statusLabel: EXAM_STEP_STATUS_LABELS.not_started };
    }
  });
}

export function resolveNextExamStep(
  activity: AssessmentActivity,
  workflowSnapshot?: WorkflowSnapshot | null,
  classApplicationId?: string | null,
): ExamStepDefinition {
  const states = deriveExamStepStatuses(activity, workflowSnapshot, classApplicationId);

  // 1. Prioritize step that needs review
  const needsReviewState = states.find((s) => s.status === 'needs_review');
  if (needsReviewState) return needsReviewState.definition;

  // 2. Prioritize step in progress
  const inProgressState = states.find((s) => s.status === 'in_progress');
  if (inProgressState) return inProgressState.definition;

  // 3. Find first ready step
  const readyState = states.find((s) => s.status === 'ready');
  if (readyState) return readyState.definition;

  // 4. If all completed, return results
  const resultsState = states.find((s) => s.definition.id === 'results');
  if (resultsState) return resultsState.definition;

  // Fallback to step 1
  return states[0]?.definition ?? WRITTEN_EXAM_STEPS[0]!;
}

export function getCanonicalWorkspaceStepPath(
  projectId: string,
  activityId: string,
  stepId: string,
  rawSearch = '',
): string {
  const search = new URLSearchParams(rawSearch);
  search.delete('projectId');
  search.delete('projectPath');
  search.delete('assessmentActivityId');
  const query = search.toString();
  return `/project/${encodeURIComponent(projectId)}/activities/${encodeURIComponent(activityId)}/${encodeURIComponent(stepId)}${query ? `?${query}` : ''}`;
}

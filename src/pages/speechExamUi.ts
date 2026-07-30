import type { SpeakingMetrics, SpeakingPerformanceLevel } from '../api/types';

export type SpeakingPipelineState = {
  whisper: { done: boolean; label: 'Hazır' | 'Canlı' | 'Tamamlandı' };
  cleanup: { done: boolean; label: 'Bekliyor' | 'Düzeltiliyor' | 'Tamamlandı' | 'İnceleme gerekli' };
  rubric: { done: boolean; label: 'Bekliyor' | 'Değerlendiriliyor' | 'Tamamlandı' | 'İnceleme gerekli' };
};

const subindicatorLabels: Record<string, string> = {
  task_relevance: 'Göreve uygunluk',
  main_idea: 'Ana düşüncenin açıklığı',
  supporting_ideas: 'Destekleyici fikirlerin geliştirilmesi',
  examples_reasons: 'Örnek ve gerekçe kullanımı',
  content_depth: 'İçerik derinliği',
  opening: 'Planlı giriş',
  idea_order: 'Fikirlerin sıralanışı',
  transitions: 'Geçişlerin işlevi',
  coherence: 'Anlam bütünlüğü',
  conclusion: 'Sonuç ve kapanış',
  sentence_clarity: 'Cümlelerin açıklığı',
  vocabulary_range: 'Söz varlığı çeşitliliği',
  contextual_word_use: 'Sözcüklerin bağlama uygunluğu',
  connectors: 'Bağlaçların kullanımı',
  repetition_control: 'Tekrarların kontrolü',
};

const performanceLevelLabels: Record<string, string> = {
  absent: 'Gözlenmedi',
  limited: 'Sınırlı',
  adequate: 'Yeterli',
  strong: 'Güçlü',
  excellent: 'Üstün',
  developing: 'Geliştirilebilir',
  star_1: 'Geliştirilebilir',
  good: 'İyi',
  moderate: 'İyi',
  star_2: 'İyi',
  very_good: 'Çok iyi',
  star_3: 'Çok iyi',
  performance_not_shown: 'Performans gösterilmedi',
  not_evaluated: 'Değerlendirilmedi',
  not_observed: 'Değerlendirilmedi',
};

export function speakingSubindicatorLabel(id: string): string {
  return subindicatorLabels[id] ?? 'Değerlendirme alt göstergesi';
}

export function speakingPerformanceLevelLabel(id: string): string {
  return performanceLevelLabels[id] ?? 'Belirsiz';
}

export function getStarScore(level: SpeakingPerformanceLevel, maxScore: number): number | null {
  const maxInt = Math.round(maxScore);
  switch (level) {
    case 'very_good':
    case 'star_3':
      return maxInt;
    case 'good':
    case 'moderate':
    case 'star_2':
      if (maxInt === 5) return 4;
      if (maxInt === 10) return 7;
      if (maxInt === 15) return 11;
      if (maxInt === 20) return 14;
      return Math.round(maxScore * 0.7);
    case 'developing':
    case 'star_1':
      if (maxInt === 5) return 2;
      if (maxInt === 10) return 4;
      if (maxInt === 15) return 6;
      if (maxInt === 20) return 8;
      return Math.round(maxScore * 0.4);
    case 'performance_not_shown':
      return 0;
    case 'not_observed':
    case 'not_evaluated':
    default:
      return null;
  }
}

export function levelToStarCount(level?: SpeakingPerformanceLevel | null): number {
  if (!level) return 0;
  switch (level) {
    case 'very_good':
    case 'star_3':
      return 3;
    case 'good':
    case 'moderate':
    case 'star_2':
      return 2;
    case 'developing':
    case 'star_1':
      return 1;
    default:
      return 0;
  }
}

export const STAR_DESCRIPTIONS: Record<number, { label: string; level: SpeakingPerformanceLevel }> = {
  1: { label: 'Geliştirilebilir', level: 'developing' },
  2: { label: 'İyi', level: 'good' },
  3: { label: 'Çok iyi', level: 'very_good' },
};

export function speakingSubindicatorMaximum(criterionId: string): number {
  return criterionId === 'content_main_idea' ? 4 : 3;
}

export function speakingMeasurementConfidenceLabel(
  confidence: 'high' | 'medium' | 'low' | 'not_evaluated',
): string {
  return {
    high: 'Yüksek güven',
    medium: 'Orta güven',
    low: 'Sınırlı güven',
    not_evaluated: 'Ölçülmedi',
  }[confidence];
}

export function speakingPipelineState(
  isCapturing: boolean,
  isCompleted: boolean,
  evaluationState?: 'draft' | 'recording' | 'paused' | 'finalizing' | 'cleaning_transcript' | 'evaluating' | 'teacher_review' | 'approved' | 'cancelled' | 'failed',
  cleanupStatus?: 'not_started' | 'running' | 'accepted' | 'needs_review' | 'failed' | 'pending' | 'succeeded',
  hasAiScores?: boolean,
  evaluationError?: string | null,
): SpeakingPipelineState {
  if (isCapturing) {
    return {
      whisper: { done: true, label: 'Canlı' },
      cleanup: { done: false, label: 'Bekliyor' },
      rubric: { done: false, label: 'Bekliyor' },
    };
  }
  if (isCompleted) {
    const cleanupFailed = cleanupStatus === 'failed' || cleanupStatus === 'needs_review';
    const cleanupSucceeded = cleanupStatus === 'succeeded' || cleanupStatus === 'accepted';
    const rubricInProgress = evaluationState === 'finalizing' || evaluationState === 'cleaning_transcript' || evaluationState === 'evaluating';
    const rubricDone = cleanupSucceeded && !rubricInProgress && hasAiScores === true && !evaluationError;
    return {
      whisper: { done: true, label: 'Tamamlandı' },
      cleanup: {
        done: cleanupSucceeded,
        label: evaluationState === 'cleaning_transcript'
          ? 'Düzeltiliyor'
          : cleanupFailed || evaluationState === 'failed'
            ? 'İnceleme gerekli'
            : evaluationState === 'finalizing'
              ? 'Bekliyor'
              : 'Tamamlandı',
      },
      rubric: {
        done: rubricDone,
        label: cleanupFailed
          ? 'Bekliyor'
          : evaluationState === 'evaluating'
            ? 'Değerlendiriliyor'
            : rubricInProgress
              ? 'Bekliyor'
              : evaluationError || hasAiScores === false
                ? 'İnceleme gerekli'
                : 'Tamamlandı',
      },
    };
  }
  return {
    whisper: { done: false, label: 'Hazır' },
    cleanup: { done: false, label: 'Bekliyor' },
    rubric: { done: false, label: 'Bekliyor' },
  };
}

export function getFluencyExplanation(
  metrics?: SpeakingMetrics | null,
  score?: number | null,
): string {
  if (
    !metrics ||
    metrics.durationMs === 0 ||
    metrics.measurementConfidence === 'not_evaluated' ||
    score === null ||
    score === undefined
  ) {
    return 'Ölçüm bekleniyor';
  }
  const parts: string[] = [];

  if (metrics.wordsPerMinute >= 80 && metrics.wordsPerMinute <= 180) {
    parts.push('Konuşma hızı uygun');
  } else if (metrics.wordsPerMinute < 80) {
    parts.push('Konuşma hızı yavaş');
  } else {
    parts.push('Konuşma hızı yüksek');
  }

  const details: string[] = [];
  if (metrics.longPauseCount > 0) {
    details.push(
      metrics.longPauseCount === 1
        ? 'bir uzun duraklama'
        : metrics.longPauseCount === 2
          ? 'iki uzun duraklama'
          : `${metrics.longPauseCount} uzun duraklama`,
    );
  }
  if (metrics.fillerCount > 0) {
    details.push(
      metrics.fillerCount === 1
        ? 'sınırlı dolgu ifadesi'
        : `${metrics.fillerCount} dolgu ifadesi`,
    );
  }
  if (metrics.repetitionCount > 0) {
    details.push(
      metrics.repetitionCount === 1
        ? 'sınırlı tekrar'
        : `${metrics.repetitionCount} belirgin tekrar`,
    );
  }

  if (details.length === 0) {
    parts.push('belirgin duraklama ve tekrar tespit edilmedi');
  } else {
    parts.push(`${details.join(' ve ')} tespit edildi`);
  }

  return `${parts.join('; ')}.`;
}

export function getDurationDeviationLabel(
  durationMs?: number,
  minDurationSeconds?: number,
  maxDurationSeconds?: number,
): string {
  if (
    durationMs === undefined ||
    durationMs === null ||
    durationMs === 0 ||
    !minDurationSeconds ||
    !maxDurationSeconds
  ) {
    return 'Ölçüm bekleniyor';
  }
  const durationSeconds = Math.round(durationMs / 1000);
  if (durationSeconds >= minDurationSeconds && durationSeconds <= maxDurationSeconds) {
    return 'Hedef aralık içinde';
  }
  if (durationSeconds < minDurationSeconds) {
    const ratio = (minDurationSeconds - durationSeconds) / minDurationSeconds;
    const percent = Math.round(ratio * 100);
    return `Alt sınırdan %${percent} kısa`;
  }
  const ratio = (durationSeconds - maxDurationSeconds) / maxDurationSeconds;
  const percent = Math.round(ratio * 100);
  return `Üst sınırdan %${percent} uzun`;
}

export function formatMsToDuration(ms?: number): string {
  if (ms === undefined || ms === null || !Number.isFinite(ms) || ms === 0) return '—';
  const seconds = Math.max(0, Math.round(ms / 1000));
  const mins = Math.floor(seconds / 60);
  const remSecs = seconds % 60;
  if (mins > 0) {
    return `${mins} dk ${remSecs} sn`;
  }
  return `${remSecs} sn`;
}

export function validateSpeakingDuration(
  minSec: number,
  recSec: number,
  maxSec: number,
): { valid: boolean; message?: string } {
  if (minSec <= 0 || recSec <= 0 || maxSec <= 0) {
    return { valid: false, message: 'Süre değerleri 0’dan büyük olmalıdır.' };
  }
  if (minSec > recSec) {
    return { valid: false, message: 'Alt sınır önerilen süreden büyük olamaz.' };
  }
  if (recSec > maxSec) {
    return { valid: false, message: 'Önerilen süre üst sınırdan büyük olamaz.' };
  }
  if (minSec >= maxSec) {
    return { valid: false, message: 'Üst sınır alt sınırdan büyük olmalıdır.' };
  }
  return { valid: true };
}

export function formatMinSecDisplay(minSec: number, maxSec: number): string {
  const minMins = Math.floor(minSec / 60);
  const minRemainder = minSec % 60;
  const maxMins = Math.floor(maxSec / 60);
  const maxRemainder = maxSec % 60;

  const minStr = minRemainder > 0 ? `${minMins} dk ${minRemainder} sn` : `${minMins} dk`;
  const maxStr = maxRemainder > 0 ? `${maxMins} dk ${maxRemainder} sn` : `${maxMins} dk`;
  return `${minStr} – ${maxStr}`;
}

export function getSpeakingSetupReadiness(
  title: string,
  taskText: string,
  selectedClassIds: string[],
  minSec: number,
  recSec: number,
  maxSec: number,
): { isReady: boolean; missingReasons: string[] } {
  const missingReasons: string[] = [];

  if (!title.trim()) {
    missingReasons.push('Sınav adını girin');
  }
  if (selectedClassIds.length === 0) {
    missingReasons.push('En az bir sınıf seçin');
  }
  if (!taskText.trim()) {
    missingReasons.push('Görev metnini girin');
  }

  const durationValidation = validateSpeakingDuration(minSec, recSec, maxSec);
  if (!durationValidation.valid) {
    missingReasons.push(durationValidation.message || 'Süre ayarlarını kontrol edin');
  }

  return {
    isReady: missingReasons.length === 0,
    missingReasons,
  };
}


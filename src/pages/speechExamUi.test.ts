import assert from 'node:assert/strict';
import test from 'node:test';
import {
  formatMinSecDisplay,
  formatMsToDuration,
  getDurationDeviationLabel,
  getFluencyExplanation,
  getSpeakingSetupReadiness,
  speakingMeasurementConfidenceLabel,
  speakingPerformanceLevelLabel,
  speakingPipelineState,
  speakingSubindicatorLabel,
  speakingSubindicatorMaximum,
  validateSpeakingDuration,
} from './speechExamUi.ts';

test('speaking pipeline shows Whisper live before cleanup and rubric evaluation', () => {
  assert.deepEqual(speakingPipelineState(true, false), {
    whisper: { done: true, label: 'Canlı' },
    cleanup: { done: false, label: 'Bekliyor' },
    rubric: { done: false, label: 'Bekliyor' },
  });
});

test('speaking pipeline exposes cleanup before rubric evaluation', () => {
  assert.deepEqual(speakingPipelineState(false, true, 'cleaning_transcript'), {
    whisper: { done: true, label: 'Tamamlandı' },
    cleanup: { done: false, label: 'Düzeltiliyor' },
    rubric: { done: false, label: 'Bekliyor' },
  });
});

test('speaking pipeline blocks rubric scoring after cleanup validation fails', () => {
  assert.deepEqual(speakingPipelineState(false, true, 'teacher_review', 'failed'), {
    whisper: { done: true, label: 'Tamamlandı' },
    cleanup: { done: false, label: 'İnceleme gerekli' },
    rubric: { done: false, label: 'Bekliyor' },
  });
});

test('rubric done requires AI scores to be present', () => {
  const state = speakingPipelineState(false, true, 'teacher_review', 'succeeded', false, null);
  assert.equal(state.rubric.done, false);
  assert.equal(state.rubric.label, 'İnceleme gerekli');
});

test('rubric done with AI scores and no error shows Tamamlandı', () => {
  const state = speakingPipelineState(false, true, 'teacher_review', 'succeeded', true, null);
  assert.equal(state.rubric.done, true);
  assert.equal(state.rubric.label, 'Tamamlandı');
});

test('rubric shows review on evaluation error even with AI scores', () => {
  const state = speakingPipelineState(false, true, 'teacher_review', 'succeeded', true, 'Reconciliation failed');
  assert.equal(state.rubric.done, false);
  assert.equal(state.rubric.label, 'İnceleme gerekli');
});

test('rubric shows Değerlendiriliyor during evaluation', () => {
  const state = speakingPipelineState(false, true, 'evaluating', 'succeeded', false, null);
  assert.equal(state.rubric.done, false);
  assert.equal(state.rubric.label, 'Değerlendiriliyor');
});

test('approved state with AI scores shows rubric done', () => {
  const state = speakingPipelineState(false, true, 'approved', 'succeeded', true, null);
  assert.equal(state.rubric.done, true);
  assert.equal(state.rubric.label, 'Tamamlandı');
});

test('idle state shows all waiting', () => {
  const state = speakingPipelineState(false, false);
  assert.equal(state.whisper.label, 'Hazır');
  assert.equal(state.cleanup.label, 'Bekliyor');
  assert.equal(state.rubric.label, 'Bekliyor');
});

test('teacher-facing speaking labels hide raw technical IDs', () => {
  assert.equal(speakingSubindicatorLabel('task_relevance'), 'Göreve uygunluk');
  assert.equal(speakingSubindicatorLabel('examples_reasons'), 'Örnek ve gerekçe kullanımı');
  assert.equal(speakingPerformanceLevelLabel('strong'), 'Güçlü');
  assert.equal(speakingSubindicatorMaximum('content_main_idea'), 4);
  assert.equal(speakingSubindicatorMaximum('speech_structure'), 3);
});

test('short sample confidence has a teacher-facing Turkish label', () => {
  assert.equal(speakingMeasurementConfidenceLabel('low'), 'Sınırlı güven');
});

test('3-star performance level labels map to Turkish teacher labels', () => {
  assert.equal(speakingPerformanceLevelLabel('star_1'), 'Geliştirilebilir');
  assert.equal(speakingPerformanceLevelLabel('developing'), 'Geliştirilebilir');
  assert.equal(speakingPerformanceLevelLabel('star_2'), 'İyi');
  assert.equal(speakingPerformanceLevelLabel('good'), 'İyi');
  assert.equal(speakingPerformanceLevelLabel('star_3'), 'Çok iyi');
  assert.equal(speakingPerformanceLevelLabel('very_good'), 'Çok iyi');
  assert.equal(speakingPerformanceLevelLabel('performance_not_shown'), 'Performans gösterilmedi');
  assert.equal(speakingPerformanceLevelLabel('not_evaluated'), 'Değerlendirilmedi');
});

test('getFluencyExplanation returns Ölçüm bekleniyor when metrics or score is null', () => {
  assert.equal(getFluencyExplanation(null, null), 'Ölçüm bekleniyor');
  assert.equal(getFluencyExplanation({ durationMs: 0 } as any, null), 'Ölçüm bekleniyor');
});

test('getFluencyExplanation formats readable Turkish summary when metrics present', () => {
  const metrics: any = {
    durationMs: 72000,
    wordsPerMinute: 110,
    longPauseCount: 2,
    fillerCount: 0,
    repetitionCount: 1,
  };
  const explanation = getFluencyExplanation(metrics, 4);
  assert.equal(explanation, 'Konuşma hızı uygun; iki uzun duraklama ve sınırlı tekrar tespit edildi.');
});

test('getDurationDeviationLabel computes exact deviation percentages', () => {
  assert.equal(getDurationDeviationLabel(0, 120, 180), 'Ölçüm bekleniyor');
  assert.equal(getDurationDeviationLabel(150000, 120, 180), 'Hedef aralık içinde');
  assert.equal(getDurationDeviationLabel(72000, 120, 180), 'Alt sınırdan %40 kısa');
  assert.equal(getDurationDeviationLabel(216000, 120, 180), 'Üst sınırdan %20 uzun');
});

test('formatMsToDuration formats duration in minutes and seconds', () => {
  assert.equal(formatMsToDuration(0), '—');
  assert.equal(formatMsToDuration(72000), '1 dk 12 sn');
  assert.equal(formatMsToDuration(45000), '45 sn');
});

test('validateSpeakingDuration validates duration bounds', () => {
  assert.deepEqual(validateSpeakingDuration(0, 180, 240), {
    valid: false,
    message: 'Süre değerleri 0’dan büyük olmalıdır.',
  });
  assert.deepEqual(validateSpeakingDuration(200, 180, 240), {
    valid: false,
    message: 'Alt sınır önerilen süreden büyük olamaz.',
  });
  assert.deepEqual(validateSpeakingDuration(120, 250, 240), {
    valid: false,
    message: 'Önerilen süre üst sınırdan büyük olamaz.',
  });
  assert.deepEqual(validateSpeakingDuration(120, 180, 240), { valid: true });
});

test('formatMinSecDisplay formats minute and second ranges', () => {
  assert.equal(formatMinSecDisplay(120, 240), '2 dk – 4 dk');
  assert.equal(formatMinSecDisplay(150, 270), '2 dk 30 sn – 4 dk 30 sn');
});

test('getSpeakingSetupReadiness requires title, taskText, selectedClassIds and valid duration', () => {
  const invalid = getSpeakingSetupReadiness('', '', [], 120, 180, 240);
  assert.equal(invalid.isReady, false);
  assert.deepEqual(invalid.missingReasons, [
    'Sınav adını girin',
    'En az bir sınıf seçin',
    'Görev metnini girin',
  ]);

  const valid = getSpeakingSetupReadiness(
    'Konuşma Sınavı',
    'Konuşma görevi metni',
    ['class_1'],
    120,
    180,
    240,
  );
  assert.equal(valid.isReady, true);
  assert.deepEqual(valid.missingReasons, []);
});

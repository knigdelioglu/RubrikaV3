export const optionalGuidanceText = 'Bu alan opsiyoneldir. Öğrenci cevapları okunduktan sonra AI ile zenginleştirilebilir.';

export const optionalGuidanceLabels = {
  partialCreditHints: 'Kısmi puan ipuçları',
  zeroScoreConditions: 'Sıfır puan koşulları',
  commonMistakes: 'Yaygın yanlışlar',
} as const;

export function optionalGuidanceEmptyText(field: keyof typeof optionalGuidanceLabels): string {
  const suffix = field === 'commonMistakes'
    ? 'Öğrenci cevapları okunduktan sonra çıkarılabilir.'
    : 'Öğrenci cevaplarından sonra öneri olarak üretilebilir.';
  return `${optionalGuidanceLabels[field]}: Henüz eklenmedi. ${suffix}`;
}

import type { DataLossPreflightReport } from '../api/types';

export function isProjectWriteBlocked(
  report: Pick<DataLossPreflightReport, 'decision' | 'initializationWriteAllowed'> | undefined,
  state: { isLoading: boolean; isError: boolean },
): boolean {
  return state.isLoading
    || state.isError
    || !report
    || (report.decision === 'DO_NOT_OPEN_FOR_WRITING' && !report.initializationWriteAllowed);
}

export function isProjectWriteControl(dataProjectWrite: string | null): boolean {
  return dataProjectWrite !== 'false';
}

export const WRITE_BLOCK_GATE_EXPRESSION =
  "isLoading || isError || !report || (report.decision === 'DO_NOT_OPEN_FOR_WRITING' && !report.initializationWriteAllowed)";

export function preflightReasonLabel(reason: string): string {
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

export type WriteBlockReasonInput = {
  report: DataLossPreflightReport | undefined;
  state: { isLoading: boolean; isError: boolean };
};

export function resolveWriteBlockReason({
  report,
  state,
}: WriteBlockReasonInput): string | null {
  if (state.isLoading) {
    return 'Veri güvenliği ön kontrolü henüz tamamlanmadı; yazma işlemleri tamamlanana kadar bekletiliyor.';
  }
  if (state.isError || !report) {
    return 'Veri güvenliği ön kontrolü alınamadı; yazma işlemleri koruma amacıyla bekletiliyor.';
  }
  if (isProjectWriteBlocked(report, state)) {
    const firstBlocker = report.blockers?.[0];
    if (firstBlocker) {
      return preflightReasonLabel(firstBlocker);
    }
    return 'Proje için veri güvenliği ön koşulu sağlanmadı.';
  }
  return null;
}

export type ModelPrivacyStatusForUi = {
  mode?: 'external' | 'managed' | null;
  privacyBlocked?: boolean | null;
};

export type ModelPrivacyWarning = {
  visible: boolean;
  title: string;
  body: string;
};

const externalPrivacyWarning: Omit<ModelPrivacyWarning, 'visible'> = {
  title: 'Harici model kullanımı açık bir gizlilik kararıdır',
  body:
    'Bu seçim öğrenci cevapları, ses kayıtları veya değerlendirme verilerinin yerel bilgisayar dışındaki bir model sunucusuna gönderilmesine izin verebilir. Yalnızca veri aktarımını ve ilgili gizlilik riskini anladıysanız onaylayın.',
};

export function getModelPrivacyWarning(
  status: ModelPrivacyStatusForUi | null | undefined,
): ModelPrivacyWarning {
  return {
    visible: status?.mode === 'external' || status?.privacyBlocked === true,
    ...externalPrivacyWarning,
  };
}

export function canConfirmExternalModel(consentGiven: boolean, pending: boolean): boolean {
  return consentGiven && !pending;
}

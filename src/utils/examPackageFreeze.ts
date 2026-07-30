import type { ProjectSnapshot, WorkflowSnapshot } from '../api/types';

export type ExamPackageFreezeUiState = {
  frozen: boolean;
  readyToFreeze: boolean;
  statusText: string;
  nextStepText: string;
  freezeButtonDisabledReason?: string;
};

export function getExamPackageFreezeUiState(
  project: Pick<ProjectSnapshot, 'examPackageFreeze'> | undefined,
  workflow: Pick<WorkflowSnapshot, 'summary'> | null | undefined,
  freezeInProgress: boolean,
): ExamPackageFreezeUiState {
  const frozen = project?.examPackageFreeze?.freezeStatus === 'frozen';
  const readyToFreeze = !frozen && workflow?.summary.readiness.examPackageFreeze === true;

  if (frozen) {
    return {
      frozen: true,
      readyToFreeze: false,
      statusText: 'Sınav paketi donduruldu',
      nextStepText: 'Sonraki adım: Öğrenci PDF’i yükleyin.',
      freezeButtonDisabledReason: 'Sınav paketi zaten donduruldu.',
    };
  }

  if (freezeInProgress) {
    return {
      frozen: false,
      readyToFreeze,
      statusText: readyToFreeze ? 'Sınav paketi donduruluyor' : 'Sınav paketi hazırlanıyor',
      nextStepText: 'Dondurma işleminin tamamlanmasını bekleyin.',
      freezeButtonDisabledReason: 'İşlem sürüyor.',
    };
  }

  return {
    frozen: false,
    readyToFreeze,
    statusText: readyToFreeze ? 'Sınav paketi dondurmaya hazır' : 'Sınav paketi dondurmaya hazır değil',
    nextStepText: readyToFreeze
      ? 'Sonraki adım: Sınav paketini dondurun.'
      : 'Sonraki adım: Eksik soru metni veya rubrikleri tamamlayın.',
    freezeButtonDisabledReason: readyToFreeze ? undefined : 'Backend dondurma hazırlığının tamamlanmadığını bildiriyor.',
  };
}

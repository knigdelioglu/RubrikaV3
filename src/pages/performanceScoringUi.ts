export type PerformanceActionAvailabilityInput = {
  rubricPresent: boolean;
  isApproved: boolean;
  isNonRatedStatus: boolean;
  hasSelectedAssessment: boolean;
  missingCriteriaCount: number;
  savePending: boolean;
  approvePending: boolean;
  statusPending: boolean;
};

export type PerformanceActionAvailability = {
  canApprove: boolean;
  canChangeStatus: boolean;
  canRevert: boolean;
  reason?: string;
};

export function derivePerformanceActionAvailability(
  input: PerformanceActionAvailabilityInput,
): PerformanceActionAvailability {
  const saveInFlight = input.savePending || input.approvePending || input.statusPending;

  const canApprove =
    input.rubricPresent &&
    !input.isApproved &&
    !input.isNonRatedStatus &&
    input.missingCriteriaCount === 0 &&
    input.hasSelectedAssessment &&
    !input.approvePending &&
    !input.savePending;

  const canChangeStatus =
    !input.isApproved && !input.isNonRatedStatus && !input.savePending && !input.statusPending;

  const canRevert =
    input.isNonRatedStatus && !input.isApproved && !input.savePending && !input.statusPending;

  const reason = !canApprove
    ? input.missingCriteriaCount > 0
      ? 'Tüm ölçütler seçilmeden onay verilemez.'
      : !input.hasSelectedAssessment
        ? 'Önce taslağı kaydedin.'
        : saveInFlight
          ? 'Kayıt sürüyor; işlem tamamlanmadan onaylanamaz.'
          : undefined
    : undefined;

  return {
    canApprove,
    canChangeStatus,
    canRevert,
    reason,
  };
}

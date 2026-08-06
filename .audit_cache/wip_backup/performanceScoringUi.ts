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
  const canApprove =
    input.rubricPresent &&
    !input.isApproved &&
    !input.isNonRatedStatus &&
    input.missingCriteriaCount === 0 &&
    input.hasSelectedAssessment &&
    !input.approvePending;

  const reason = !canApprove
    ? input.missingCriteriaCount > 0
      ? 'Tüm ölçütler seçilmeden onay verilemez.'
      : !input.hasSelectedAssessment
        ? 'Önce taslağı kaydedin.'
        : undefined
    : undefined;

  return {
    canApprove,
    canChangeStatus: !input.isApproved && !input.isNonRatedStatus,
    canRevert: input.isNonRatedStatus && !input.isApproved,
    reason,
  };
}

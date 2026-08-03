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

import type { WorkflowSnapshot } from '../../api/types';

export function getWorkflowSummaryText(workflow?: WorkflowSnapshot): string {
  return workflow?.summary?.text?.trim() || '';
}

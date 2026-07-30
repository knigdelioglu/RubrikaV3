// Core Tauri API wrapper that can be expanded with event listeners and logging
import { listen } from '@tauri-apps/api/event';
import type { Event } from '@tauri-apps/api/event';

export type JobStartedEvent = { jobId: string; kind: string };
export type JobProgressEvent = { jobId: string; current: number; total: number; message: string };
export type JobSucceededEvent = { jobId: string; result?: unknown };
export type JobFailedEvent = { jobId: string; error: unknown };
export type JobEvent = JobStartedEvent | JobProgressEvent | JobSucceededEvent | JobFailedEvent;

export function createIdempotentListenerCleanup(
  unlisteners: Array<() => void | Promise<void>>,
): () => void {
  let cleanedUp = false;
  return () => {
    if (cleanedUp) return;
    cleanedUp = true;
    void Promise.allSettled(unlisteners.map(async (unlisten) => unlisten()));
  };
}

export const tauriClient = {
  listenToJobEvents: async (callback: (event: Event<JobEvent>) => void) => {
    const unlistenStarted = await listen<JobEvent>('job_started', callback);
    const unlistenProgress = await listen<JobEvent>('job_progress', callback);
    const unlistenSucceeded = await listen<JobEvent>('job_succeeded', callback);
    const unlistenFailed = await listen<JobEvent>('job_failed', callback);
    return createIdempotentListenerCleanup([
      unlistenStarted,
      unlistenProgress,
      unlistenSucceeded,
      unlistenFailed,
    ]);
  },
};

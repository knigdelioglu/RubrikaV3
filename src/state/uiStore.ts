import { useSyncExternalStore } from 'react';

// A minimal UI store implementation if needed
let state = {
  sidebarOpen: true,
};
const listeners = new Set<() => void>();

export const uiStore = {
  getState() {
    return state;
  },
  toggleSidebar() {
    state = { ...state, sidebarOpen: !state.sidebarOpen };
    listeners.forEach((l) => l());
  },
  subscribe(listener: () => void) {
    listeners.add(listener);
    return () => listeners.delete(listener);
  },
};

export function useUiStore() {
  return useSyncExternalStore(uiStore.subscribe, uiStore.getState);
}

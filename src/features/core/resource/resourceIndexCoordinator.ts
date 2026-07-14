import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';

const INVALIDATION_DEBOUNCE_MS = 50;
const COMMAND_ECHO_SUPPRESSION_MS = 500;

let invalidationTimer: ReturnType<typeof setTimeout> | null = null;
let invalidationPromise: Promise<boolean> | null = null;
let lastCommandRefreshAt: number | null = null;
let lastCommandRefresh: Promise<boolean> | null = null;

function refreshResourceIndex(): Promise<boolean> {
  return useProjectIOStore.getState().refreshResourceIndex();
}

export function commitAfterCommand(): Promise<boolean> {
  lastCommandRefreshAt = Date.now();
  lastCommandRefresh = refreshResourceIndex();
  return lastCommandRefresh;
}

export function notifyIndexInvalidated(_source: 'event' | 'watcher'): Promise<boolean> {
  if (
    lastCommandRefreshAt !== null
    && Date.now() - lastCommandRefreshAt < COMMAND_ECHO_SUPPRESSION_MS
  ) {
    return lastCommandRefresh ?? Promise.resolve(true);
  }

  if (invalidationPromise) return invalidationPromise;

  invalidationPromise = new Promise((resolve) => {
    invalidationTimer = setTimeout(() => {
      invalidationTimer = null;
      void refreshResourceIndex()
        .then(resolve)
        .finally(() => {
          invalidationPromise = null;
        });
    }, INVALIDATION_DEBOUNCE_MS);
  });

  return invalidationPromise;
}

export function resetResourceIndexCoordinatorForTests(): void {
  if (invalidationTimer) clearTimeout(invalidationTimer);
  invalidationTimer = null;
  invalidationPromise = null;
  lastCommandRefreshAt = null;
  lastCommandRefresh = null;
}

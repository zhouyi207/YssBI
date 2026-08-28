import { useProjectIOStore } from '@/features/application/project/projectIOStore';
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';

const INVALIDATION_DEBOUNCE_MS = 50;

interface InvalidationState {
  readonly identity: ProjectIdentitySnapshot;
  latestVersion: number;
  timer: ReturnType<typeof setTimeout> | null;
  promise: Promise<boolean>;
}

const invalidationsByIdentity = new Map<string, InvalidationState>();

function identityKey(identity: ProjectIdentitySnapshot): string {
  return `${identity.projectInstanceId}:${identity.epoch}`;
}

function refreshResourceIndex(identity: ProjectIdentitySnapshot): Promise<boolean> {
  if (!isCurrentProjectIdentity(identity)) return Promise.resolve(false);
  return useProjectIOStore.getState().refreshResourceIndex();
}

async function refreshInvalidatedVersions(state: InvalidationState): Promise<boolean> {
  let result = false;
  while (isCurrentProjectIdentity(state.identity)) {
    const refreshingVersion = state.latestVersion;
    result = await refreshResourceIndex(state.identity);
    if (!isCurrentProjectIdentity(state.identity)) return false;
    if (state.latestVersion <= refreshingVersion) return result;
  }
  return false;
}

export function commitAfterCommand(): Promise<boolean> {
  return refreshResourceIndex(captureProjectIdentity());
}

export function notifyIndexInvalidated(
  identity: ProjectIdentitySnapshot,
  version: number,
): Promise<boolean> {
  if (!isCurrentProjectIdentity(identity)) return Promise.resolve(false);

  const key = identityKey(identity);
  const existing = invalidationsByIdentity.get(key);
  if (existing) {
    existing.latestVersion = Math.max(existing.latestVersion, version);
    return existing.promise;
  }

  const state: InvalidationState = {
    identity,
    latestVersion: version,
    timer: null,
    promise: Promise.resolve(false),
  };
  state.promise = new Promise<boolean>((resolve, reject) => {
    state.timer = setTimeout(() => {
      state.timer = null;
      void refreshInvalidatedVersions(state).then(resolve, reject);
    }, INVALIDATION_DEBOUNCE_MS);
  }).finally(() => {
    if (invalidationsByIdentity.get(key) === state) {
      invalidationsByIdentity.delete(key);
    }
  });
  invalidationsByIdentity.set(key, state);
  return state.promise;
}

export function resetResourceIndexCoordinatorForTests(): void {
  for (const state of invalidationsByIdentity.values()) {
    if (state.timer) clearTimeout(state.timer);
  }
  invalidationsByIdentity.clear();
}

import { useSyncExternalStore } from 'react';

import { uiStore } from '@/features/core/ui/UIStore';
import type { ApplicationUiState } from '@/shared/types/ui';

function getApplicationUiSnapshot(): ApplicationUiState {
  return uiStore.getState();
}

function subscribeApplicationUi(listener: () => void): () => void {
  return uiStore.subscribe(listener);
}

export function useApplicationUiRead(): ApplicationUiState {
  return useSyncExternalStore(
    subscribeApplicationUi,
    getApplicationUiSnapshot,
    getApplicationUiSnapshot,
  );
}

/** Global overlay read/actions exposed to App composition. Core owns the mutable store. */
export const applicationUi = {
  cancelProgress: () => uiStore.cancelProgress(),
  closeModal: (id: string) => uiStore.closeModal(id),
};

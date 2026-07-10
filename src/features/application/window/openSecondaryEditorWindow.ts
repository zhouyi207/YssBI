import {
  createPersistedWindow,
  type PersistedWindowOptions,
} from './createPersistedWindow';
import { createEphemeralWindowLabel } from './windowLabels';
import { readSecondaryWindowState } from './usePersistedSecondaryWindow';

export function buildSecondaryEditorWindowRequest(label: string): PersistedWindowOptions {
  return {
    geometry: {
      source: 'provided',
      state: readSecondaryWindowState(label),
    },
    label,
    url: 'index.html#/editor',
    title: 'YssBI Node Editor',
    visible: true,
  };
}

export async function openSecondaryEditorWindow(): Promise<void> {
  const label = createEphemeralWindowLabel('window');
  await createPersistedWindow(buildSecondaryEditorWindowRequest(label));
}

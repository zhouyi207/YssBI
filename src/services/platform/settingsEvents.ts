import { emit, listen } from '@tauri-apps/api/event';
import type { AppSettings } from '@/shared/types/settings';
import type { PlatformFailure, PlatformOutcome, PlatformUnsubscribe } from './platformTypes';

export const SETTINGS_CHANGED_EVENT = 'client-settings-updated';

function operationFailure(operation: 'publishSettingsChanged' | 'subscribeSettingsChanged'): PlatformFailure {
  return { operation, code: 'operationFailed' };
}

function invalidPayload(): PlatformFailure {
  return {
    operation: 'subscribeSettingsChanged',
    code: 'invalidResult',
    resultKind: 'eventPayload',
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isSettingsPayload(value: unknown): value is AppSettings {
  return isRecord(value)
    && Object.keys(value).length === 4
    && ['theme', 'editor', 'appearance', 'project'].every((key) => key in value)
    && Object.values(value).every(isRecord);
}

export async function publishSettingsChanged(settings: AppSettings): Promise<PlatformOutcome<void>> {
  try {
    await emit(SETTINGS_CHANGED_EVENT, settings);
    return { ok: true, value: undefined };
  } catch {
    return { ok: false, failure: operationFailure('publishSettingsChanged') };
  }
}

export async function subscribeSettingsChanged(
  listener: (outcome: PlatformOutcome<AppSettings>) => void,
): Promise<PlatformOutcome<PlatformUnsubscribe>> {
  try {
    const unlisten = await listen<unknown>(SETTINGS_CHANGED_EVENT, (event) => {
      listener(isSettingsPayload(event.payload)
        ? { ok: true, value: event.payload }
        : { ok: false, failure: invalidPayload() });
    });
    return { ok: true, value: unlisten };
  } catch {
    return { ok: false, failure: operationFailure('subscribeSettingsChanged') };
  }
}

export interface SettingsEvent {
  readonly projectInstanceId: string;
  readonly revision: number;
}

export interface SettingsEventSubscription {
  readonly subscribe: (listener: (event: SettingsEvent) => void) => () => void;
}

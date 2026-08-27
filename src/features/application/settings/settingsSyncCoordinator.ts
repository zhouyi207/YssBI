import type { SettingsEventSubscription } from '@/services/platform/settingsEvents';

export class SettingsSyncCoordinator {
  private unsubscribe: (() => void) | null = null;

  constructor(private readonly events: SettingsEventSubscription) {}

  start(onEvent: (event: Parameters<Parameters<SettingsEventSubscription['subscribe']>[0]>[0]) => void): void {
    this.unsubscribe = this.events.subscribe(onEvent);
  }

  stop(): void {
    this.unsubscribe?.();
    this.unsubscribe = null;
  }
}

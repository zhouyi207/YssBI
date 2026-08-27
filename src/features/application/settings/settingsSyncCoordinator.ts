import {
  publishSettingsChanged,
  subscribeSettingsChanged,
} from '@/services/platform/settingsEvents';
import {
  applyClientSettingsFromRemote,
  setClientSettingsPublisher,
} from '@/features/core/settings/settingsStore';

export class SettingsSyncCoordinator {
  private unsubscribe: (() => void) | null = null;
  private stopped = false;

  async start(): Promise<void> {
    this.stopped = false;
    setClientSettingsPublisher((settings) => {
      void publishSettingsChanged(settings);
    });

    const result = await subscribeSettingsChanged((outcome) => {
      if (!this.stopped && outcome.ok) applyClientSettingsFromRemote(outcome.value);
    });
    if (this.stopped) {
      if (result.ok) result.value();
      return;
    }
    if (result.ok) this.unsubscribe = result.value;
  }

  stop(): void {
    this.stopped = true;
    setClientSettingsPublisher(null);
    this.unsubscribe?.();
    this.unsubscribe = null;
  }
}

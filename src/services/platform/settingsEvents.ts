export interface SettingsEvent {
  readonly projectInstanceId: string;
  readonly revision: number;
}

export interface SettingsEventSubscription {
  readonly subscribe: (listener: (event: SettingsEvent) => void) => () => void;
}

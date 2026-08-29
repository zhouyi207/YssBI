import type {
  DialogOptions,
  ConfirmTriResult,
} from '@/shared/types/ui';
import { uiStore } from './UIStore';

export interface UiCapability {
  readonly confirm: (
    options: Omit<DialogOptions, 'onConfirm' | 'onCancel'>,
  ) => Promise<boolean>;
  readonly confirm3: (
    options: Omit<DialogOptions, 'onConfirm' | 'onCancel' | 'onDiscard'> & {
      discardText: string;
    },
  ) => Promise<ConfirmTriResult>;
}

/** View-safe modal actions; the mutable modal store remains private to Core. */
export const ui: UiCapability = {
  confirm: (options) => uiStore.confirm(options),
  confirm3: (options) => uiStore.confirm3(options),
};

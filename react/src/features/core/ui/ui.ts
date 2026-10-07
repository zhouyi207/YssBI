import type { DialogOptions, ConfirmTriResult } from "@/shared/types/ui";
import { uiStore } from "./UIStore";

export interface UiCapability {
  readonly showSettings: () => void;
  readonly confirm: (
    options: Omit<DialogOptions, "onConfirm" | "onCancel">,
    parentId?: string,
  ) => Promise<boolean>;
  readonly confirm3: (
    options: Omit<DialogOptions, "onConfirm" | "onCancel" | "onDiscard"> & {
      discardText: string;
    },
  ) => Promise<ConfirmTriResult>;
}

/** View-safe modal actions; the mutable modal store remains private to Core. */
export const ui: UiCapability = {
  showSettings: () => uiStore.showSettings(),
  confirm: (options, parentId) => uiStore.confirm(options, parentId),
  confirm3: (options) => uiStore.confirm3(options),
};

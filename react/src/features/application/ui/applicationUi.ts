import { useStore } from "zustand";

import { uiStore } from "@/features/core/ui/UIStore";
import type { ApplicationUiState } from "@/features/core/ui/applicationUiTypes";

export type {
  ImportDialogOptions,
  ImportDataSourceType,
  SqliteTableSelectDialogOptions,
  ExcelSheetSelectDialogOptions,
  SqlConnectionDialogOptions,
  SqlRemoteTableSelectDialogOptions,
} from "@/features/core/ui/applicationUiTypes";

export function useApplicationUiRead<T>(selector: (state: ApplicationUiState) => T): T {
  return useStore(uiStore, selector);
}

/** Global overlay read/actions exposed to App composition. Core owns the mutable store. */
export const applicationUi = {
  cancelProgress: () => uiStore.cancelProgress(),
  closeModal: (id: string) => {
    const { modals, progress } = uiStore.getState();
    if (!progress && modals[modals.length - 1]?.id === id) uiStore.closeModal(id);
  },
};

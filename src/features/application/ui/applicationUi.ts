import { useSyncExternalStore } from "react";

import { uiStore } from "@/features/core/ui/UIStore";
import type { ApplicationUiState } from "@/features/core/ui/applicationUiTypes";

type ModalOptions<Type extends ApplicationUiState["modals"][number]["type"]> = Extract<
  ApplicationUiState["modals"][number],
  { type: Type }
>["options"];

export type ImportDialogOptions = ModalOptions<"import">;
export type ImportDataSourceType = Parameters<ImportDialogOptions["onSelect"]>[0];
export type SqliteTableSelectDialogOptions = ModalOptions<"sqliteTableSelect">;
export type ExcelSheetSelectDialogOptions = ModalOptions<"excelSheetSelect">;
export type SqlConnectionDialogOptions = ModalOptions<"sqlConnection">;
export type SqlRemoteTableSelectDialogOptions = ModalOptions<"sqlRemoteTableSelect">;

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

import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import type { FileResourceKind } from "@/shared/types/domain/resource";
import { createFile } from "./fileManagement";
import { showBlockingIpcError } from "@/features/application/editor/blockingErrorDialog";
import { isEditorOpenRejectionHandled } from "@/features/application/editor/openEditorPanel";

export function useFileManagement() {
  const { t } = useTranslation();
  const create = useCallback(
    async (kind: FileResourceKind) => {
      try {
        await createFile(kind);
      } catch (error) {
        if (!isEditorOpenRejectionHandled(error))
          showBlockingIpcError(error, (code) => t("documents.operationFailed", { error: code }));
      }
    },
    [t],
  );
  return { createFile: create };
}

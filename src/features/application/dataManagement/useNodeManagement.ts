import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import type { NodeCreationDescriptor } from "@/features/domain/nodeCatalog/creationDescriptor";
import { createNodeFromDescriptor } from "@/features/application/nodeCatalog/createNodeFromDescriptor";
import { DEFAULT_LANGUAGE } from "@/shared/types/settings";
import {
  isEditorCommandTargetCurrent,
  captureActiveEditorCommandTarget,
  type EditorCommandTarget,
} from "@/features/application/editor/editorCommandFocus";

export function useNodeManagement() {
  const { i18n } = useTranslation();
  const locale = i18n.resolvedLanguage || i18n.language || DEFAULT_LANGUAGE;

  const createNode = useCallback(
    async (
      descriptor: NodeCreationDescriptor,
      position: { x: number; y: number },
      target?: EditorCommandTarget,
    ): Promise<boolean> => {
      const currentTarget = target ?? captureActiveEditorCommandTarget();
      if (
        !currentTarget ||
        (currentTarget.resourceKind !== "event" && currentTarget.resourceKind !== "function") ||
        !isEditorCommandTargetCurrent(currentTarget)
      )
        return false;
      const outcome = await createNodeFromDescriptor({
        graphPath: currentTarget.resourceRef,
        locale,
        descriptor,
        position,
        connectFrom: null,
      });
      return outcome.status === "applied";
    },
    [locale],
  );

  return { createNode };
}

import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import type { NodeCreationDescriptor } from "@/features/domain/nodeCatalog/creationDescriptor";
import { createNodeFromDescriptor } from "@/features/application/nodeCatalog/createNodeFromDescriptor";
import { DEFAULT_LANGUAGE } from "@/shared/types/settings";
import { useActiveGraphContext } from "@/features/application/editor/editorGroupContext";
import {
  isEditorCommandTargetCurrent,
  type EditorCommandTarget,
} from "@/features/application/editor/editorCommandFocus";

export function useNodeManagement() {
  const activeResourceRef = useActiveGraphContext()?.graphPath ?? null;
  const { i18n } = useTranslation();
  const locale = i18n.resolvedLanguage || i18n.language || DEFAULT_LANGUAGE;

  const createNode = useCallback(
    async (
      descriptor: NodeCreationDescriptor,
      position: { x: number; y: number },
      target?: EditorCommandTarget,
    ): Promise<boolean> => {
      const graphPath = target?.resourceRef ?? activeResourceRef;
      if (
        !graphPath ||
        (target &&
          ((target.resourceKind !== "event" && target.resourceKind !== "function") ||
            !isEditorCommandTargetCurrent(target)))
      )
        return false;
      const outcome = await createNodeFromDescriptor({
        graphPath,
        locale,
        descriptor,
        position,
        connectFrom: null,
      });
      return outcome.status === "applied";
    },
    [activeResourceRef, locale],
  );

  return { createNode };
}

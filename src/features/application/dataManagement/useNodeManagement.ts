import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import type { NodeCreationDescriptor } from "@/features/domain/nodeCatalog/creationDescriptor";
import { createNodeFromDescriptor } from "@/features/application/nodeCatalog/createNodeFromDescriptor";
import { DEFAULT_LANGUAGE } from "@/shared/types/settings";
import { useActiveEditorGroup } from "@/features/core/editor/hooks/useActiveEditorGroup";
import { executeCommand } from "@/features/core/history";
import { canDeleteNode } from "@/features/core/dataStore/graphNodeSelectors";
import { logger } from "@/features/application/observability/appLogger";
import {
  isEditorCommandTargetCurrent,
  type EditorCommandTarget,
} from "@/features/application/editor/editorCommandFocus";

export function useNodeManagement() {
  const { activeResourceRef } = useActiveEditorGroup();
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

  const deleteNode = useCallback(
    async (nodeId: string): Promise<boolean> => {
      if (!activeResourceRef) {
        logger.graph.warn("Cannot delete node: no active tab", "NodeManagement");
        return false;
      }
      if (!canDeleteNode(activeResourceRef, nodeId)) return false;

      try {
        return await executeCommand(activeResourceRef, "DeleteNodes", { nodeIds: [nodeId] });
      } catch (error) {
        logger.graph.error(
          `Failed to delete node: ${error instanceof Error ? error.message : String(error)}`,
          "NodeManagement",
        );
        return false;
      }
    },
    [activeResourceRef],
  );

  const deleteNodes = useCallback(
    async (nodeIds: string[]): Promise<string[]> => {
      if (!activeResourceRef || nodeIds.length === 0) return [];
      const deletableIds = nodeIds.filter((id) => canDeleteNode(activeResourceRef, id));
      if (deletableIds.length === 0) return [];

      try {
        const applied = await executeCommand(activeResourceRef, "DeleteNodes", {
          nodeIds: deletableIds,
        });
        return applied ? deletableIds : [];
      } catch (error) {
        logger.graph.error(
          `Failed to delete nodes: ${error instanceof Error ? error.message : String(error)}`,
          "NodeManagement",
        );
        return [];
      }
    },
    [activeResourceRef],
  );

  return { createNode, deleteNode, deleteNodes };
}

import { useCallback } from "react";
import { lookupGraphResourceKind, useResourceStore } from "@/features/core/resource";
import { useActiveEditorGroup } from "@/features/application/editor/editorGroupContext";
import { useEditorStore } from "@/features/core/editor/stores/useEditorStore";
import { PROJECT_TREE_CATEGORY_IDS, useSidebarStore } from "@/features/core/sidebar";
import { revealWorkbenchView } from "@/modules/workbench/public";
import {
  createVariableAction,
  deleteVariableAction,
  updateVariableAction,
} from "@/features/application/dataManagement/variableActions";

export interface VariableCreationOptions {
  graphScope?: {
    graphPath: string;
    graphType: "event" | "function";
  };
}

/**
 * Variable Management Hook
 * Binds UI state and delegates CRUD to variable application actions.
 */
export function useVariableManagement() {
  const { activeResourceRef, panels } = useActiveEditorGroup();
  const variablesGraphScopePath = useEditorStore((s) => s.variablesGraphScopePath);
  const localGraphPath = variablesGraphScopePath ?? activeResourceRef;
  const graphTypeFromPanel = localGraphPath
    ? panels.find((panel) => panel.metadata.resourceRef === localGraphPath)?.metadata.resourceKind
    : undefined;
  const graphTypeFromResource = useResourceStore((s) =>
    localGraphPath ? lookupGraphResourceKind(s.resources, localGraphPath) : undefined,
  );
  const rawType = graphTypeFromPanel || graphTypeFromResource;
  const graphType = (rawType === "event" || rawType === "function" ? rawType : undefined) as
    | "event"
    | "function"
    | undefined;

  const addVariable = useCallback(
    async (
      name?: string,
      type: string = "Int64",
      isGlobal: boolean = false,
      options?: VariableCreationOptions,
    ) => {
      const explicitGraphScope = options?.graphScope;
      const created = await createVariableAction({
        name,
        type,
        isGlobal,
        activeGraphPath: isGlobal ? null : (explicitGraphScope?.graphPath ?? localGraphPath),
        graphType: isGlobal ? undefined : (explicitGraphScope?.graphType ?? graphType),
      });
      if (created) {
        void revealWorkbenchView("project");
        const sidebar = useSidebarStore.getState();
        sidebar.setProjectTreeCategoriesExpanded(
          isGlobal
            ? [PROJECT_TREE_CATEGORY_IDS.variables, PROJECT_TREE_CATEGORY_IDS.globalVariables]
            : [PROJECT_TREE_CATEGORY_IDS.variables, PROJECT_TREE_CATEGORY_IDS.localVariables],
          true,
        );
      }
      return created;
    },
    [localGraphPath, graphType],
  );

  const updateVariable = useCallback(
    async (id: string, data: Parameters<typeof updateVariableAction>[1]) => {
      await updateVariableAction(id, data);
    },
    [],
  );

  const deleteVariable = useCallback(async (id: string) => {
    await deleteVariableAction(id);
  }, []);

  return {
    addVariable,
    updateVariable,
    deleteVariable,
  };
}

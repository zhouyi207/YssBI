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

export function useVariableManagement() {
  const { activeResourceRef, panels } = useActiveEditorGroup();
  const variablesGraphScopePath = useEditorStore((s) => s.variablesGraphScopePath);
  const localGraphPath = variablesGraphScopePath ?? activeResourceRef;
  const panelResourceKind = localGraphPath
    ? panels.find((panel) => panel.metadata.resourceRef === localGraphPath)?.metadata.resourceKind
    : undefined;
  const indexedGraphKind = useResourceStore((s) =>
    localGraphPath ? lookupGraphResourceKind(s.resources, localGraphPath) : undefined,
  );
  const resourceKind = panelResourceKind || indexedGraphKind;
  const graphType =
    resourceKind === "event" || resourceKind === "function" ? resourceKind : undefined;

  const addVariable = useCallback(
    async (
      name?: string,
      dataTypeKey: string = "Int64",
      isGlobal: boolean = false,
      options?: VariableCreationOptions,
    ) => {
      const explicitGraphScope = options?.graphScope;
      const variableId = await createVariableAction({
        name,
        dataTypeKey,
        isGlobal,
        activeGraphPath: isGlobal ? null : (explicitGraphScope?.graphPath ?? localGraphPath),
        graphType: isGlobal ? undefined : (explicitGraphScope?.graphType ?? graphType),
      });
      if (variableId) {
        void revealWorkbenchView("project");
        const sidebar = useSidebarStore.getState();
        sidebar.setProjectTreeCategoriesExpanded(
          isGlobal
            ? [PROJECT_TREE_CATEGORY_IDS.variables, PROJECT_TREE_CATEGORY_IDS.globalVariables]
            : [PROJECT_TREE_CATEGORY_IDS.variables, PROJECT_TREE_CATEGORY_IDS.localVariables],
          true,
        );
      }
      return variableId;
    },
    [localGraphPath, graphType],
  );

  return {
    addVariable,
    updateVariable: updateVariableAction,
    deleteVariable: deleteVariableAction,
  };
}

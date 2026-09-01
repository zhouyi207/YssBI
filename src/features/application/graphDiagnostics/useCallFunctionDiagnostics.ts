import { useMemo } from "react";
import { useGraphDataStore } from "@/features/core/dataStore/graphDataStore";
import { useResourceStore } from "@/features/core/resource";
import {
  countCallFunctionIssuesByGraph,
  isFunctionResourceAvailable,
  type CallFunctionIssue,
} from "@/features/domain/graphDiagnostics";
import { isCallFunctionNodeType } from "@/features/domain/nodeCatalog";

export function useCallFunctionIssueCountsByGraph(): Record<string, number> {
  const graphEntities = useGraphDataStore((s) => s.graphEntities);
  const resources = useResourceStore((s) => s.resources);

  return useMemo(
    () => countCallFunctionIssuesByGraph(graphEntities, resources),
    [graphEntities, resources],
  );
}

export function useCallFunctionIssue(
  graphPath: string | undefined,
  nodeId: string,
): CallFunctionIssue | null {
  const node = useGraphDataStore((s) =>
    graphPath ? s.getGraphNode(graphPath, nodeId) : undefined,
  );
  const targetPath =
    node && isCallFunctionNodeType(node.nodeType) ? node.subGraphPath?.trim() : undefined;
  const targetAvailable = useResourceStore((s) =>
    targetPath ? isFunctionResourceAvailable(s.resources, targetPath) : false,
  );

  return useMemo(() => {
    if (!graphPath || !node || !isCallFunctionNodeType(node.nodeType)) return null;
    if (!targetPath) {
      return { graphPath, nodeId: node.id, kind: "empty_target" };
    }
    if (!targetAvailable) {
      return {
        graphPath,
        nodeId: node.id,
        kind: "missing_target",
        subGraphPath: targetPath,
      };
    }
    return null;
  }, [graphPath, node, targetAvailable, targetPath]);
}

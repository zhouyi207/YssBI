import {
  applyGraphDraftMutation,
  type ApplyGraphDraftMutationOutcome,
} from "@/features/application/graphDraft/graphDraftCoordinator";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";

export interface SetNodeParametersInput {
  graphPath: string;
  nodeId: string;
  locale: string;
  parameters: Record<string, unknown>;
}

export function setNodeParameters(
  input: SetNodeParametersInput,
): Promise<ApplyGraphDraftMutationOutcome> {
  const projected = useGraphProjectionStore.getState().getGraphNode(input.graphPath, input.nodeId);
  const merged = Object.fromEntries([
    ...(projected?.parameterEditors ?? []).map(
      (parameter) => [parameter.key, parameter.value] as const,
    ),
    ...Object.entries(input.parameters),
  ]);
  const parameters = Object.fromEntries(
    Object.entries(merged).filter(([, value]) => value !== null && value !== undefined),
  );
  return applyGraphDraftMutation({
    graphPath: input.graphPath,
    locale: input.locale,
    mutation: {
      type: "setParameters",
      payload: {
        nodeId: input.nodeId,
        parameters,
      },
    },
  });
}

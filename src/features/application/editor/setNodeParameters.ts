import {
  applyGraphDraftMutation,
  type ApplyGraphDraftMutationOutcome,
} from "@/features/application/graphDraft/graphDraftCoordinator";
import { useGraphDraftStore } from "@/features/core/graphDraft/graphDraftStore";

export interface SetNodeParametersInput {
  graphPath: string;
  nodeId: string;
  locale: string;
  parameters: Record<string, unknown>;
}

export function setNodeParameters(
  input: SetNodeParametersInput,
): Promise<ApplyGraphDraftMutationOutcome> {
  const document = useGraphDraftStore.getState().sessions[input.graphPath]?.document;
  const merged = { ...document?.nodes[input.nodeId]?.parameters, ...input.parameters };
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

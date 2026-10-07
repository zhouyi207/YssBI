import {
  applyGraphMutation,
  type ApplyGraphMutationOutcome,
} from "@/features/application/graphEditing/graphEditCoordinator";

export interface SetNodeParametersInput {
  graphPath: string;
  nodeId: string;
  locale: string;
  parameters: Record<string, unknown>;
}

export function setNodeParameters(
  input: SetNodeParametersInput,
): Promise<ApplyGraphMutationOutcome> {
  return applyGraphMutation({
    graphPath: input.graphPath,
    locale: input.locale,
    mutation: {
      type: "setParameters",
      payload: { nodeId: input.nodeId, parameters: input.parameters },
    },
  });
}

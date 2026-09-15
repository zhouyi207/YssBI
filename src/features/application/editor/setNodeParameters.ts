import {
  applyGraphMutation,
  type ApplyGraphMutationOutcome,
} from "@/features/application/graphEditing/graphEditCoordinator";
import { useGraphEditingStore } from "@/features/core/graphEditing/graphEditingStore";

export interface SetNodeParametersInput {
  graphPath: string;
  nodeId: string;
  locale: string;
  parameters: Record<string, unknown>;
}

export function setNodeParameters(
  input: SetNodeParametersInput,
): Promise<ApplyGraphMutationOutcome> {
  const document = useGraphEditingStore.getState().sessions[input.graphPath]?.document;
  const merged = { ...document?.nodes[input.nodeId]?.parameters, ...input.parameters };
  const parameters = Object.fromEntries(
    Object.entries(merged).filter(([, value]) => value !== null && value !== undefined),
  );
  return applyGraphMutation({
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

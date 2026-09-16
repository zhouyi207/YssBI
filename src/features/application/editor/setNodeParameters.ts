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
    mutation: (document) => ({
      type: "setParameters",
      payload: {
        nodeId: input.nodeId,
        parameters: Object.fromEntries(
          Object.entries({
            ...document.nodes[input.nodeId]?.parameters,
            ...input.parameters,
          }).filter(([, value]) => value !== null && value !== undefined),
        ),
      },
    }),
  });
}

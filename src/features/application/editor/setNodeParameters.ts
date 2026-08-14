import {
  executeEditorMutation,
  type ExecuteEditorMutationOutcome,
} from '@/features/application/editorMutation/editorMutationCoordinator';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';

export interface SetNodeParametersInput {
  graphPath: string;
  nodeId: string;
  locale: string;
  parameters: Record<string, unknown>;
}

export function setNodeParameters(
  input: SetNodeParametersInput,
): Promise<ExecuteEditorMutationOutcome> {
  const projected = useGraphDataStore.getState().getGraphNode(input.graphPath, input.nodeId);
  const merged = Object.fromEntries([
    ...(projected?.parameterEditors ?? []).map((parameter) => [parameter.key, parameter.value] as const),
    ...Object.entries(input.parameters),
  ]);
  const parameters = Object.fromEntries(
    Object.entries(merged).filter(([, value]) => value !== null && value !== undefined),
  );
  return executeEditorMutation({
    graphPath: input.graphPath,
    locale: input.locale,
    mutation: {
      type: 'setParameters',
      payload: {
        nodeId: input.nodeId,
        parameters,
      },
    },
  });
}

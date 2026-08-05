import {
  executeEditorMutation,
  type ExecuteEditorMutationOutcome,
} from '@/features/application/editorMutation/editorMutationCoordinator';

export interface SetNodeParametersInput {
  graphPath: string;
  nodeId: string;
  locale: string;
  parameters: Record<string, unknown>;
}

export function setNodeParameters(
  input: SetNodeParametersInput,
): Promise<ExecuteEditorMutationOutcome> {
  return executeEditorMutation({
    graphPath: input.graphPath,
    locale: input.locale,
    mutation: {
      type: 'setParameters',
      payload: {
        nodeId: input.nodeId,
        parameters: input.parameters,
      },
    },
  });
}

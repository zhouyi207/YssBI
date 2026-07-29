import { commitFunctionSignature } from '@/features/application/editorMutation/functionSignatureCoordinator';

export async function updateFunctionSignature(
  functionPath: string,
  patch: import('@/shared/types').FunctionSignaturePatch,
): Promise<void> {
  if (!patch.inputs && !patch.outputs) return;

  await commitFunctionSignature(functionPath, patch);
}

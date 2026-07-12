import type { EditorFunctions, EditorVariables } from '@/features/core/editor';
import { useVariableStore } from '@/features/core/dataStore';

export function isVariableAvailable(
  variableId: string,
  variables: EditorVariables,
): boolean {
  if (variableId in variables) return true;
  return variableId in useVariableStore.getState().variables;
}

export function isFunctionAvailable(
  functionId: string,
  functions: EditorFunctions,
): boolean {
  return functionId in functions;
}

import type { EditorGraphMutationDto } from '@/shared/types/dto/editorMutation';
import { executeGraphMutation } from '../graphMutationPort';

export function executeGraphIntent(graphPath: string, mutation: EditorGraphMutationDto) {
  return executeGraphMutation(graphPath, mutation);
}

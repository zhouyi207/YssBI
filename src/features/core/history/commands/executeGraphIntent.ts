import { executeEditorMutation } from '@/features/application/editorMutation/editorMutationCoordinator';
import { currentProjectionLocale } from '@/features/application/editorProjection/graphProjectionCoordinator';
import type { EditorGraphMutationDto } from '@/shared/types/dto/editorMutation';

export function executeGraphIntent(graphPath: string, mutation: EditorGraphMutationDto) {
  return executeEditorMutation({
    graphPath,
    locale: currentProjectionLocale(),
    mutation,
  });
}

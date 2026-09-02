import type { EditorGraphMutationDto } from "@/shared/types/domain/editorMutation";
import { executeGraphDraftMutation } from "../graphDraftPort";

export function executeGraphDraftIntent(graphPath: string, mutation: EditorGraphMutationDto) {
  return executeGraphDraftMutation(graphPath, mutation);
}

import type { EditorGraphMutationDto } from "@/shared/types/domain/editorMutation";
import { executeGraphMutation } from "../graphMutationPort";

export function executeGraphIntent(graphPath: string, mutation: EditorGraphMutationDto) {
  return executeGraphMutation(graphPath, mutation);
}

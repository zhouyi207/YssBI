import type { EditorGraphMutationDto } from "@/shared/types/domain/editorMutation";
import type { GraphMutationCommandResult } from "./types";

export interface GraphMutationPort {
  execute(graphPath: string, mutation: EditorGraphMutationDto): Promise<GraphMutationCommandResult>;
}

let port: GraphMutationPort | null = null;

export function registerGraphMutationPort(next: GraphMutationPort): void {
  port = next;
}

export function executeGraphMutation(graphPath: string, mutation: EditorGraphMutationDto) {
  if (!port) throw new Error("Graph mutation port is not registered");
  return port.execute(graphPath, mutation);
}

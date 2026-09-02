import type { EditorGraphMutationDto } from "@/shared/types/domain/editorMutation";
import type { GraphDraftCommandResult } from "./types";

export interface GraphDraftPort {
  execute(graphPath: string, mutation: EditorGraphMutationDto): Promise<GraphDraftCommandResult>;
}

let port: GraphDraftPort | null = null;

export function registerGraphDraftPort(next: GraphDraftPort): void {
  port = next;
}

export function executeGraphDraftMutation(graphPath: string, mutation: EditorGraphMutationDto) {
  if (!port) throw new Error("Graph draft port is not registered");
  return port.execute(graphPath, mutation);
}

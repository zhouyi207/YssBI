import type { GraphDocumentPatchDto } from "@/shared/types/domain/editorMutation";

export function insertedNodeIdsFromPatch(patch: GraphDocumentPatchDto): string[] {
  return patch.operations.flatMap((operation) =>
    operation.operation === "insert_node" ? [operation.node.id] : [],
  );
}

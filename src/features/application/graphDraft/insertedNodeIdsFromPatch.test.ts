import { describe, expect, it } from "vitest";
import type { GraphDocumentOperationDto } from "@/shared/types/domain/editorMutation";
import { insertedNodeIdsFromPatch } from "./insertedNodeIdsFromPatch";

const node = (id: string) => ({
  id,
  node_type: "tests.node",
  position: { x: 0, y: 0 },
  parameters: {},
  user_label: null,
});

describe("insertedNodeIdsFromPatch", () => {
  it("returns inserted draft node IDs in operation order", () => {
    const operations: GraphDocumentOperationDto[] = [
      { operation: "insert_node", node: node("node-b") },
      { operation: "insert_node", node: node("node-a") },
      { operation: "remove_node", node: node("removed") },
    ];
    expect(insertedNodeIdsFromPatch({ operations })).toEqual(["node-b", "node-a"]);
  });
});

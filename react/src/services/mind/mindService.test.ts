import { describe, expect, it, vi } from "vitest";
import { MindService, parseMindSnapshot } from "./mindService";
import { invokeCommand } from "@/services/ipc";

vi.mock("@/services/ipc", () => ({ invokeCommand: vi.fn() }));

const snapshot = {
  projectInstanceId: "project-a",
  path: "minds/Plan.yssbi-mind",
  version: { sessionId: "editor-a", revision: 1 },
  dirty: true,
  kind: "mind",
  content: { rootId: "root", nodes: [{ id: "root", parentId: null, content: "Plan" }] },
};

describe("authored document transport", () => {
  it("accepts domain documents and rejects renderer state or mismatched file kinds", () => {
    expect(parseMindSnapshot(snapshot)).toEqual(snapshot);
    expect(() => parseMindSnapshot({ ...snapshot, path: "docs/Plan.md" })).toThrow();
    for (const field of [{ measured: { width: 200 } }, { position: { x: 10, y: 20 } }]) {
      const rendererState = structuredClone(snapshot);
      Object.assign(rendererState.content.nodes[0], field);
      expect(() => parseMindSnapshot(rendererState)).toThrow();
    }
  });

  it("parses a revisioned document edit receipt through the shared resource publication contract", async () => {
    const operationId = "00000000-0000-0000-0000-000000000123";
    const state = { path: snapshot.path, kind: "mind", name: "Plan" };
    vi.mocked(invokeCommand).mockResolvedValueOnce({
      snapshot,
      mutation: {
        operationId,
        projectInstanceId: "project-a",
        publicationRevision: 2,
        moves: [],
        projectionReplacements: [],
        projectionStatus: { status: "complete", expectedGraphPaths: [] },
        deltas: [
          {
            resource: { kind: "mind", key: snapshot.path },
            fromRevision: 0,
            toRevision: 1,
            causedBy: operationId,
            payload: {
              kind: "resource_lifecycle",
              patch: { before: { ...state, revision: 0 }, after: { ...state, revision: 1 } },
            },
          },
        ],
      },
    });
    const result = await MindService.command("project-a", operationId, {
      op: "edit",
      path: snapshot.path,
      version: { sessionId: "editor-a", revision: 0 },
      edits: [{ op: "set_content", nodeId: "root", content: "Plan" }],
    });
    expect(result.snapshot?.version.revision).toBe(1);
    expect(invokeCommand).toHaveBeenCalledWith("edit_project_mind", {
      projectInstanceId: "project-a",
      operationId,
      command: {
        op: "edit",
        path: snapshot.path,
        version: { sessionId: "editor-a", revision: 0 },
        edits: [{ op: "set_content", nodeId: "root", content: "Plan" }],
      },
    });
    expect(result.mutation.deltas[0].resource.kind).toBe("mind");
  });

  it("admits only one connected tree before publishing a Mind snapshot", () => {
    const root = { id: "root", parentId: null, content: "Plan" };
    const invalidTrees = [
      [{ ...root, parentId: "root" }],
      [root, { id: "other", parentId: null, content: "Other root" }],
      [root, { id: "a", parentId: "b", content: "A" }, { id: "b", parentId: "a", content: "B" }],
    ];
    for (const nodes of invalidTrees) {
      expect
        .soft(() => parseMindSnapshot({ ...snapshot, content: { rootId: "root", nodes } }))
        .toThrow("Invalid mind hierarchy");
    }

    const nodes = Array.from({ length: 5_000 }, (_, index) => ({
      id: String(index),
      parentId: index === 0 ? null : String(index - 1),
      content: "Topic",
    })).reverse();
    const content = { rootId: "0", nodes };
    expect(parseMindSnapshot({ ...snapshot, content }).content).toBe(content);
  });
});

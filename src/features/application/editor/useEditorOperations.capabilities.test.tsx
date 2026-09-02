// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ClipboardSubgraphDto } from "@/shared/types/dto/clipboardSubgraph";
import type { GraphDraftCommandResult } from "@/features/core/history/types";
import { useEditorOperations } from "./useEditorOperations";
import { EDITOR_MUTATION_CAPABILITIES } from "./editorMutationAvailability";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  activeRole: "editor" as "editor" | "result" | "view",
  activeGroupId: "group-a" as string | null,
  activeResourceByGroup: new Map<string, string>(),
  selectionByGroup: new Map<string, string[]>(),
  canCopyNode: vi.fn((_graphPath: string, _nodeId: string) => true),
  canDeleteNode: vi.fn((_graphPath: string, _nodeId: string) => true),
  exportEditorSubgraph: vi.fn(),
  writeGraphClipboard: vi.fn(),
  readGraphClipboard: vi.fn(),
  executeCommandWithResult: vi.fn(),
  updateSelectedConnectionIds: vi.fn(),
  setInspectionContext: vi.fn(),
  revealInspect: vi.fn(async () => undefined),
  graphError: vi.fn(),
}));

vi.mock("./editorCommandFocus", () => ({
  captureActiveEditorCommandTarget: () => {
    const groupId = mocks.activeGroupId;
    const resourceRef = groupId ? mocks.activeResourceByGroup.get(groupId) : undefined;
    if (mocks.activeRole !== "editor" || !groupId || !resourceRef) return null;
    return {
      panelInstanceId: `editor-${groupId}`,
      groupId,
      resourceRef,
      resourceKind: "event" as const,
    };
  },
  isEditorCommandTargetCurrent: (target: {
    panelInstanceId: string;
    groupId: string;
    resourceRef: string;
  }) =>
    mocks.activeRole === "editor" &&
    mocks.activeGroupId === target.groupId &&
    target.panelInstanceId === `editor-${target.groupId}` &&
    mocks.activeResourceByGroup.get(target.groupId) === target.resourceRef,
}));
vi.mock("@/modules/workbench/public", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/modules/workbench/public")>()),
  getEditorGroupGraphSelection: (groupId: string) => ({
    nodeIds: new Set(mocks.selectionByGroup.get(groupId) ?? []),
    connectionIds: new Set<string>(),
  }),
  updateEditorGroupSelectedNodeIds: (
    value: string[] | ((previous: string[]) => string[]),
    groupId = "group-a",
  ) => {
    const previous = mocks.selectionByGroup.get(groupId) ?? [];
    const nodeIds = typeof value === "function" ? value(previous) : value;
    mocks.selectionByGroup.set(groupId, nodeIds);
    return { groupId, nodeIds };
  },
  updateEditorGroupSelectedConnectionIds: mocks.updateSelectedConnectionIds,
}));
vi.mock("@/modules/workbench/internal/dockview", () => ({
  workbenchDockviewRead: {
    getActiveEditorPanelInGroup: (groupId: string) => {
      const resourceRef = mocks.activeResourceByGroup.get(groupId);
      return resourceRef
        ? {
            panelInstanceId: `editor-${groupId}`,
            groupId,
            component: "EditorResource" as const,
            metadata: { role: "editor" as const, resourceRef, resourceKind: "event" as const },
            active: true,
            location: { type: "grid" as const },
          }
        : undefined;
    },
  },
}));
vi.mock("@/features/core/dataStore/graphNodeSelectors", () => ({
  canCopyNode: mocks.canCopyNode,
  canDeleteNode: mocks.canDeleteNode,
  canCutNode: (graphPath: string, nodeId: string) =>
    mocks.canCopyNode(graphPath, nodeId) && mocks.canDeleteNode(graphPath, nodeId),
}));
vi.mock("@/features/application/graphDraft/subgraphExportCoordinator", () => ({
  exportEditorSubgraph: mocks.exportEditorSubgraph,
}));
vi.mock("@/services/clipboard", () => ({
  writeGraphClipboard: mocks.writeGraphClipboard,
  readGraphClipboard: mocks.readGraphClipboard,
}));
vi.mock("@/features/core/history", () => ({
  executeCommand: vi.fn(),
  executeCommandWithResult: mocks.executeCommandWithResult,
}));
vi.mock("@/features/application/graphDraft/historyCoordinator", () => ({
  undoEditorHistory: vi.fn(),
  redoEditorHistory: vi.fn(),
}));
vi.mock("@/features/application/graphDraft/safeGraphDraftEdit", () => ({
  executeSafeGraphDraftEdit: vi.fn(),
}));
vi.mock("./edgeOperations", () => ({ disconnectConnectionsById: vi.fn() }));
vi.mock("@/features/core/dataStore/graphProjectionStore", () => ({
  useGraphProjectionStore: { getState: vi.fn(() => ({ getGraphNodePins: () => [] })) },
}));
vi.mock("./rightSidebarActions", () => ({
  setInspectionContext: mocks.setInspectionContext,
  revealInspect: mocks.revealInspect,
}));
vi.mock("@/features/application/observability/appLogger", () => ({
  logger: {
    graph: { error: mocks.graphError },
  },
}));

const graphPath = "events/main.yssbi-event";
const snapshot: ClipboardSubgraphDto = {
  schemaVersion: 1,
  nodes: [],
  portBindings: [],
  inputStates: [],
  connections: [],
};

function appliedWithInserted(...nodeIds: string[]): GraphDraftCommandResult {
  return {
    status: "applied",
    result: {
      document: {
        nodes: {},
        port_bindings: [],
        connections: {},
        input_states: [],
      },
      patch: {
        operations: nodeIds.map((id) => ({
          operation: "insert_node" as const,
          node: {
            id,
            node_type: "tests.node",
            position: { x: 0, y: 0 },
            parameters: {},
            user_label: null,
          },
        })),
      },
      projectionReplacement: {} as never,
    },
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

describe("useEditorOperations authoritative subgraph workflows", () => {
  let root: Root;
  let operations!: ReturnType<typeof useEditorOperations>;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.canCopyNode.mockReset();
    mocks.canDeleteNode.mockReset();
    mocks.exportEditorSubgraph.mockReset();
    mocks.writeGraphClipboard.mockReset();
    mocks.readGraphClipboard.mockReset();
    mocks.executeCommandWithResult.mockReset();
    mocks.updateSelectedConnectionIds.mockReset();
    mocks.setInspectionContext.mockReset();
    mocks.revealInspect.mockReset();
    mocks.revealInspect.mockResolvedValue(undefined);
    mocks.graphError.mockReset();
    mocks.activeRole = "editor";
    mocks.activeGroupId = "group-a";
    mocks.activeResourceByGroup = new Map([["group-a", graphPath]]);
    mocks.selectionByGroup = new Map([["group-a", ["node-a", "node-b"]]]);
    mocks.canCopyNode.mockReturnValue(true);
    mocks.canDeleteNode.mockReturnValue(true);
    mocks.exportEditorSubgraph.mockResolvedValue(snapshot);
    mocks.writeGraphClipboard.mockResolvedValue(undefined);
    mocks.readGraphClipboard.mockResolvedValue(snapshot);
    root = createRoot(document.createElement("div"));
    function Harness() {
      operations = useEditorOperations();
      return null;
    }
    act(() => root.render(<Harness />));
  });

  afterEach(() => {
    act(() => root.unmount());
  });

  it("enables paste and duplicate capabilities", () => {
    expect(EDITOR_MUTATION_CAPABILITIES).toMatchObject({ pasteNodes: true, duplicateNodes: true });
  });

  it("passively synchronizes connection selection without revealing Inspect", () => {
    mocks.updateSelectedConnectionIds.mockReturnValueOnce({
      groupId: "group-a",
      connectionIds: ["connection-a"],
    });

    act(() => operations.setSelectedConnectionIds(["connection-a", "connection-a"], "group-a"));

    expect(mocks.updateSelectedConnectionIds).toHaveBeenCalledWith(
      ["connection-a", "connection-a"],
      "group-a",
    );
    expect(mocks.setInspectionContext).toHaveBeenCalledWith(graphPath, []);
    expect(mocks.revealInspect).not.toHaveBeenCalled();
  });

  it("denies direct canvas operations while a Result is physically active", async () => {
    mocks.activeRole = "result";

    await act(async () => operations.copyNodes(["node-a", "node-b"]));

    expect(mocks.exportEditorSubgraph).not.toHaveBeenCalled();
    expect(mocks.writeGraphClipboard).not.toHaveBeenCalled();
  });

  it("copies by awaiting backend export before system clipboard write", async () => {
    const order: string[] = [];
    mocks.exportEditorSubgraph.mockImplementation(async () => {
      order.push("export");
      return snapshot;
    });
    mocks.writeGraphClipboard.mockImplementation(async () => {
      order.push("write");
    });

    await act(async () => operations.copyNodes(["node-a", "node-b"]));

    expect(order).toEqual(["export", "write"]);
    expect(mocks.exportEditorSubgraph).toHaveBeenCalledWith({
      graphPath,
      nodeIds: ["node-a", "node-b"],
    });
  });

  it("does not export unless every selected node is copyable", async () => {
    mocks.canCopyNode.mockImplementation((_path: string, id: string) => id !== "node-b");
    await act(async () => operations.copyNodes(["node-a", "node-b"]));
    expect(mocks.exportEditorSubgraph).not.toHaveBeenCalled();
  });

  it("cuts only after clipboard write, uses one DeleteNodes, then clears selection", async () => {
    const order: string[] = [];
    mocks.exportEditorSubgraph.mockImplementation(async () => {
      order.push("export");
      return snapshot;
    });
    mocks.writeGraphClipboard.mockImplementation(async () => {
      order.push("write");
    });
    mocks.executeCommandWithResult.mockImplementation(async () => {
      order.push("delete");
      return appliedWithInserted();
    });

    await act(async () => operations.cutNodes(["node-a", "node-b"]));

    expect(order).toEqual(["export", "write", "delete"]);
    expect(mocks.executeCommandWithResult).toHaveBeenCalledOnce();
    expect(mocks.executeCommandWithResult).toHaveBeenCalledWith(graphPath, "DeleteNodes", {
      nodeIds: ["node-a", "node-b"],
    });
    expect(mocks.selectionByGroup.get("group-a")).toEqual([]);
  });

  it("preserves selection and does not delete when clipboard writing fails", async () => {
    mocks.writeGraphClipboard.mockRejectedValueOnce(new Error("permission denied"));
    await act(async () => operations.cutNodes(["node-a", "node-b"]));
    expect(mocks.executeCommandWithResult).not.toHaveBeenCalled();
    expect(mocks.selectionByGroup.get("group-a")).toEqual(["node-a", "node-b"]);
    expect(mocks.graphError).toHaveBeenCalled();
  });

  it("retains clipboard and selection when cut deletion is not applied", async () => {
    mocks.executeCommandWithResult.mockResolvedValueOnce({ status: "conflict" });
    await act(async () => operations.cutNodes(["node-a", "node-b"]));
    expect(mocks.writeGraphClipboard).toHaveBeenCalledOnce();
    expect(mocks.selectionByGroup.get("group-a")).toEqual(["node-a", "node-b"]);
    expect(mocks.graphError).toHaveBeenCalled();
  });

  it("pastes one raw snapshotJson mutation and selects only committed insert_node IDs", async () => {
    mocks.executeCommandWithResult.mockResolvedValueOnce(appliedWithInserted("new-b", "new-a"));

    await act(async () => operations.paste({ x: 120, y: 240 }));

    expect(mocks.executeCommandWithResult).toHaveBeenCalledOnce();
    expect(mocks.executeCommandWithResult).toHaveBeenCalledWith(graphPath, "InsertSubgraph", {
      snapshotJson: JSON.stringify(snapshot),
      anchor: { x: 120, y: 240 },
    });
    expect(mocks.selectionByGroup.get("group-a")).toEqual(["new-b", "new-a"]);
  });

  it.each([{ status: "conflict" as const }, { status: "stale" as const }])(
    "preserves selection when paste settles as $status",
    async (outcome) => {
      mocks.executeCommandWithResult.mockResolvedValueOnce(outcome);
      await act(async () => operations.paste({ x: 1, y: 2 }));
      expect(mocks.selectionByGroup.get("group-a")).toEqual(["node-a", "node-b"]);
    },
  );

  it.each(["duplicate", "paste", "cut"] as const)(
    "preserves a newer node selection when %s settles in the same group and resource",
    async (operationType) => {
      const pending = deferred<GraphDraftCommandResult | null>();
      mocks.executeCommandWithResult.mockReturnValueOnce(pending.promise);

      const operation =
        operationType === "duplicate"
          ? operations.duplicateNodes(["node-a", "node-b"])
          : operationType === "paste"
            ? operations.paste({ x: 20, y: 30 })
            : operations.cutNodes(["node-a", "node-b"]);
      await vi.waitFor(() => expect(mocks.executeCommandWithResult).toHaveBeenCalledOnce());
      mocks.selectionByGroup.set("group-a", ["user-selected"]);
      pending.resolve(appliedWithInserted("committed-node"));
      await act(async () => operation);

      expect(mocks.activeResourceByGroup.get("group-a")).toBe(graphPath);
      expect(mocks.selectionByGroup.get("group-a")).toEqual(["user-selected"]);
    },
  );

  it("duplicates once and selects committed IDs only in the same active group and resource", async () => {
    mocks.executeCommandWithResult.mockResolvedValueOnce(appliedWithInserted("copy-a", "copy-b"));
    await act(async () => operations.duplicateNodes(["node-a", "node-b"]));
    expect(mocks.executeCommandWithResult).toHaveBeenCalledWith(graphPath, "DuplicateSubgraph", {
      nodeIds: ["node-a", "node-b"],
      offset: { x: 40, y: 40 },
    });
    expect(mocks.selectionByGroup.get("group-a")).toEqual(["copy-a", "copy-b"]);

    mocks.selectionByGroup.set("group-a", ["copy-a", "copy-b"]);
    const pending = deferred<GraphDraftCommandResult | null>();
    mocks.executeCommandWithResult.mockReturnValueOnce(pending.promise);
    const operation = operations.duplicateNodes(["copy-a"]);
    mocks.activeResourceByGroup.set("group-a", "events/other.yssbi-event");
    pending.resolve(appliedWithInserted("wrong-resource"));
    await act(async () => operation);
    expect(mocks.selectionByGroup.get("group-a")).toEqual(["copy-a", "copy-b"]);
  });

  it("never allocates graph identities in the frontend workflows", async () => {
    const randomId = vi.spyOn(crypto, "randomUUID");
    mocks.executeCommandWithResult.mockResolvedValueOnce(appliedWithInserted("backend-id"));
    await act(async () => operations.duplicateNodes(["node-a"]));
    expect(randomId).not.toHaveBeenCalled();
  });
});

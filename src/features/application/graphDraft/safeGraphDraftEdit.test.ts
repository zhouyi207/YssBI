import { beforeEach, describe, expect, it, vi } from "vitest";
import type { GraphDraftCommandInvocation } from "@/features/core/history/commandExecutor";
import { executeSafeGraphDraftEdit } from "./safeGraphDraftEdit";

const executeCommandOutcome = vi.hoisted(() => vi.fn());
const graphWarn = vi.hoisted(() => vi.fn());

vi.mock("@/features/core/history", () => ({ executeCommandOutcome }));
vi.mock("@/app/i18n", () => ({ i18n: { t: (key: string) => `localized:${key}` } }));
vi.mock("@/features/application/observability/appLogger", () => ({
  logger: {
    graph: { warn: graphWarn },
  },
}));

describe("executeSafeGraphDraftEdit", () => {
  beforeEach(() => vi.clearAllMocks());

  it("accepts graph mutation command args with a typed outcome boundary", () => {
    type Invocation = Parameters<typeof executeSafeGraphDraftEdit>;
    const invocation: Invocation = [
      "events/main",
      "Insert reroute",
      "InsertReroute",
      { connectionId: "edge-1", position: { x: 1, y: 2 } },
    ];

    expect(invocation[2]).toBe("InsertReroute");
  });

  it.each([
    [
      "Alt disconnect",
      "DisconnectPort",
      { pinId: "pin-1" },
      { status: "rejected", code: "graph_connection_type_mismatch" },
      "graph_connection_type_mismatch",
    ],
    [
      "Break all links",
      "DisconnectNode",
      { nodeId: "node-1" },
      { status: "rejected", code: "graph_node_not_found" },
      "graph_node_not_found",
    ],
    [
      "Delete nodes",
      "DeleteNodes",
      { nodeIds: ["node-1"] },
      { status: "rejected", code: "graph_managed_node_delete_forbidden" },
      "graph_managed_node_delete_forbidden",
    ],
  ] as const)("rejects %s without replay", async (operation, command, args, outcome, code) => {
    executeCommandOutcome.mockResolvedValue({
      ...outcome,
      message: "raw backend UUID 00000000-0000-0000-0000-000000000000",
    });

    const invocation: GraphDraftCommandInvocation =
      command === "DisconnectPort"
        ? ["DisconnectPort", { pinId: args.pinId }]
        : command === "DisconnectNode"
          ? ["DisconnectNode", { nodeId: args.nodeId }]
          : ["DeleteNodes", { nodeIds: [...args.nodeIds] }];

    await expect(executeSafeGraphDraftEdit("events/main", operation, ...invocation)).resolves.toBe(
      false,
    );

    expect(executeCommandOutcome).toHaveBeenCalledOnce();
    expect(executeCommandOutcome).toHaveBeenCalledWith("events/main", command, args);
    expect(graphWarn).toHaveBeenCalledWith(
      `Graph mutation outcome code=${code} graphPath=events/main operation=${operation}`,
      "GraphDraft",
    );
    expect(JSON.stringify(graphWarn.mock.calls)).not.toContain("raw backend");
    expect(JSON.stringify(graphWarn.mock.calls)).not.toContain("00000000");
  });
});

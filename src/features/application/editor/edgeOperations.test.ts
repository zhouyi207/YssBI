import { beforeEach, describe, expect, it, vi } from "vitest";
import { executeGraphEdit } from "@/features/application/graphEditing";
import {
  connectPinsById,
  disconnectConnectionById,
  disconnectConnectionsById,
  disconnectPinById,
  insertRerouteAtConnection,
} from "./edgeOperations";

vi.mock("@/features/application/graphEditing", () => ({
  executeGraphEdit: vi.fn(),
}));

const graphPath = "events/main.yssbi-event";

describe("edge operations", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it.each([[[]], [[""]], [["   "]], [["edge-1", ""]]] as const)(
    "rejects invalid disconnect IDs without a command call: %j",
    async (connectionIds) => {
      await expect(disconnectConnectionsById(graphPath, connectionIds)).resolves.toBe(false);

      expect(executeGraphEdit).not.toHaveBeenCalled();
    },
  );

  it("deduplicates disconnect IDs in first-seen order and sends one intent", async () => {
    vi.mocked(executeGraphEdit).mockResolvedValueOnce({
      status: "applied",
      result: {} as never,
      insertedNodeIds: [],
    });

    await expect(
      disconnectConnectionsById(graphPath, ["edge-b", "edge-a", "edge-b", "edge-c", "edge-a"]),
    ).resolves.toBe(true);

    expect(executeGraphEdit).toHaveBeenCalledTimes(1);
    expect(executeGraphEdit).toHaveBeenCalledWith(graphPath, "DisconnectConnections", {
      connectionIds: ["edge-b", "edge-a", "edge-c"],
    });
  });

  it("exposes detail connection operations through the existing graph commands", async () => {
    vi.mocked(executeGraphEdit).mockResolvedValue({
      status: "applied",
      result: {},
    } as never);

    await expect(connectPinsById(graphPath, "output-a", "input-b")).resolves.toEqual({
      status: "applied",
      result: {},
    });
    await expect(disconnectConnectionById(graphPath, "edge-a")).resolves.toEqual({
      status: "applied",
      result: {},
    });
    await expect(disconnectPinById(graphPath, "input-b")).resolves.toEqual({
      status: "applied",
      result: {},
    });

    expect(executeGraphEdit).toHaveBeenNthCalledWith(1, graphPath, "ConnectPins", {
      pinA: "output-a",
      pinB: "input-b",
    });
    expect(executeGraphEdit).toHaveBeenNthCalledWith(2, graphPath, "DisconnectConnections", {
      connectionIds: ["edge-a"],
    });
    expect(executeGraphEdit).toHaveBeenNthCalledWith(3, graphPath, "DisconnectPort", {
      pinId: "input-b",
    });
  });

  it.each([
    ["", { x: 120, y: 80 }],
    ["   ", { x: 120, y: 80 }],
    ["edge-1", { x: Infinity, y: 80 }],
    ["edge-1", { x: 120, y: -Infinity }],
    ["edge-1", { x: NaN, y: 80 }],
  ] as const)(
    "rejects invalid reroute input without a command call: %j %j",
    async (connectionId, position) => {
      await expect(insertRerouteAtConnection(graphPath, connectionId, position)).resolves.toEqual({
        status: "unavailable",
      });

      expect(executeGraphEdit).not.toHaveBeenCalled();
    },
  );

  it("copies the position and sends one typed InsertReroute intent", async () => {
    const outcome = { status: "saving" } as const;
    vi.mocked(executeGraphEdit).mockResolvedValueOnce(outcome);
    const position = { x: 120, y: 80 };

    const result = insertRerouteAtConnection(graphPath, "edge-1", position);
    position.x = 999;

    await expect(result).resolves.toEqual(outcome);
    expect(executeGraphEdit).toHaveBeenCalledTimes(1);
    expect(executeGraphEdit).toHaveBeenCalledWith(graphPath, "InsertReroute", {
      connectionId: "edge-1",
      position: { x: 120, y: 80 },
    });
  });

  it.each([
    { status: "applied", result: {} },
    { status: "noop", result: {} },
    { status: "stale" },
    { status: "saving" },
    { status: "rejected", code: "graph_connection_not_found" },
    { status: "failed" },
  ] as const)("propagates the typed reroute outcome unchanged: %j", async (outcome) => {
    vi.mocked(executeGraphEdit).mockResolvedValueOnce(outcome as never);

    await expect(insertRerouteAtConnection(graphPath, "edge-1", { x: 120, y: 80 })).resolves.toBe(
      outcome,
    );

    expect(executeGraphEdit).toHaveBeenCalledTimes(1);
  });
});

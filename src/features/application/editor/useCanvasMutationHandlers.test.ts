import { beforeEach, describe, expect, it, vi } from "vitest";
import { createCanvasMutationHandlers } from "./useCanvasMutationHandlers";

const executeSafeGraphDraftEditOutcome = vi.hoisted(() => vi.fn());
const insertRerouteAtConnection = vi.hoisted(() => vi.fn());
const graphWarn = vi.hoisted(() => vi.fn());

vi.mock("@/features/application/graphDraft/safeGraphDraftEdit", () => ({
  executeSafeGraphDraftEditOutcome,
}));
vi.mock("./edgeOperations", () => ({ insertRerouteAtConnection }));
vi.mock("@/features/application/graphDraft/registerGraphDraftPort", () => ({
  ensureGraphDraftPortRegistered: vi.fn(),
}));
vi.mock("@/features/application/observability/appLogger", () => ({
  logger: {
    graph: { warn: graphWarn },
  },
}));
vi.mock("@/app/i18n", () => ({ i18n: { t: (key: string) => `localized:${key}` } }));

describe("canvas mutation application wiring", () => {
  beforeEach(() => vi.clearAllMocks());

  it.each([
    ["connect", "Canvas connect", "ConnectPins", { pinA: "source", pinB: "target" }],
    [
      "moveConnections",
      "Canvas move connections",
      "MoveConnections",
      { sourcePinId: "source", targetPinId: "target" },
    ],
  ] as const)(
    "maps %s to one safe graph mutation intent",
    async (intent, operation, command, args) => {
      executeSafeGraphDraftEditOutcome.mockResolvedValueOnce({ status: "applied", result: {} });
      const handlers = createCanvasMutationHandlers();

      const outcome = await handlers.submitConnection({
        graphPath: "events/main",
        intent,
        sourcePinId: "source",
        targetPinId: "target",
      });

      expect(executeSafeGraphDraftEditOutcome).toHaveBeenCalledOnce();
      expect(executeSafeGraphDraftEditOutcome).toHaveBeenCalledWith(
        "events/main",
        operation,
        command,
        args,
      );
      expect(outcome).toEqual({ status: "applied" });
    },
  );

  it("maps Alt disconnect and reroute through application operations", async () => {
    executeSafeGraphDraftEditOutcome.mockResolvedValueOnce({ status: "applied", result: {} });
    insertRerouteAtConnection.mockResolvedValueOnce({ status: "applied", result: {} });
    const handlers = createCanvasMutationHandlers();

    await handlers.disconnectPort("events/main", "pin-a");
    await handlers.insertRerouteAtConnection({
      graphPath: "events/main",
      connectionId: "edge-a",
      position: { x: 25, y: 40 },
    });

    expect(executeSafeGraphDraftEditOutcome).toHaveBeenCalledWith(
      "events/main",
      "Alt disconnect port",
      "DisconnectPort",
      { pinId: "pin-a" },
    );
    expect(insertRerouteAtConnection).toHaveBeenCalledWith("events/main", "edge-a", {
      x: 25,
      y: 40,
    });
  });

  it("adapts application error codes to a safe core outcome", async () => {
    executeSafeGraphDraftEditOutcome.mockResolvedValueOnce({
      status: "rejected",
      code: "graph_connection_type_mismatch",
    });
    const handlers = createCanvasMutationHandlers();

    const outcome = await handlers.submitConnection({
      graphPath: "events/main",
      intent: "connect",
      sourcePinId: "source",
      targetPinId: "target",
    });
    expect(outcome).toEqual({
      status: "failed",
      message: "localized:canvas.connection.errors.graph_connection_type_mismatch",
    });

    if (outcome.status === "failed" && outcome.message) {
      handlers.reportMutationFailure({
        graphPath: "events/main",
        intent: "connect",
        message: outcome.message,
      });
    }
    expect(JSON.stringify(graphWarn.mock.calls)).not.toContain("graph_connection_type_mismatch");
  });
});

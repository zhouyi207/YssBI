import { invoke } from "@tauri-apps/api/core";
import { describe, expect, it, vi } from "vitest";

import editorProjection from "@/tests/fixtures/node-system-contracts/editor-projection.json";
import { GraphProjectionChannelService } from "./graphProjectionChannelService";

const channels: Array<{ onmessage: (value: unknown) => void }> = [];

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  Channel: class {
    onmessage = (_value: unknown) => undefined;

    constructor() {
      channels.push(this);
    }
  },
}));

const graphPath = "events/contract.yssbi-event";
const publication = {
  type: "projectionReplaced" as const,
  projectInstanceId: "project-a",
  graphSessionId: "graph-session-a",
  graphPath,
  requestGeneration: 2,
  replacement: { graphPath, projection: editorProjection },
};

describe("GraphProjectionChannelService", () => {
  it("buffers live events until the snapshot boundary is activated", async () => {
    vi.mocked(invoke).mockImplementation(async (command, args) => {
      if (command === "subscribe_graph_projections") {
        const channel = (args as { onEvents: { onmessage: (value: unknown) => void } }).onEvents;
        channel.onmessage(publication);
        return {
          subscriptionId: "subscription-a",
          snapshot: {
            projectInstanceId: "project-a",
            streamId: "stream-a",
            projections: [],
            latestGenerationByGraph: {},
          },
        };
      }
      return undefined;
    });
    const received: unknown[] = [];

    const subscription = await GraphProjectionChannelService.subscribe(
      "project-a",
      (event) => received.push(event),
      vi.fn(),
    );

    expect(received).toEqual([]);
    subscription.activate();
    expect(received).toEqual([publication]);
    await subscription.unsubscribe();
    expect(invoke).toHaveBeenLastCalledWith("unsubscribe_graph_projections", {
      subscriptionId: "subscription-a",
    });
    expect(channels).toHaveLength(1);
  });
});

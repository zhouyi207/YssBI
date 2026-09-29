import { invoke } from "@tauri-apps/api/core";
import { describe, expect, it, vi } from "vitest";
import { getConnectionCandidates } from "./connectionCandidatesService";
import type {
  ConnectionCandidates,
  ConnectionCandidatesRequest,
} from "@/shared/types/domain/connectionCandidates";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const request: ConnectionCandidatesRequest = {
  projectInstanceId: "project",
  graphPath: "events/Main.yssbi-event",
  version: { sessionId: "00000000-0000-0000-0000-000000000090", revision: "4" },
  sourcePort: {
    kind: "declared",
    nodeId: "00000000-0000-0000-0000-000000000101",
    portKey: "value",
  },
  intent: "connect",
};
const response: ConnectionCandidates = {
  ...request,
  semanticInputHash: "a".repeat(64),
  candidates: [
    {
      port: { kind: "declared", nodeId: "00000000-0000-0000-0000-000000000102", portKey: "left" },
      decision: {
        kind: "replace",
        displacedConnectionIds: ["00000000-0000-0000-0000-000000000103"],
      },
    },
  ],
};

describe("connection candidate transport", () => {
  it("requests a versioned backend projection and preserves replacement decisions", async () => {
    vi.mocked(invoke).mockResolvedValue(response);
    await expect(getConnectionCandidates(request)).resolves.toEqual(response);
    expect(invoke).toHaveBeenLastCalledWith("get_connection_candidates", { request });
  });

  it("rejects mismatched identities and malformed or duplicate decisions", async () => {
    for (const invalid of [
      { ...response, version: { ...request.version, revision: "3" } },
      { ...response, graphPath: "events/Other.yssbi-event" },
      { ...response, projectInstanceId: "other" },
      { ...response, sourcePort: response.candidates[0].port },
      { ...response, intent: "moveConnections" },
      { ...response, candidates: [...response.candidates, ...response.candidates] },
      {
        ...response,
        candidates: [
          { ...response.candidates[0], decision: { kind: "replace", displacedConnectionIds: [] } },
        ],
      },
      { ...response, candidates: [{ ...response.candidates[0], decision: { kind: "invalid" } }] },
    ]) {
      vi.mocked(invoke).mockResolvedValue(invalid);
      await expect(getConnectionCandidates(request)).rejects.toThrow();
    }
  });
});

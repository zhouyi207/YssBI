import * as invalidationChannel from "@/services/result/resultInvalidationChannel";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ResultDescriptor, ResultPage } from "@/shared/types/domain/result";
import type { RunEventKind } from "@/shared/types/domain/runEvent";
import { ResultService } from "@/services/result/resultService";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { useExecutionStore } from "@/features/core/execution";
import { makeEditorProjectionFixture } from "@/tests/helpers/editorProjectionFixtures";
import {
  observeResultRunEvent,
  readPinResultStatus,
  resetResultQueryProject,
  resultQueryCoordinator,
  resultQueryRead,
} from "./runtime";

const graphPath = "events/Main.yssbi-event";
const fixture = makeEditorProjectionFixture({ graphPath });
const output = { graphPath, port: fixture.outputAddress };
const request = { graphPath, output: output.port };
const descriptor = (id: string): ResultDescriptor => ({
  resultId: id,
  state: { kind: "ready" },
  provenance: {
    runId: id,
    activationId: id,
    createdAtMs: "1000",
    graphPath,
    nodeId: output.port.nodeId,
    output,
  },
  presentation: { kind: "inspector" },
  valueKind: "sequence",
  totalCount: 1,
  metadata: null,
  title: "Result",
});
const page = (id: string): ResultPage => ({
  resultId: id,
  offset: 0,
  requestedLimit: 200,
  actualCount: 1,
  totalCount: 1,
  hasMore: false,
  nextOffset: null,
  valueKind: "sequence",
  metadata: null,
  values: [[1]],
});
function event(runId: string, kind: RunEventKind) {
  observeResultRunEvent({ run: { runId, graphPath, projectSessionId: "session" }, kind });
}

beforeEach(() => {
  vi.restoreAllMocks();
  resetResultQueryProject();
  useExecutionStore.setState({ graphs: {} });
  useProjectIOStore.setState({ projectInstanceId: "project-1" });
  useGraphProjectionStore.getState().replaceProjection(graphPath, fixture.projection);
});

describe("current result lifecycle", () => {
  it("evicts values and pages on rerun, rejects late requests, and publishes only the current result", async () => {
    const publishInvalidation = vi.spyOn(invalidationChannel, "publishResultInvalidation");
    vi.spyOn(ResultService, "getPinResult").mockResolvedValue(descriptor("1"));
    vi.spyOn(ResultService, "getValue").mockResolvedValue({ kind: "sequence", value: [[1]] });
    vi.spyOn(ResultService, "getPage").mockResolvedValue(page("1"));
    await resultQueryCoordinator.loadPinResult(request);
    await resultQueryCoordinator.loadValue({ resultId: "1" });
    const pageRequest = { resultId: "1", offset: 0, limit: 200 };
    await resultQueryCoordinator.loadPage(pageRequest);
    let settle!: (value: ResultPage) => void;
    vi.mocked(ResultService.getPage).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          settle = resolve;
        }),
    );
    const pending = resultQueryCoordinator.loadPage(pageRequest);
    event("2", { type: "runStarted", outputs: [output] });
    expect(resultQueryRead.getDescriptor("1")).toBeNull();
    expect(resultQueryRead.getValue("1")).toBeNull();
    expect(resultQueryRead.getPage(pageRequest)).toBeNull();
    expect(resultQueryRead.getPinResult(request)).toBeNull();
    expect(readPinResultStatus(request)).toBe("running");
    expect(publishInvalidation).toHaveBeenCalledWith(["1"]);
    settle(page("1"));
    await expect(pending).resolves.toEqual({ status: "stale" });
    expect(resultQueryRead.getPage(pageRequest)).toBeNull();
    vi.mocked(ResultService.getPinResult).mockResolvedValue(descriptor("2"));
    event("2", { type: "runCompleted" });
    await vi.waitFor(() => expect(resultQueryRead.getPinResult(request)?.resultId).toBe("2"));
    event("1", { type: "runStarted", outputs: [output] });
    expect(resultQueryRead.getPinResult(request)?.resultId).toBe("2");
    event("3", { type: "runStarted", outputs: [output] });
    event("3", { type: "runCancelled" });
    expect(resultQueryRead.getPinResult(request)).toBeNull();
    expect(resultQueryRead.getDescriptor("2")).toBeNull();
    expect(readPinResultStatus(request)).toBe("cancelled");
    vi.mocked(ResultService.getPinResult).mockResolvedValue(descriptor("1"));
    observeResultRunEvent({
      run: { runId: "1", graphPath, projectSessionId: "new-session" },
      kind: { type: "runStarted", outputs: [output] },
    });
    expect(readPinResultStatus(request)).toBe("running");
    observeResultRunEvent({
      run: { runId: "1", graphPath, projectSessionId: "new-session" },
      kind: { type: "runCompleted" },
    });
    await vi.waitFor(() => expect(resultQueryRead.getPinResult(request)?.resultId).toBe("1"));
    expect(resultQueryRead.getDescriptor("2")).toBeNull();
  });

  it("releases results when the graph semantic inputs change or its projection is unloaded", async () => {
    vi.spyOn(ResultService, "getPinResult").mockResolvedValue(descriptor("1"));
    await resultQueryCoordinator.loadPinResult(request);
    const changed = structuredClone(fixture.projection);
    changed.basis.semanticInputHash = "1".repeat(64);
    useGraphProjectionStore.getState().replaceProjection(graphPath, changed);
    expect(resultQueryRead.getPinResult(request)).toBeNull();
    expect(resultQueryRead.getDescriptor("1")).toBeNull();
    await resultQueryCoordinator.loadPinResult(request);
    useGraphProjectionStore.getState().clearGraph(graphPath);
    expect(resultQueryRead.getDescriptor("1")).toBeNull();
    expect(useExecutionStore.getState().graphs[graphPath]?.pinResults.size).toBe(0);
  });
});

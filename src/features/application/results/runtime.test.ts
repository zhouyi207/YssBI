import { resultSessionFixture, resultReferenceFixture } from "@/tests/helpers/resultFixture";
import * as invalidationChannel from "@/services/result/resultSessionChannel";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ResultDescriptor, ResultPage } from "@/shared/types/domain/result";
import type { RunEventKind } from "@/shared/types/domain/runEvent";
import { ResultService } from "@/services/result/resultService";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
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

  executionSessionId: resultSessionFixture,
  provenance: {
    runId: id,
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
  observeResultRunEvent({
    run: { runId, graphPath, executionSessionId: resultSessionFixture },
    kind,
  });
}

beforeEach(() => {
  vi.restoreAllMocks();
  resetResultQueryProject();
  useExecutionStore.setState({ graphs: {} });
  startProjectLifecycle("project-1");
  useGraphProjectionStore.getState().replaceProjection(graphPath, fixture.projection);
});

describe("current result lifecycle", () => {
  it("evicts values and pages on rerun, rejects late requests, and publishes only the current result", async () => {
    const publishInvalidation = vi.spyOn(invalidationChannel, "publishResultSessionEnd");
    vi.spyOn(ResultService, "getPinResult").mockResolvedValue(descriptor("1"));
    vi.spyOn(ResultService, "getValue").mockResolvedValue({ kind: "sequence", value: [[1]] });
    vi.spyOn(ResultService, "getPage").mockResolvedValue(page("1"));
    await resultQueryCoordinator.loadPinResult(request);
    await resultQueryCoordinator.loadValue({
      executionSessionId: resultSessionFixture,
      resultId: "1",
    });
    const pageRequest = {
      executionSessionId: resultSessionFixture,
      resultId: "1",
      offset: 0,
      limit: 200,
    };
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
    expect(resultQueryRead.getDescriptor(resultReferenceFixture("1"))).toBeNull();
    expect(resultQueryRead.getValue(resultReferenceFixture("1"))).toBeNull();
    expect(resultQueryRead.getPage(pageRequest)).toBeNull();
    expect(resultQueryRead.getPinResult(request)).toBeNull();
    expect(readPinResultStatus(request)).toBe("running");
    expect(publishInvalidation).not.toHaveBeenCalled();
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
    expect(resultQueryRead.getDescriptor(resultReferenceFixture("2"))).toBeNull();
    expect(readPinResultStatus(request)).toBe("cancelled");
    vi.mocked(ResultService.getPinResult).mockResolvedValue(descriptor("1"));
    observeResultRunEvent({
      run: { runId: "1", graphPath, executionSessionId: "new-session" },
      kind: { type: "runStarted", outputs: [output] },
    });
    expect(readPinResultStatus(request)).toBe("running");
    observeResultRunEvent({
      run: { runId: "1", graphPath, executionSessionId: "new-session" },
      kind: { type: "runCompleted" },
    });
    await vi.waitFor(() => expect(resultQueryRead.getPinResult(request)?.resultId).toBe("1"));
    expect(resultQueryRead.getDescriptor(resultReferenceFixture("2"))).toBeNull();
    event("1", { type: "runCancelled" });
    expect(resultQueryRead.getPinResult(request)?.resultId).toBe("1");
  });

  it("keeps held snapshots separate from current pins when a graph changes", async () => {
    vi.spyOn(ResultService, "getPinResult").mockResolvedValue(descriptor("1"));
    await resultQueryCoordinator.loadPinResult(request);
    const reference = resultReferenceFixture("1");
    const release = resultQueryCoordinator.retainPayload(reference);
    vi.spyOn(ResultService, "getValue").mockResolvedValue({ kind: "value", value: 42 });
    vi.spyOn(ResultService, "getDescriptor").mockResolvedValue(descriptor("1"));
    await resultQueryCoordinator.loadValue(reference);
    const changed = structuredClone(fixture.projection);
    changed.basis.semanticInputHash = "1".repeat(64);
    useGraphProjectionStore.getState().replaceProjection(graphPath, changed);
    expect(resultQueryRead.getPinResult(request)).toBeNull();
    expect(resultQueryRead.getDescriptor(reference)?.resultId).toBe("1");
    expect(resultQueryRead.getValue(reference)).toEqual({ kind: "value", value: 42 });
    await resultQueryCoordinator.loadDescriptor(reference);
    expect(resultQueryRead.getPinResult(request)).toBeNull();
    release();
    expect(resultQueryRead.getDescriptor(reference)).toBeNull();
    await resultQueryCoordinator.loadPinResult(request);
    useGraphProjectionStore.getState().clearGraph(graphPath);
    expect(resultQueryRead.getDescriptor(resultReferenceFixture("1"))).toBeNull();
    expect(resultQueryRead.getPinResult(request)).toBeNull();
  });
  it("keeps one page, releases closed-view payloads, and supports detached result reads", async () => {
    clearProjectLifecycle();
    const releasePayload = resultQueryCoordinator.retainPayload(resultReferenceFixture("1"));
    vi.spyOn(ResultService, "getDescriptor").mockResolvedValue(descriptor("1"));
    vi.spyOn(ResultService, "getPage").mockImplementation(async (id, offset, limit) => ({
      ...page(id.resultId),
      offset,
      requestedLimit: limit,
    }));
    await resultQueryCoordinator.loadDescriptor({
      executionSessionId: resultSessionFixture,
      resultId: "1",
    });
    const first = {
      executionSessionId: resultSessionFixture,
      resultId: "1",
      offset: 0,
      limit: 200,
    };
    const second = { ...first, offset: 200 };
    await resultQueryCoordinator.loadPage(first);
    await resultQueryCoordinator.loadPage(second);
    expect(resultQueryRead.getPage(first)).toBeNull();
    expect(resultQueryRead.getPage(second)?.offset).toBe(200);
    let settle!: (value: ResultPage) => void;
    vi.mocked(ResultService.getPage).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          settle = resolve;
        }),
    );
    const pending = resultQueryCoordinator.loadPage(first);
    releasePayload();
    expect(resultQueryRead.getPage(second)).toBeNull();
    expect(resultQueryRead.getDescriptor(resultReferenceFixture("1"))).toBeNull();
    settle(page("1"));
    await expect(pending).resolves.toEqual({ status: "stale" });
    expect(resultQueryRead.getPage(first)).toBeNull();
  });
});

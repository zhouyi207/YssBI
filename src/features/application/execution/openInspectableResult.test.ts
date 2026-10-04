import {
  resultSessionFixture,
  resultReferenceFixture,
  resultLeaseIdFixture,
} from "@/tests/helpers/resultFixture";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";
import { useResourceStore } from "@/features/core/resource/resourceStore";

import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import type { ResultDescriptor, ResultLease } from "@/shared/types/domain/result";
import { IPC_ERROR_BRAND } from "@/shared/constants/ipcError";

const mocks = vi.hoisted(() => ({
  upsertResult: vi.fn(),
  showWorkbenchLayoutError: vi.fn(),
  loadPinResult: vi.fn(),
  getPinResult: vi.fn(),
  acquire: vi.fn(),
  finish: vi.fn(),
}));

vi.mock("@/features/application/results/resultLeases", () => ({
  resultLeases: { acquire: mocks.acquire, finish: mocks.finish },
}));

vi.mock("@/features/application/window", () => ({
  openPresentationWindow: vi.fn(),
  presentationWindowPayloadFromDescriptor: vi.fn(() => ({ kind: "plot", windowTitle: "Plot" })),
}));

vi.mock("@/modules/workbench/internal/layout/workbenchControl", () => ({
  workbenchLayoutControl: { upsertResult: mocks.upsertResult },
}));

vi.mock("@/modules/workbench/internal/application/workbenchLayoutErrorFeedback", () => ({
  showWorkbenchLayoutError: mocks.showWorkbenchLayoutError,
}));

vi.mock("@/features/application/results/runtime", () => ({
  resultQueryCoordinator: {
    loadPinResult: mocks.loadPinResult,
  },
  resultQueryRead: {
    getPinResult: mocks.getPinResult,
  },
}));

import { openInspectableResult, openPinInspectableView } from "./openInspectableResult";

const descriptor: ResultDescriptor = {
  resultId: "17",

  executionSessionId: resultSessionFixture,
  provenance: {
    runId: "run-1",
    graphPath: "events/Main.yssbi-event",
    nodeId: "node-1",
    output: {
      graphPath: "events/Main.yssbi-event",
      port: { kind: "declared", nodeId: "node-1", portKey: "result" },
    },
    createdAtMs: "1787270400000",
  },
  presentation: { kind: "inspector" },
  valueKind: "scalar",
  metadata: null,
  totalCount: null,
  title: "Node result",
};

beforeEach(() => {
  vi.restoreAllMocks();
  mocks.upsertResult.mockReset();
  mocks.upsertResult.mockImplementation(async (metadata) => ({
    panelInstanceId: "result-panel",
    metadata: { role: "result", ...metadata },
  }));
  mocks.acquire.mockReset();
  mocks.acquire.mockResolvedValue({
    descriptor,
    leaseId: resultLeaseIdFixture(100),
  });
  mocks.finish.mockReset();
  mocks.showWorkbenchLayoutError.mockReset();
  mocks.loadPinResult.mockReset();
  mocks.loadPinResult.mockResolvedValue({ status: "published" });
  mocks.getPinResult.mockReset();
  mocks.getPinResult.mockReturnValue(null);
  clearProjectLifecycle();
  startProjectLifecycle("project-1");
});

afterEach(() => useResourceStore.getState().clear());

describe("openInspectableResult", () => {
  it("resolves input targets from the current graph at invocation and tries them in order", async () => {
    const graphPath = "events/Main.yssbi-event";
    const [first, second, target] = [1, 2, 3].map((id) =>
      makeEditorProjectionFixture({
        graphPath,
        nodeId: `00000000-0000-0000-0000-${String(id).padStart(12, "0")}`,
      }),
    );
    const connection = {
      connectionId: "connection-1",
      input: target.inputAddress,
      output: first.outputAddress,
      order: null,
    };
    const session = makeGraphEditorSession({
      ...first.projection,
      nodes: [first, second, target].flatMap((part) => part.projection.nodes),
      connections: [connection],
    });
    useResourceStore.getState().installGraphSession(graphPath, session, { mode: "load" });
    const params = { graphPath, address: target.inputAddress, direction: "input" as const };
    const updated = {
      ...session,
      editing: {
        ...session.editing,
        version: { ...session.editing.version, revision: "1" },
      },
      projection: {
        ...session.projection,
        connections: [
          { ...connection, connectionId: "connection-2", output: second.outputAddress },
          connection,
        ],
      },
    };
    useResourceStore.getState().installGraphSession(graphPath, updated, { mode: "load" });
    mocks.getPinResult.mockReturnValueOnce(null).mockReturnValueOnce(descriptor);

    await expect(openPinInspectableView(params)).resolves.toBe(true);

    expect(mocks.loadPinResult.mock.calls).toEqual([
      [{ graphPath, output: second.outputAddress }],
      [{ graphPath, output: first.outputAddress }],
    ]);
    expect(mocks.upsertResult).toHaveBeenCalledOnce();
  });

  it("atomically upserts the logical Result panel", async () => {
    await expect(
      openInspectableResult({
        kind: "result",
        executionSessionId: resultSessionFixture,
        resultId: "17",
      }),
    ).resolves.toBe(true);

    expect(mocks.acquire).toHaveBeenCalledExactlyOnceWith(resultReferenceFixture("17"));
    expect(mocks.upsertResult).toHaveBeenCalledOnce();
    expect(mocks.upsertResult).toHaveBeenCalledWith({
      reference: resultReferenceFixture("17"),
      leaseId: resultLeaseIdFixture(100),
      title: "Node result",
      presentation: { kind: "inspector" },
    });
    const graphPath = "events/Main.yssbi-event";
    const fixture = makeEditorProjectionFixture({ graphPath });
    const session = makeGraphEditorSession(fixture.projection);
    session.resultState = {
      ...session.resultState,
      executionSessionId: resultSessionFixture,
      outputs: [
        { output: { graphPath, port: fixture.outputAddress }, state: "stale", resultId: "17" },
      ],
    };
    useResourceStore.getState().installGraphSession(graphPath, session, { mode: "load" });
    const target = { graphPath, address: fixture.outputAddress, direction: "output" as const };
    await expect(openPinInspectableView(target, "previous")).resolves.toBe(true);
    expect(mocks.acquire).toHaveBeenLastCalledWith(resultReferenceFixture("17"));
    expect(mocks.loadPinResult).not.toHaveBeenCalled();
    useResourceStore.getState().setGraphResultState(graphPath, {
      ...session.resultState,
      revision: "1",
      outputs: [
        { output: { graphPath, port: fixture.outputAddress }, state: "valid", resultId: "18" },
      ],
    });
    await expect(openPinInspectableView(target, "previous")).resolves.toBe(false);
    expect(mocks.acquire).toHaveBeenCalledTimes(2);
  });

  it("stops the upstream target sequence when the project changes during a result read", async () => {
    const graphPath = "events/Main.yssbi-event";
    const [first, second, target] = [1, 2, 3].map((id) =>
      makeEditorProjectionFixture({
        graphPath,
        nodeId: `00000000-0000-0000-0000-${String(id).padStart(12, "0")}`,
      }),
    );
    useResourceStore.getState().installGraphSession(
      graphPath,
      makeGraphEditorSession({
        ...first.projection,
        nodes: [first, second, target].flatMap((part) => part.projection.nodes),
        connections: [first, second].map((part, index) => ({
          connectionId: `connection-${index}`,
          input: target.inputAddress,
          output: part.outputAddress,
          order: null,
        })),
      }),
      { mode: "load" },
    );
    mocks.loadPinResult.mockImplementationOnce(async () => {
      clearProjectLifecycle();
      startProjectLifecycle("project-2");
      return { status: "stale" };
    });
    mocks.getPinResult.mockReturnValue(descriptor);

    await expect(
      openPinInspectableView({ graphPath, address: target.inputAddress, direction: "input" }),
    ).resolves.toBe(false);

    expect(mocks.loadPinResult).toHaveBeenCalledExactlyOnceWith({
      graphPath,
      output: first.outputAddress,
    });
    expect(mocks.acquire).not.toHaveBeenCalled();
    expect(mocks.upsertResult).not.toHaveBeenCalled();
  });

  it("drops current Pin result that settles after the project identity changes", async () => {
    let settlePinResult!: (value: ResultDescriptor | null) => void;
    let markQueryStarted!: () => void;
    const queryStarted = new Promise<void>((resolve) => {
      markQueryStarted = resolve;
    });
    mocks.loadPinResult.mockImplementationOnce(() => {
      markQueryStarted();
      return new Promise<{ status: "published" }>((resolve) => {
        settlePinResult = (value) => {
          mocks.getPinResult.mockReturnValue(value);
          resolve({ status: "published" });
        };
      });
    });

    const pending = openInspectableResult({
      kind: "outputPin",
      graphPath: "events/Main.yssbi-event",
      output: { kind: "declared", nodeId: "node-1", portKey: "result" },
    });
    await queryStarted;
    clearProjectLifecycle();
    startProjectLifecycle("project-2");
    settlePinResult(descriptor);

    await expect(pending).resolves.toBe(false);
    expect(mocks.acquire).not.toHaveBeenCalled();
    expect(mocks.upsertResult).not.toHaveBeenCalled();
  });

  it("releases a lease that settles after the project identity changes", async () => {
    let settleLease!: (value: ResultLease) => void;
    let markLeaseStarted!: () => void;
    const leaseStarted = new Promise<void>((resolve) => {
      markLeaseStarted = resolve;
    });
    mocks.acquire.mockImplementationOnce(() => {
      markLeaseStarted();
      return new Promise<ResultLease>((resolve) => {
        settleLease = resolve;
      });
    });

    const pending = openInspectableResult({
      kind: "result",
      executionSessionId: resultSessionFixture,
      resultId: "17",
    });
    await leaseStarted;
    clearProjectLifecycle();
    startProjectLifecycle("project-2");
    settleLease({ descriptor, leaseId: resultLeaseIdFixture(100) });

    await expect(pending).resolves.toBe(false);
    expect(mocks.finish).toHaveBeenCalledWith(resultLeaseIdFixture(100), false);
    expect(mocks.upsertResult).not.toHaveBeenCalled();
  });

  it("leaves an unavailable result closed without reporting a layout failure", async () => {
    mocks.acquire.mockRejectedValueOnce({
      [IPC_ERROR_BRAND]: true,
      code: "result_not_found",
      details: null,
      incidentId: null,
    });
    await expect(
      openInspectableResult({ kind: "result", ...resultReferenceFixture("17") }),
    ).resolves.toBe(false);
    expect(mocks.upsertResult).not.toHaveBeenCalled();
    expect(mocks.showWorkbenchLayoutError).not.toHaveBeenCalled();
    expect(mocks.finish).not.toHaveBeenCalled();
  });

  it("maps root Result upsert failures through typed layout feedback", async () => {
    const failure = new Error("private FlexLayout failure");
    mocks.upsertResult.mockRejectedValueOnce(failure);

    await expect(
      openInspectableResult({
        kind: "result",
        executionSessionId: resultSessionFixture,
        resultId: "17",
      }),
    ).resolves.toBe(false);

    expect(mocks.showWorkbenchLayoutError).toHaveBeenCalledOnce();
    expect(mocks.showWorkbenchLayoutError).toHaveBeenCalledWith(failure);
    expect(mocks.finish).toHaveBeenCalledWith(resultLeaseIdFixture(100), false);
  });
});

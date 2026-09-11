import {
  resultSessionFixture,
  resultReferenceFixture,
  resultLeaseIdFixture,
} from "@/tests/helpers/resultFixture";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { openPresentationWindow } from "@/features/application/window";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import type { ResultDescriptor, ResultReference } from "@/shared/types/domain/result";

const mocks = vi.hoisted(() => ({
  upsertResult: vi.fn(),
  showWorkbenchLayoutError: vi.fn(),
  loadPinResult: vi.fn(),
  loadDescriptor: vi.fn(),
  getPinResult: vi.fn(),
  getDescriptor: vi.fn(),
  acquire: vi.fn(),
  finish: vi.fn(),
}));

vi.mock("@/features/application/results/resultLeases", () => ({
  resultLeases: { acquire: mocks.acquire, finish: mocks.finish },
}));

vi.mock("@/features/application/window", () => ({
  openPresentationWindow: vi.fn(),
  presentationWindowPayloadFromDescriptor: vi.fn(() => ({ route: "/plot", windowTitle: "Plot" })),
}));

vi.mock("@/modules/workbench/internal/dockview/workbenchControl", () => ({
  workbenchDockviewControl: { upsertResult: mocks.upsertResult },
}));

vi.mock("@/modules/workbench/internal/application/workbenchLayoutErrorFeedback", () => ({
  showWorkbenchLayoutError: mocks.showWorkbenchLayoutError,
}));

vi.mock("@/features/application/results/runtime", () => ({
  resultQueryCoordinator: {
    loadPinResult: mocks.loadPinResult,
    loadDescriptor: mocks.loadDescriptor,
  },
  resultQueryRead: {
    getPinResult: mocks.getPinResult,
    getDescriptor: mocks.getDescriptor,
  },
}));

import { openInspectableResult } from "./openInspectableResult";

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

const plotDescriptor: ResultDescriptor = {
  ...descriptor,
  resultId: "18",
  presentation: { kind: "plot", chart: "scatter" },
  title: "Scatter result",
};

const t = ((key: string) => key) as never;

beforeEach(() => {
  vi.restoreAllMocks();
  mocks.upsertResult.mockReset();
  mocks.upsertResult.mockImplementation(async (metadata) => ({
    panelInstanceId: "result-panel",
    metadata: { role: "result", ...metadata },
  }));
  mocks.acquire.mockReset();
  mocks.acquire.mockImplementation(async (descriptor) => ({
    descriptor,
    leaseId: resultLeaseIdFixture(100),
  }));
  mocks.finish.mockReset();
  mocks.showWorkbenchLayoutError.mockReset();
  mocks.loadPinResult.mockReset();
  mocks.loadPinResult.mockResolvedValue({ status: "published" });
  mocks.loadDescriptor.mockReset();
  mocks.loadDescriptor.mockResolvedValue({ status: "published" });
  mocks.getPinResult.mockReset();
  mocks.getPinResult.mockReturnValue(null);
  mocks.getDescriptor.mockReset();
  mocks.getDescriptor.mockImplementation((reference: ResultReference) =>
    reference.resultId === plotDescriptor.resultId ? plotDescriptor : descriptor,
  );
  clearProjectLifecycle();
  startProjectLifecycle("project-1");
});

describe("openInspectableResult", () => {
  it("atomically upserts the logical Result panel", async () => {
    await expect(
      openInspectableResult(
        { kind: "result", executionSessionId: resultSessionFixture, resultId: "17" },
        t,
      ),
    ).resolves.toBe(true);

    expect(mocks.upsertResult).toHaveBeenCalledOnce();
    expect(mocks.upsertResult).toHaveBeenCalledWith({
      reference: resultReferenceFixture("17"),
      leaseId: resultLeaseIdFixture(100),
      title: "Node result",
      presentation: { kind: "inspector" },
    });
  });

  it("routes plot descriptors through the same root Result upsert without opening a window", async () => {
    mocks.getDescriptor.mockReturnValueOnce(plotDescriptor);

    await expect(
      openInspectableResult(
        { kind: "result", executionSessionId: resultSessionFixture, resultId: "18" },
        t,
      ),
    ).resolves.toBe(true);

    expect(mocks.upsertResult).toHaveBeenCalledWith({
      reference: resultReferenceFixture("18"),
      leaseId: resultLeaseIdFixture(100),
      title: "Scatter result",
      presentation: { kind: "plot", chart: "scatter" },
    });
    expect(openPresentationWindow).not.toHaveBeenCalled();
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

    const pending = openInspectableResult(
      {
        kind: "outputPin",
        graphPath: "events/Main.yssbi-event",
        output: { kind: "declared", nodeId: "node-1", portKey: "result" },
      },
      t,
    );
    await queryStarted;
    clearProjectLifecycle();
    startProjectLifecycle("project-2");
    settlePinResult(descriptor);

    await expect(pending).resolves.toBe(false);
    expect(mocks.loadDescriptor).not.toHaveBeenCalled();
    expect(mocks.upsertResult).not.toHaveBeenCalled();
  });

  it("drops a descriptor that settles after the project identity changes", async () => {
    let settleDescriptor!: (value: { status: "published" }) => void;
    let markDescriptorStarted!: () => void;
    const descriptorStarted = new Promise<void>((resolve) => {
      markDescriptorStarted = resolve;
    });
    mocks.loadDescriptor.mockImplementationOnce(() => {
      markDescriptorStarted();
      return new Promise<{ status: "published" }>((resolve) => {
        settleDescriptor = resolve;
      });
    });

    const pending = openInspectableResult(
      { kind: "result", executionSessionId: resultSessionFixture, resultId: "17" },
      t,
    );
    await descriptorStarted;
    clearProjectLifecycle();
    startProjectLifecycle("project-2");
    mocks.getDescriptor.mockReturnValue(descriptor);
    settleDescriptor({ status: "published" });

    await expect(pending).resolves.toBe(false);
    expect(mocks.upsertResult).not.toHaveBeenCalled();
  });

  it("maps root Result upsert failures through typed layout feedback", async () => {
    const failure = new Error("private Dockview failure");
    mocks.upsertResult.mockRejectedValueOnce(failure);

    await expect(
      openInspectableResult(
        { kind: "result", executionSessionId: resultSessionFixture, resultId: "17" },
        t,
      ),
    ).resolves.toBe(false);

    expect(mocks.showWorkbenchLayoutError).toHaveBeenCalledOnce();
    expect(mocks.showWorkbenchLayoutError).toHaveBeenCalledWith(failure);
  });
});

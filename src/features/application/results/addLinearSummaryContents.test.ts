import { beforeEach, expect, it, vi } from "vitest";
import reportFixture from "@/tests/fixtures/node-system-contracts/ols-summary-report.json";
import type { ApplyGraphMutationInput } from "@/features/application/graphEditing/graphEditCoordinator";
import type { GraphDocumentDto } from "@/shared/types/domain/editorMutation";
import type { ResultDescriptor } from "@/shared/types/domain/result";
import { addLinearSummaryContents } from "./addLinearSummaryContents";

const mocks = vi.hoisted(() => ({
  mutate: vi.fn(),
  getState: vi.fn(),
  loadGraph: vi.fn(),
  execute: vi.fn(),
  descriptor: vi.fn(),
  pin: vi.fn(),
  acquire: vi.fn(),
  finish: vi.fn(),
  loadValue: vi.fn(),
  getValue: vi.fn(),
  retainPayload: vi.fn(),
}));
vi.mock("@/features/application/graphEditing/graphEditCoordinator", () => ({
  applyGraphMutation: mocks.mutate,
  enqueueGraphTask: async (_path: string, work: () => unknown) => work(),
}));
vi.mock("@/features/application/project/projectIOStore", () => ({
  useProjectIOStore: { getState: () => ({ loadGraph: mocks.loadGraph }) },
}));
vi.mock("@/features/core/dataStore/graphProjectionStore", () => ({
  useGraphProjectionStore: { getState: mocks.getState },
}));
vi.mock("@/features/core/projectLifecycle/projectLifecycleAuthority", () => ({
  captureProjectIdentity: () => ({ projectInstanceId: "project", epoch: 1 }),
  isCurrentProjectIdentity: () => true,
}));
vi.mock("@/features/application/editor/observeGraphRunEvent", () => ({
  installGraphRunEvent: vi.fn(),
}));
vi.mock("@/features/application/graphProjection/graphActivity", () => ({
  recoverGraphExecution: vi.fn(async () => {}),
}));
vi.mock("@/services/project/projectService", () => ({
  ProjectService: { executeGraph: mocks.execute },
}));
vi.mock("@/services/result/resultService", () => ({
  ResultService: { getDescriptor: mocks.descriptor, getPinResult: mocks.pin },
}));
vi.mock("./resultLeases", () => ({
  resultLeases: { acquire: mocks.acquire, finish: mocks.finish },
}));
vi.mock("./runtime", () => ({
  resultQueryCoordinator: { loadValue: mocks.loadValue, retainPayload: mocks.retainPayload },
  resultQueryRead: { getValue: mocks.getValue },
}));

const graphPath = "events/summary.yssbi-event";
const nodeId = "00000000-0000-0000-0000-000000000009";
const version = { sessionId: "00000000-0000-0000-0000-000000000008", revision: "4" };
const original: ResultDescriptor = {
  ...reportFixture.resultRef,
  title: "Summary",
  valueKind: "scalar",
  metadata: null,
  totalCount: null,
  presentation: { kind: "report", report: "linearRegressionSummary" },
  provenance: {
    graphPath,
    nodeId,
    runId: "2",
    createdAtMs: "1",
    output: { graphPath, port: { kind: "declared", nodeId, portKey: "report" } },
  },
};
const next = { ...original, resultId: "18", provenance: { ...original.provenance, runId: "3" } };
const document: GraphDocumentDto = {
  nodes: {
    [nodeId]: {
      id: nodeId,
      node_type: "yssbi.statistics.linear.summary",
      position: { x: 0, y: 0 },
      user_label: null,
      parameters: { serial_tests: true },
    },
  },
  connections: {},
  input_states: [],
  port_bindings: [],
};

beforeEach(() => {
  vi.resetAllMocks();
  mocks.descriptor.mockResolvedValue(original);
  mocks.loadGraph.mockResolvedValue(true);
  mocks.mutate.mockResolvedValue({ status: "applied", result: { editing: { version } } });
  mocks.getState.mockReturnValue({
    sessions: { [graphPath]: { version, semanticInputHash: "hash", saving: false } },
  });
  mocks.execute.mockImplementation(async ({ onEvent }) => {
    onEvent({
      run: { executionSessionId: next.executionSessionId, graphPath, runId: "3" },
      kind: { type: "runCompleted" },
    });
  });
  mocks.pin.mockResolvedValue(next);
  mocks.acquire.mockResolvedValue({ leaseId: "new-lease", descriptor: next });
  mocks.retainPayload.mockReturnValue(vi.fn());
  mocks.loadValue.mockResolvedValue({ status: "published" });
  mocks.getValue.mockReturnValue({
    kind: "value",
    value: {
      ...reportFixture,
      resultRef: { executionSessionId: next.executionSessionId, resultId: next.resultId },
    },
  });
});

it("adds to the source node, executes its output, and releases a superseded report before publication", async () => {
  const additions = { acf_pacf: true as const, acf_max_lag: 3 };
  const updated = await addLinearSummaryContents(original, additions, () => true);
  const input = mocks.mutate.mock.calls[0][0] as ApplyGraphMutationInput;
  expect(typeof input.mutation === "function" && input.mutation(document)).toEqual({
    type: "setParameters",
    payload: { nodeId, parameters: additions },
  });
  expect(mocks.execute.mock.calls[0][0].demand).toEqual({
    type: "outputs",
    outputs: [original.provenance.output],
    reuseInputs: true,
    includeDefaultResults: false,
  });
  expect(updated.data.resultRef.resultId).toBe("18");
  expect(mocks.finish).not.toHaveBeenCalled();
  updated.release();
  updated.release();
  expect(mocks.finish).toHaveBeenCalledExactlyOnceWith("new-lease", false);

  mocks.finish.mockClear();
  let active = true;
  mocks.loadValue.mockImplementationOnce(async () => {
    active = false;
    return { status: "published" };
  });
  await expect(addLinearSummaryContents(original, additions, () => active)).rejects.toMatchObject({
    code: "report_summary_changed",
  });
  expect(mocks.finish).toHaveBeenCalledExactlyOnceWith("new-lease", false);
});

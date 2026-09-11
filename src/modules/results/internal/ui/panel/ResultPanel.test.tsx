// @vitest-environment happy-dom
import { act, useState } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { resultReferenceFixture } from "@/tests/helpers/resultFixture";
import type { ResultDescriptor, ResultReference } from "@/shared/types/domain/result";
import { ResultService } from "@/services/result/resultService";
import {
  startProjectLifecycle,
  clearProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import {
  observeResultRunEvent,
  resetResultQueryProject,
  resultQueryCoordinator,
  resultQueryRead,
} from "@/features/application/results/runtime";
import { ResultPanel } from "./ResultPanel";

vi.mock("./ResultContent", () => ({
  ResultContent: ({ reference }: { reference: ResultReference }) => {
    const [mounted] = useState(reference.resultId);
    return <div data-result={reference.resultId}>{mounted}</div>;
  },
}));
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

it("keeps the opened snapshot when its pin is invalidated and rerun", async () => {
  const reference = resultReferenceFixture("1");
  const output = {
    graphPath: "events/main.yssbi-event",
    port: {
      kind: "declared" as const,
      nodeId: "00000000-0000-0000-0000-000000000002",
      portKey: "report",
    },
  };
  const descriptor: ResultDescriptor = {
    ...reference,
    provenance: {
      runId: "1",
      graphPath: output.graphPath,
      nodeId: output.port.nodeId,
      output,
      createdAtMs: "1",
    },
    presentation: { kind: "report", report: "olsSummary" },
    valueKind: "scalar",
    metadata: null,
    totalCount: 1,
    title: "OLS Summary",
  };
  const host = document.createElement("div");
  const root = createRoot(host);
  startProjectLifecycle("project");
  vi.spyOn(ResultService, "getPinResult").mockResolvedValue(descriptor);
  try {
    const request = { graphPath: output.graphPath, output: output.port };
    await resultQueryCoordinator.loadPinResult(request);
    await act(async () => root.render(<ResultPanel reference={reference} />));
    const content = host.querySelector("[data-result]");
    act(() =>
      observeResultRunEvent({
        run: {
          executionSessionId: reference.executionSessionId,
          graphPath: output.graphPath,
          runId: "2",
        },
        kind: { type: "runStarted", outputs: [output] },
      }),
    );
    expect(resultQueryRead.getPinResult(request)).toBeNull();
    expect(host.querySelector("[data-result]")).toBe(content);
    expect(host.textContent).toBe("1");
    vi.mocked(ResultService.getPinResult).mockResolvedValue({
      ...descriptor,
      resultId: "2",
      provenance: { ...descriptor.provenance, runId: "2" },
    });
    await act(async () =>
      observeResultRunEvent({
        run: {
          executionSessionId: reference.executionSessionId,
          graphPath: output.graphPath,
          runId: "2",
        },
        kind: { type: "runCompleted" },
      }),
    );
    expect(resultQueryRead.getPinResult(request)?.resultId).toBe("2");
    expect(host.querySelector("[data-result]")).toBe(content);
    expect(host.textContent).toBe("1");
  } finally {
    act(() => root.unmount());
    resetResultQueryProject();
    clearProjectLifecycle();
    vi.restoreAllMocks();
  }
});

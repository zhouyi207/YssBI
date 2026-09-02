// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ProjectService } from "@/services/project/projectService";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import type { RunEvent } from "@/shared/types/domain/runEvent";
import { openInspectableResult } from "@/features/application/execution/openInspectableResult";
import { useProjectOperations } from "./useProjectOperations";

const projectInstanceId = "project-instance-1";
const graphPath = "events/Main.yssbi-event";

function runStartedEvent(): RunEvent {
  return {
    run: {
      projectSessionId: "backend-session-1",
      graphPath,
      runId: "run-stale",
    },
    kind: { type: "runStarted" },
  };
}

const executionState = {
  graphs: {},
  startExecution: vi.fn(),
  setActiveRunId: vi.fn(),
  commitExecutionVisual: vi.fn(),
  setRecording: vi.fn(),
  completeExecution: vi.fn(),
  failExecution: vi.fn(),
  interruptExecution: vi.fn(),
  getGraph: vi.fn(() => ({ status: "completed" })),
  clearGraphRunProjections: vi.fn(),
};

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock("@/features/core/execution", () => ({
  useExecutionStore: { getState: () => executionState },
  graphHasClearableArtifacts: () => true,
}));
vi.mock("@/features/core/execution/executionRecording", () => ({
  ensureGraphExecutionTerminal: vi.fn(),
}));
vi.mock("./resolveExecutionGraphPath", () => ({
  resolveExecutionGraphPath: (targetGraphPath?: string) =>
    targetGraphPath ?? "events/Main.yssbi-event",
  getExecutionEventTarget: () => ({
    graphPath: "events/Main.yssbi-event",
    name: "Main",
  }),
}));
vi.mock("@/features/application/execution/openInspectableResult", () => ({
  openInspectableResult: vi.fn().mockResolvedValue(true),
}));
vi.mock("@/features/core/dataStore", () => ({
  loadActivatedProject: vi.fn(),
  resolveActiveProjectPath: vi.fn(),
  useGraphProjectionStore: {
    getState: () => ({
      graphEntities: {
        [graphPath]: {
          nodes: {
            "view-node": { nodeType: "yssbi.debug.view" },
            "other-node": { nodeType: "yssbi.statistics.ols.summary" },
          },
        },
      },
    }),
  },
}));
(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

describe("useProjectOperations execution demand", () => {
  let container: HTMLDivElement;
  let root: Root;
  let operations: ReturnType<typeof useProjectOperations>;

  beforeEach(() => {
    vi.clearAllMocks();
    clearProjectLifecycle();
    startProjectLifecycle(projectInstanceId);
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    vi.spyOn(ProjectService, "executeGraphDocument").mockResolvedValue(undefined);

    function Harness() {
      operations = useProjectOperations();
      return null;
    }
    act(() => root.render(<Harness />));
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.restoreAllMocks();
    clearProjectLifecycle();
  });

  it("passes explicit Default demand for an ordinary event run", async () => {
    await act(async () => {
      await operations.executeGraph();
    });

    expect(ProjectService.executeGraphDocument).toHaveBeenCalledWith(
      projectInstanceId,
      graphPath,
      { type: "default" },
      expect.any(Function),
      expect.any(Function),
    );
  });

  it("opens only the backend-requested result window", async () => {
    vi.mocked(ProjectService.executeGraphDocument).mockImplementation(
      async (_projectInstanceId, _graphPath, _demand, onEvent) => {
        onEvent?.({
          ...runStartedEvent(),
          kind: {
            type: "pinPreviewResultReady",
            output: {
              graphPath,
              port: {
                kind: "declared",
                nodeId: "00000000-0000-0000-0000-000000000001",
                portKey: "value",
              },
            },
            generation: 3,
            resultId: "16",
          },
        });
        expect(openInspectableResult).not.toHaveBeenCalled();
        onEvent?.({
          ...runStartedEvent(),
          kind: {
            type: "resultInspectionRequested",
            resultId: "17",
            source: {
              graphPath: "functions/Inspect.yssbi-function",
              nodeId: null,
              portAddress: null,
            },
          },
        });
        return undefined;
      },
    );

    await act(async () => {
      await operations.executeGraph();
    });

    expect(openInspectableResult).toHaveBeenCalledOnce();
    expect(openInspectableResult).toHaveBeenCalledWith(
      { kind: "result", resultId: "17" },
      expect.any(Function),
    );
  });

  it("clears only frontend run projections", async () => {
    await act(async () => {
      await operations.clearGraphArtifacts(graphPath);
    });

    expect(executionState.clearGraphRunProjections).toHaveBeenCalledWith(graphPath);
    expect("clearGraphExecutionArtifacts" in ProjectService).toBe(false);
  });

  it("ignores delayed events and completion after project lifecycle replacement", async () => {
    let emit!: (event: RunEvent) => void;
    let resolveExecution!: () => void;
    vi.mocked(ProjectService.executeGraphDocument).mockImplementation(
      (_projectInstanceId, _graphPath, _demand, onEvent) =>
        new Promise((resolve) => {
          emit = onEvent ?? (() => undefined);
          resolveExecution = resolve;
        }),
    );

    const execution = operations.executeGraph();
    startProjectLifecycle("project-instance-2");
    emit(runStartedEvent());
    resolveExecution();
    await act(async () => execution);

    expect(executionState.setActiveRunId).not.toHaveBeenCalled();
    expect(executionState.commitExecutionVisual).not.toHaveBeenCalled();
    expect(executionState.completeExecution).not.toHaveBeenCalled();
    expect(executionState.failExecution).not.toHaveBeenCalled();
    expect(executionState.interruptExecution).not.toHaveBeenCalled();
  });
});

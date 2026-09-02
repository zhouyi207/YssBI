// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { EditorCommandTarget } from "./editorCommandFocus";

const target: EditorCommandTarget = Object.freeze({
  panelInstanceId: "panel-main",
  groupId: "group-main",
  resourceRef: "events/Main.yssbi-event",
  resourceKind: "event",
});

const mocks = vi.hoisted(() => ({
  targetCurrent: true,
  resolveActiveProjectPath: vi.fn(async () => "D:/projects/demo"),
  saveGraphDraft: vi.fn(async () => true),
  saveChart: vi.fn(async () => true),
  warnCallFunctionIssuesBeforeSave: vi.fn(),
  showBlockingMessage: vi.fn(),
  showBlockingIpcError: vi.fn(),
}));

vi.mock("react-i18next", () => ({
  initReactI18next: { type: "3rdParty", init: () => undefined },
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock("@/features/core/dataStore", () => ({
  loadActivatedProject: vi.fn(),
  resolveActiveProjectPath: mocks.resolveActiveProjectPath,
}));

vi.mock("@/features/application/project/projectSession", () => ({
  resolveActiveProjectPath: mocks.resolveActiveProjectPath,
}));

vi.mock("@/features/application/chart/saveChartDocument", () => ({
  saveChartDocument: mocks.saveChart,
}));

vi.mock("@/features/core/chart/chartDocumentStore", () => ({
  useChartDocumentStore: {
    getState: () => ({ saveDocument: mocks.saveChart }),
  },
}));

vi.mock("@/features/core/resource", () => ({
  isResourceDocumentDirty: vi.fn(() => false),
}));

vi.mock("@/features/application/graphDraft/saveGraphDraft", () => ({
  saveGraphDraft: mocks.saveGraphDraft,
}));

vi.mock("@/features/application/graphDiagnostics/warnCallFunctionIssues", () => ({
  warnCallFunctionIssuesBeforeSave: mocks.warnCallFunctionIssuesBeforeSave,
}));

vi.mock("@/features/application/execution/openInspectableResult", () => ({
  openInspectableResult: vi.fn(async () => true),
}));

vi.mock("./editorCommandFocus", () => ({
  captureActiveEditorCommandTarget: () => target,
  isEditorCommandTargetCurrent: () => mocks.targetCurrent,
}));

vi.mock("./blockingErrorDialog", () => ({
  showBlockingMessage: mocks.showBlockingMessage,
  showBlockingIpcError: mocks.showBlockingIpcError,
}));

vi.mock("@/features/core/execution", () => ({
  useExecutionStore: { getState: () => ({}) },
  getExecutionEventGraph: vi.fn(),
  resolveExecutionGraphPath: vi.fn(),
  graphHasClearableArtifacts: vi.fn(),
}));

vi.mock("@/features/application/observability/appLogger", () => ({
  logger: {
    app: { error: vi.fn() },
    exec: { info: vi.fn(), error: vi.fn() },
  },
}));

import { useProjectOperations } from "./useProjectOperations";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("useProjectOperations saveGraph target authority", () => {
  let host: HTMLDivElement;
  let root: Root;
  let operations!: ReturnType<typeof useProjectOperations>;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.targetCurrent = true;
    mocks.resolveActiveProjectPath.mockResolvedValue("D:/projects/demo");
    mocks.saveGraphDraft.mockResolvedValue(true);
    mocks.saveChart.mockResolvedValue(true);

    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);

    function Harness() {
      operations = useProjectOperations();
      return null;
    }

    act(() => root.render(<Harness />));
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("saves the captured target resource instead of a later active layout tab", async () => {
    await act(async () => {
      await operations.saveGraph(target);
    });

    expect(mocks.warnCallFunctionIssuesBeforeSave).toHaveBeenCalledWith(target.resourceRef);
    expect(mocks.saveGraphDraft).toHaveBeenCalledWith(target.resourceRef, target.resourceKind);
  });

  it("stops before save when the target changes while project authority resolves", async () => {
    mocks.resolveActiveProjectPath.mockImplementationOnce(async () => {
      mocks.targetCurrent = false;
      return "D:/projects/demo";
    });

    await act(async () => {
      await operations.saveGraph(target);
    });

    expect(mocks.saveGraphDraft).not.toHaveBeenCalled();
  });

  it("ignores stale settlement feedback when the target changes during save", async () => {
    mocks.saveGraphDraft.mockImplementationOnce(async () => {
      mocks.targetCurrent = false;
      return true;
    });

    await act(async () => {
      await operations.saveGraph(target);
    });

    expect(mocks.saveGraphDraft).toHaveBeenCalledOnce();
    expect(mocks.showBlockingMessage).not.toHaveBeenCalled();
  });

  it("saves a chart by its captured target and ignores stale settlement feedback", async () => {
    const chartTarget: EditorCommandTarget = Object.freeze({
      panelInstanceId: "panel-chart",
      groupId: "group-main",
      resourceRef: "charts/Summary.yssbi-chart",
      resourceKind: "chart",
    });
    mocks.saveChart.mockImplementationOnce(async () => {
      mocks.targetCurrent = false;
      return false;
    });

    await act(async () => {
      await operations.saveGraph(chartTarget);
    });

    expect(mocks.saveChart).toHaveBeenCalledWith(chartTarget.resourceRef);
    expect(mocks.saveGraphDraft).not.toHaveBeenCalled();
    expect(mocks.showBlockingMessage).not.toHaveBeenCalled();
  });
});

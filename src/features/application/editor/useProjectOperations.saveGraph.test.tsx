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
  hydrateProjectPath: vi.fn(async () => "D:/projects/demo"),
  saveGraph: vi.fn(async () => true),
  saveChart: vi.fn(async () => true),
  showBlockingMessage: vi.fn(),
  showBlockingIpcError: vi.fn(),
}));

vi.mock("react-i18next", () => ({
  initReactI18next: { type: "3rdParty", init: () => undefined },
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock("@/features/application/project/projectSession", () => ({
  hydrateProjectPath: mocks.hydrateProjectPath,
}));

vi.mock("@/features/application/chart/saveChartDocument", () => ({
  saveChartDocument: mocks.saveChart,
}));

vi.mock("@/features/application/graphEditing/saveGraph", () => ({
  saveGraph: mocks.saveGraph,
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
    mocks.hydrateProjectPath.mockResolvedValue("D:/projects/demo");
    mocks.saveGraph.mockResolvedValue(true);
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

    expect(mocks.saveGraph).toHaveBeenCalledWith(target.resourceRef, target.resourceKind);
  });

  it("stops before save when the target changes while project authority resolves", async () => {
    mocks.hydrateProjectPath.mockImplementationOnce(async () => {
      mocks.targetCurrent = false;
      return "D:/projects/demo";
    });

    await act(async () => {
      await operations.saveGraph(target);
    });

    expect(mocks.saveGraph).not.toHaveBeenCalled();
  });

  it("ignores stale settlement feedback when the target changes during save", async () => {
    mocks.saveGraph.mockImplementationOnce(async () => {
      mocks.targetCurrent = false;
      return true;
    });

    await act(async () => {
      await operations.saveGraph(target);
    });

    expect(mocks.saveGraph).toHaveBeenCalledOnce();
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
    expect(mocks.saveGraph).not.toHaveBeenCalled();
    expect(mocks.showBlockingMessage).not.toHaveBeenCalled();
  });
});

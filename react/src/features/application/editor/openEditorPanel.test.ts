import { afterEach, expect, it, vi } from "vitest";
import { LayoutModelBinding } from "@/modules/workbench/internal/layout/layoutModelBinding";
import { createEmptyWorkbenchLayout } from "@/modules/workbench/internal/layout/workbenchLayoutDefaults";
import { configureWorkbenchModel } from "@/modules/workbench/internal/layout/workbenchActivityGroup";
import { createWorkbenchLayoutRuntime } from "@/modules/workbench/internal/layout/workbenchLayoutInternal";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";

const mocks = vi.hoisted(() => ({
  runtime: undefined as ReturnType<typeof createWorkbenchLayoutRuntime> | undefined,
  boundary: "" as string,
  replaceProject: () => {},
  showError: vi.fn(),
  reveal: vi.fn(async () => true),
  ensureViewport: vi.fn(),
}));

vi.mock("@/modules/workbench/public", async () => {
  const { WorkbenchLayoutError } =
    await import("@/modules/workbench/internal/layout/workbenchTypes");
  return {
    WorkbenchLayoutError,
    showWorkbenchLayoutError: mocks.showError,
    workbenchLayoutControl: {
      openEditor: async (
        request: Parameters<
          ReturnType<typeof createWorkbenchLayoutRuntime>["control"]["openEditor"]
        >[0],
      ) => {
        const panel = await mocks.runtime!.control.openEditor(request);
        if (mocks.boundary === "open") mocks.replaceProject();
        return panel;
      },
    },
  };
});
vi.mock("./editorOpenTarget", () => ({
  resolveEditorOpenTargetGroupId: async () => {
    const groupId = await mocks.runtime!.control.ensureCentralGroup();
    if (mocks.boundary === "target") mocks.replaceProject();
    return groupId;
  },
}));
vi.mock("./resolveResourceDisplayName", () => ({
  resolveResourceDisplayName: (_target: unknown, fallback: string) => fallback,
}));
vi.mock("./editorPanelActivation", () => ({ revealActiveEditorDetails: mocks.reveal }));
vi.mock("@/features/core/viewport", () => ({
  ensureEditorViewport: mocks.ensureViewport,
  editorViewportScope: (groupId: string, graphPath: string) => ({ groupId, graphPath }),
}));
vi.mock("@/utils/frontendLogger", () => ({ logger: { graph: { trace: vi.fn() } } }));
vi.mock("./openEditorPanel", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./openEditorPanel")>();
  return {
    ...actual,
    openEditorPanel: (...args: Parameters<typeof actual.openEditorPanel>) => {
      const opened = actual.openEditorPanel(...args);
      if (mocks.boundary === "caller") void opened.then(mocks.replaceProject, () => {});
      return opened;
    },
  };
});

import { openFileInEditor } from "./openFileInEditor";
import { openDatabaseInEditor } from "./openDatabaseInEditor";
import { openGraphInEditor } from "./openGraphInEditor";

afterEach(() => {
  mocks.runtime?.internal.unbind();
  clearProjectLifecycle();
});

it("does not admit or reveal an old editor open after project replacement at each await boundary", async () => {
  for (const boundary of ["target", "open", "caller"]) {
    vi.clearAllMocks();
    const runtime = createWorkbenchLayoutRuntime();
    const binding = new LayoutModelBinding(createEmptyWorkbenchLayout(), configureWorkbenchModel);
    mocks.runtime = runtime;
    mocks.boundary = boundary;
    runtime.internal.bind(binding);
    runtime.internal.completeHydration();
    startProjectLifecycle("original");
    mocks.replaceProject = () => {
      startProjectLifecycle("successor");
      runtime.internal.invalidatePendingOperations();
    };
    try {
      if (boundary === "target") await openFileInEditor("charts/A.yssbi-chart", "chart");
      else if (boundary === "open") await openDatabaseInEditor("database-a");
      else await openGraphInEditor("events/A.yssbi-event", "A", "event_graph");
      if (boundary === "target") expect.soft(runtime.read.listPanels(), boundary).toHaveLength(0);
      expect.soft(mocks.reveal, boundary).not.toHaveBeenCalled();
      expect.soft(mocks.ensureViewport, boundary).not.toHaveBeenCalled();
      expect.soft(mocks.showError, boundary).not.toHaveBeenCalled();
    } finally {
      runtime.internal.unbind();
    }
  }
});

// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useHistoryStore } from "@/features/core/history";
import { HistoryService } from "@/services/nodeSystem/historyService";
import {
  resetHistoryCoordinator,
  setHistoryStatus,
} from "@/features/application/editorMutation/historyCoordinator";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import { useEditorHistoryAvailability } from "./useEditorHistoryAvailability";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const activeEditor = vi.hoisted(() => ({
  activeTabId: "events/Main.yssbi-event" as string | null,
}));
const projectInstanceId = "00000000-0000-0000-0000-000000000601";

vi.mock("@/features/core/editor/hooks/useActiveEditorGroup", () => ({
  useActiveEditorGroup: () => ({ activeTabId: activeEditor.activeTabId }),
}));
vi.mock("@/services/nodeSystem/historyService", () => ({
  HistoryService: {
    getStatus: vi.fn(async () => ({ canUndo: false, canRedo: false })),
    undo: vi.fn(),
    redo: vi.fn(),
  },
}));

describe("useEditorHistoryAvailability", () => {
  let host: HTMLDivElement;
  let root: Root;
  let current: ReturnType<typeof useEditorHistoryAvailability> | undefined;

  function Harness() {
    current = useEditorHistoryAvailability();
    return null;
  }

  beforeEach(() => {
    vi.clearAllMocks();
    resetHistoryCoordinator();
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    activeEditor.activeTabId = "events/Main.yssbi-event";
    useHistoryStore.setState({ canUndo: false, canRedo: false, pending: false }, true);
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("queries backend history status when availability is first consumed", async () => {
    vi.mocked(HistoryService.getStatus).mockResolvedValueOnce({ canUndo: true, canRedo: false });

    await act(async () => {
      root.render(createElement(Harness));
      await Promise.resolve();
    });

    expect(HistoryService.getStatus).toHaveBeenCalledOnce();
    expect(HistoryService.getStatus).toHaveBeenCalledWith(projectInstanceId);
    expect(current).toMatchObject({ canUndo: true, canRedo: false, pending: false });
  });

  it("uses project history status only for an active graph and masks it while pending", () => {
    setHistoryStatus({ canUndo: true, canRedo: true });
    act(() => root.render(createElement(Harness)));

    expect(current).toEqual({
      activeTabId: "events/Main.yssbi-event",
      canUndo: true,
      canRedo: true,
      pending: false,
    });

    act(() => useHistoryStore.setState({ pending: true }));
    expect(current).toMatchObject({ canUndo: false, canRedo: false, pending: true });

    activeEditor.activeTabId = null;
    act(() => root.render(createElement(Harness)));
    expect(current).toEqual({ activeTabId: null, canUndo: false, canRedo: false, pending: true });
  });
});

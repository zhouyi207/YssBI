// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useEditorKeyboard } from "./useEditorKeyboard";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

type TestPanel = {
  panelInstanceId: string;
  groupId: string;
  metadata:
    | { role: "editor"; resourceRef: string; resourceKind: "event" | "function" | "worksheet" }
    | { role: "result"; resultId: string }
    | { role: "view"; viewId: string };
};

const mocks = vi.hoisted(() => ({
  ignoreShortcut: false,
  targetCurrent: true,
  activePanel: null as TestPanel | null,
  groupPanels: [] as TestPanel[],
  history: { canUndo: false, canRedo: false, pending: false },
  interaction: { type: "idle" } as { type: string },
  selection: { nodeIds: new Set<string>(), connectionIds: new Set<string>() },
  setModifierKeys: vi.fn(),
  resetModifierKeys: vi.fn(),
  activate: vi.fn(async () => true),
  requestCloseWorkbenchPanel: vi.fn(async () => true),
  toggleWorkbenchView: vi.fn(async () => true),
  toggleActivityWorkbenchGroup: vi.fn(async () => undefined),
  toggleBottomWorkbenchGroup: vi.fn(async () => undefined),
  clearSelection: vi.fn(),
  cancelCanvasInteraction: vi.fn(),
  commands: {
    deleteSelected: vi.fn(),
    undo: vi.fn(),
    redo: vi.fn(),
    copy: vi.fn(),
    cut: vi.fn(),
    paste: vi.fn(),
    duplicateSelected: vi.fn(),
    saveGraph: vi.fn(),
    saveGraphAs: vi.fn(),
    importGraph: vi.fn(),
    addEvent: vi.fn(),
    closeTab: vi.fn(),
    setActiveTabId: vi.fn(),
    splitEditorRight: vi.fn(),
    selectAllNodes: vi.fn(async () => true),
    focusSelectedNodes: vi.fn(() => true),
    fitCompleteGraph: vi.fn(() => true),
  },
}));

vi.mock("./editorCommandFocus", () => ({
  captureActiveEditorCommandTarget: () => {
    const panel = mocks.activePanel;
    if (!panel || panel.metadata.role !== "editor") return null;
    return {
      panelInstanceId: panel.panelInstanceId,
      groupId: panel.groupId,
      resourceRef: panel.metadata.resourceRef,
      resourceKind: panel.metadata.resourceKind,
    };
  },
  isEditorCommandTargetCurrent: (target: {
    panelInstanceId: string;
    groupId: string;
    resourceRef: string;
    resourceKind: string;
  }) => {
    const panel = mocks.activePanel;
    return (
      mocks.targetCurrent &&
      panel?.metadata.role === "editor" &&
      panel.panelInstanceId === target.panelInstanceId &&
      panel.groupId === target.groupId &&
      panel.metadata.resourceRef === target.resourceRef &&
      panel.metadata.resourceKind === target.resourceKind
    );
  },
  shouldIgnoreEditorShortcutEvent: () => mocks.ignoreShortcut,
}));
vi.mock("@/features/core/keyboard", () => ({
  useModifierKeyStore: {
    getState: () => ({
      setModifierKeys: mocks.setModifierKeys,
      resetModifierKeys: mocks.resetModifierKeys,
    }),
  },
}));
vi.mock("@/features/core/history", () => ({
  useHistoryStore: Object.assign(
    (selector: (state: typeof mocks.history) => unknown) => selector(mocks.history),
    { getState: () => mocks.history },
  ),
}));
vi.mock("@/features/core/dockview/workbenchRead", () => ({
  workbenchDockviewRead: {
    getActivePanel: () => mocks.activePanel ?? undefined,
    listGroupPanels: (groupId: string) =>
      mocks.groupPanels.filter((panel) => panel.groupId === groupId),
  },
}));

vi.mock("@/features/core/dockview/workbenchControl", () => ({
  workbenchDockviewControl: {
    activate: mocks.activate,
  },
}));
vi.mock("./workbenchPanelClose", () => ({
  requestCloseWorkbenchPanel: mocks.requestCloseWorkbenchPanel,
}));
vi.mock("@/features/application/layout/workbenchLayoutActions", () => ({
  toggleWorkbenchView: mocks.toggleWorkbenchView,
  toggleActivityWorkbenchGroup: mocks.toggleActivityWorkbenchGroup,
  toggleBottomWorkbenchGroup: mocks.toggleBottomWorkbenchGroup,
}));
vi.mock("@/features/core/layout/layoutTabQueries", () => ({
  clearEditorGroupGraphSelection: mocks.clearSelection,
  getEditorGroupGraphSelection: () => mocks.selection,
}));
vi.mock("@/features/core/viewport", () => ({
  getViewport: () => ({ x: 0, y: 0, scale: 1 }),
  editorViewportScope: (groupId: string, graphPath: string) => ({ groupId, graphPath }),
}));
vi.mock("@/features/core/workbench", () => ({
  useWorkbenchStore: { getState: () => ({ setNodeDocumentationOpen: vi.fn() }) },
}));
vi.mock("@/features/core/graphInteraction/graphInteractionStore", () => ({
  getCanvasInteraction: () => mocks.interaction,
  useGraphInteractionStore: { getState: () => ({}) },
}));
vi.mock("@/features/core/canvas/canvasInteractionCleanup", () => ({
  cancelCanvasInteraction: mocks.cancelCanvasInteraction,
}));
vi.mock("@/features/core/editor", () => ({
  useEditorStore: { getState: () => ({ setContextMenu: vi.fn() }) },
}));
vi.mock("./EditorSessionContext", () => ({
  useEditorSessionCommandsContext: () => mocks.commands,
}));

const callbacks = mocks.commands;
const editorPanel = (): TestPanel => ({
  panelInstanceId: "editor-a",
  groupId: "group-a",
  metadata: {
    role: "editor",
    resourceRef: "events/main.yssbi-event",
    resourceKind: "event",
  },
});
const resultPanel = (): TestPanel => ({
  panelInstanceId: "result-a",
  groupId: "group-a",
  metadata: { role: "result", resultId: "result-a" },
});
const logsPanel = (): TestPanel => ({
  panelInstanceId: "logs-a",
  groupId: "group-a",
  metadata: { role: "view", viewId: "logs" },
});

let root: Root;

function Harness() {
  useEditorKeyboard();
  return null;
}

function keydown(key: string, init: KeyboardEventInit = {}): KeyboardEvent {
  const event = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...init });
  window.dispatchEvent(event);
  return event;
}

describe("useEditorKeyboard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.ignoreShortcut = false;
    mocks.targetCurrent = true;
    mocks.activePanel = editorPanel();
    mocks.groupPanels = [editorPanel(), resultPanel(), logsPanel()];
    mocks.history = { canUndo: false, canRedo: false, pending: false };
    mocks.interaction = { type: "idle" };
    mocks.selection = { nodeIds: new Set(), connectionIds: new Set() };
    callbacks.selectAllNodes.mockResolvedValue(true);
    callbacks.focusSelectedNodes.mockReturnValue(true);
    callbacks.fitCompleteGraph.mockReturnValue(true);
    document.body.replaceChildren();
    root = createRoot(document.createElement("div"));
    act(() => root.render(<Harness />));
  });

  afterEach(() => {
    act(() => root.unmount());
  });

  it.each([{ ctrlKey: true }, { metaKey: true }])(
    "routes Ctrl/Meta+A through a captured physical editor target",
    (modifier) => {
      const event = keydown("a", modifier);

      expect(callbacks.selectAllNodes).toHaveBeenCalledWith(
        expect.objectContaining({
          panelInstanceId: "editor-a",
          groupId: "group-a",
          resourceRef: "events/main.yssbi-event",
        }),
      );
      expect(event.defaultPrevented).toBe(true);
    },
  );

  it("routes only plain F and plain Home through the physical editor target", () => {
    const focusEvent = keydown("f");
    const homeEvent = keydown("Home");
    const modifiedFocusEvent = keydown("f", { ctrlKey: true });
    const modifiedHomeEvent = keydown("Home", { shiftKey: true });

    expect(callbacks.focusSelectedNodes).toHaveBeenCalledOnce();
    expect(callbacks.fitCompleteGraph).toHaveBeenCalledOnce();
    expect(callbacks.focusSelectedNodes).toHaveBeenCalledWith(
      expect.objectContaining({
        panelInstanceId: "editor-a",
      }),
    );
    expect(callbacks.fitCompleteGraph).toHaveBeenCalledWith(
      expect.objectContaining({
        panelInstanceId: "editor-a",
      }),
    );
    expect(focusEvent.defaultPrevented).toBe(true);
    expect(homeEvent.defaultPrevented).toBe(true);
    expect(modifiedFocusEvent.defaultPrevented).toBe(false);
    expect(modifiedHomeEvent.defaultPrevented).toBe(false);
  });

  it.each([
    ["F", "f", {}, callbacks.focusSelectedNodes],
    ["Home", "Home", {}, callbacks.fitCompleteGraph],
  ] as const)(
    "does not prevent default when %s is a command no-op",
    (_label, key, init, callback) => {
      callback.mockReturnValueOnce(false);
      const event = keydown(key, init);
      expect(callback).toHaveBeenCalledOnce();
      expect(event.defaultPrevented).toBe(false);
    },
  );

  it.each([
    ["Ctrl+A", "a", { ctrlKey: true }, callbacks.selectAllNodes],
    ["F", "f", {}, callbacks.focusSelectedNodes],
    ["Home", "Home", {}, callbacks.fitCompleteGraph],
  ] as const)("ignores repeated %s keydown", (_label, key, init, callback) => {
    const event = keydown(key, { ...init, repeat: true });
    expect(callback).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(false);
  });

  it("delegates input and modal suppression to shouldIgnoreEditorShortcutEvent", () => {
    mocks.ignoreShortcut = true;

    keydown("a", { ctrlKey: true });
    keydown("Delete");
    keydown("w", { ctrlKey: true });

    expect(callbacks.selectAllNodes).not.toHaveBeenCalled();
    expect(callbacks.deleteSelected).not.toHaveBeenCalled();
    expect(mocks.requestCloseWorkbenchPanel).not.toHaveBeenCalled();
  });

  it("denies editor mutations and navigation while a Result is physically active", () => {
    mocks.activePanel = resultPanel();
    mocks.history = { canUndo: true, canRedo: true, pending: false };

    keydown("a", { ctrlKey: true });
    keydown("f");
    keydown("Home");
    keydown("Delete");
    keydown("z", { ctrlKey: true });
    keydown("c", { ctrlKey: true });
    keydown("x", { ctrlKey: true });
    keydown("v", { ctrlKey: true });
    keydown("d", { ctrlKey: true });
    keydown("s", { ctrlKey: true });
    keydown("\\", { ctrlKey: true });

    expect(callbacks.selectAllNodes).not.toHaveBeenCalled();
    expect(callbacks.focusSelectedNodes).not.toHaveBeenCalled();
    expect(callbacks.fitCompleteGraph).not.toHaveBeenCalled();
    expect(callbacks.deleteSelected).not.toHaveBeenCalled();
    expect(callbacks.undo).not.toHaveBeenCalled();
    expect(callbacks.copy).not.toHaveBeenCalled();
    expect(callbacks.cut).not.toHaveBeenCalled();
    expect(callbacks.paste).not.toHaveBeenCalled();
    expect(callbacks.duplicateSelected).not.toHaveBeenCalled();
    expect(callbacks.saveGraph).not.toHaveBeenCalled();
    expect(callbacks.splitEditorRight).not.toHaveBeenCalled();
  });

  it("closes the physically active Result panel through the close coordinator", () => {
    mocks.activePanel = resultPanel();

    const event = keydown("w", { ctrlKey: true });

    expect(event.defaultPrevented).toBe(true);
    expect(mocks.requestCloseWorkbenchPanel).toHaveBeenCalledWith("result-a");
    expect(callbacks.closeTab).not.toHaveBeenCalled();
  });

  it("cycles every physical panel in active-group order in both directions", () => {
    mocks.activePanel = resultPanel();

    const nextEvent = keydown("Tab", { ctrlKey: true });
    expect(nextEvent.defaultPrevented).toBe(true);
    expect(mocks.activate).toHaveBeenLastCalledWith("logs-a");

    mocks.activate.mockClear();
    const previousEvent = keydown("Tab", { ctrlKey: true, shiftKey: true });
    expect(previousEvent.defaultPrevented).toBe(true);
    expect(mocks.activate).toHaveBeenLastCalledWith("editor-a");
  });

  it("passes the captured editor target to Ctrl+S but keeps Ctrl+Shift+S project-scoped", () => {
    const saveEvent = keydown("s", { ctrlKey: true });
    const saveAsEvent = keydown("s", { ctrlKey: true, shiftKey: true });

    expect(callbacks.saveGraph).toHaveBeenCalledWith({
      panelInstanceId: "editor-a",
      groupId: "group-a",
      resourceRef: "events/main.yssbi-event",
      resourceKind: "event",
    });
    expect(callbacks.saveGraphAs).toHaveBeenCalledWith();
    expect(saveEvent.defaultPrevented).toBe(true);
    expect(saveAsEvent.defaultPrevented).toBe(true);
  });

  it("runs project-scoped Ctrl+Shift+S while a Result is physically active", () => {
    mocks.activePanel = resultPanel();

    const event = keydown("s", { ctrlKey: true, shiftKey: true });

    expect(callbacks.saveGraphAs).toHaveBeenCalledOnce();
    expect(callbacks.saveGraph).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(true);
  });

  it("uses semantic workbench layout actions for Ctrl+B, Ctrl+I, and Ctrl+backtick", () => {
    keydown("b", { ctrlKey: true });
    keydown("i", { ctrlKey: true });
    keydown("`", { ctrlKey: true });

    expect(mocks.toggleActivityWorkbenchGroup).toHaveBeenCalledOnce();
    expect(mocks.toggleWorkbenchView).toHaveBeenCalledWith("inspect");
    expect(mocks.toggleBottomWorkbenchGroup).toHaveBeenCalledOnce();
  });
});

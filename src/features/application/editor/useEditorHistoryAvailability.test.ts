// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useGraphDraftStore } from "@/features/core/graphDraft";
import { useEditorHistoryAvailability } from "./useEditorHistoryAvailability";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const graphPath = "events/Main.yssbi-event";
const activeEditor = vi.hoisted(() => ({
  activeResourceRef: "events/Main.yssbi-event" as string | null,
}));

vi.mock("./editorGroupContext", () => ({
  useActiveEditorGroup: () => ({ activeResourceRef: activeEditor.activeResourceRef }),
}));

const draftDocument = { nodes: {}, port_bindings: [], connections: {}, input_states: [] };
const version = { document: draftDocument, projection: {} as never };

describe("useEditorHistoryAvailability", () => {
  let host: HTMLDivElement;
  let root: Root;
  let current: ReturnType<typeof useEditorHistoryAvailability> | undefined;

  function Harness() {
    current = useEditorHistoryAvailability();
    return null;
  }

  beforeEach(() => {
    activeEditor.activeResourceRef = graphPath;
    useGraphDraftStore.setState({ sessions: {} });
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("derives undo/redo only from the active frontend draft and masks both while saving", () => {
    useGraphDraftStore.setState({
      sessions: {
        [graphPath]: {
          ...version,
          sessionId: 1,
          draftGeneration: 0,
          semanticInputHash: "0".repeat(64),
          compiledInputHash: null,
          compileRequest: null,
          saveDirty: false,
          compileDirty: true,
          savedDocument: draftDocument,
          saving: false,
          compileStatus: "uncompiled",
          compiledArtifactId: null,
          compileCacheHit: false,
          undoStack: [version],
          redoStack: [version],
        },
      },
    });
    act(() => root.render(createElement(Harness)));
    expect(current).toEqual({
      activeResourceRef: graphPath,
      canUndo: true,
      canRedo: true,
      pending: false,
    });

    act(() => useGraphDraftStore.getState().beginSave(graphPath));
    expect(current).toEqual({
      activeResourceRef: graphPath,
      canUndo: false,
      canRedo: false,
      pending: true,
    });

    activeEditor.activeResourceRef = null;
    act(() => root.render(createElement(Harness)));
    expect(current).toEqual({
      activeResourceRef: null,
      canUndo: false,
      canRedo: false,
      pending: false,
    });
  });
});

// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { useGestureStore } from "@/features/core/gesture/useGestureStore";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import {
  getCanvasInteraction,
  useGraphInteractionStore,
} from "@/features/core/graphInteraction/graphInteractionStore";
import {
  cancelCanvasInteraction,
  clearCanvasInteractionGraph,
  clearCanvasInteractionProject,
  registerCanvasInteractionCleanup,
  resetCanvasInteractionCleanupForTests,
  startCanvasInteraction,
} from "./canvasInteractionCleanup";

const graphPath = "events/main";
const groupId = "group-a";

afterEach(() => {
  resetCanvasInteractionCleanupForTests();
  useGraphInteractionStore.setState({ interactions: {} });
  useGestureStore.getState().clearGesture(false);
  clearProjectLifecycle();
  document.body.innerHTML = "";
});

describe("canvasInteractionCleanup", () => {
  it("keeps successor ownership when explicit cancellation or replacement reenters", () => {
    for (const stage of [
      "cancel-cleanup",
      "start-cleanup-project",
      "start-cancel-publication",
      "start-publication",
    ] as const) {
      resetCanvasInteractionCleanupForTests();
      useGraphInteractionStore.setState({ interactions: {} });
      startProjectLifecycle("original");
      const scope = { graphPath, groupId, interactionType: "selecting" as const };
      if (stage !== "start-publication") {
        useGraphInteractionStore.getState().startInteraction(graphPath, {
          type: "selecting",
          session: { groupId, panelInstanceId: "old-pane" },
        });
      }
      const replacementCleanup = vi.fn();
      const remainingOldCleanup = vi.fn();
      let successor = useGraphInteractionStore.getState().interactions[graphPath];
      const replace = () => {
        if (stage === "start-cleanup-project") startProjectLifecycle("successor");
        else {
          useGraphInteractionStore.getState().startInteraction(graphPath, {
            type: "selecting",
            session: { groupId, panelInstanceId: "new-pane" },
          });
          successor = useGraphInteractionStore.getState().interactions[graphPath];
        }
        registerCanvasInteractionCleanup(scope, replacementCleanup);
      };
      if (stage !== "start-publication") {
        registerCanvasInteractionCleanup(scope, () => {
          if (stage === "cancel-cleanup" || stage === "start-cleanup-project") replace();
        });
        registerCanvasInteractionCleanup(scope, remainingOldCleanup);
      }
      let armed = stage.endsWith("publication");
      const stop = useGraphInteractionStore.subscribe(() => {
        if (!armed) return;
        armed = false;
        replace();
      });
      try {
        if (stage === "cancel-cleanup") {
          expect.soft(cancelCanvasInteraction(graphPath, groupId), stage).toBe("idle");
        } else {
          expect
            .soft(
              startCanvasInteraction(graphPath, {
                type: "panning",
                session: { groupId, panelInstanceId: "requested-pane" },
              }),
              stage,
            )
            .toBeNull();
        }
        expect
          .soft(useGraphInteractionStore.getState().interactions[graphPath], stage)
          .toBe(successor);
        expect
          .soft(remainingOldCleanup, stage)
          .toHaveBeenCalledTimes(stage === "start-cancel-publication" ? 1 : 0);
        expect.soft(replacementCleanup, stage).not.toHaveBeenCalled();
      } finally {
        stop();
      }
      cancelCanvasInteraction(graphPath, groupId);
      expect.soft(replacementCleanup, stage).toHaveBeenCalledOnce();
    }
    const installed = startCanvasInteraction(graphPath, {
      type: "panning",
      session: { groupId, panelInstanceId: "current-pane" },
    });
    expect(installed).toBe(useGraphInteractionStore.getState().interactions[graphPath]);
  });

  it("preserves successor cleanups, interactions and gesture state during lifecycle release", () => {
    const first = { graphPath, groupId, interactionType: "selecting" as const };
    const second = { ...first, groupId: "group-b" };
    for (const clear of [
      () => clearCanvasInteractionGraph(graphPath),
      clearCanvasInteractionProject,
    ]) {
      for (const stage of ["callback", "publication"]) {
        resetCanvasInteractionCleanupForTests();
        useGraphInteractionStore.setState({ interactions: {} });
        useGestureStore.getState().clearGesture(false);
        startProjectLifecycle("original");
        useGraphInteractionStore.getState().startInteraction(graphPath, {
          type: "selecting",
          session: { groupId, panelInstanceId: "old-pane" },
        });
        const replacementCleanup = vi.fn();
        let replaceSecond = () => {};
        let successor: ReturnType<typeof useGraphInteractionStore.getState>["interactions"][string];
        const replace = () => {
          if (stage === "publication") startProjectLifecycle("successor");
          replaceSecond();
          registerCanvasInteractionCleanup(second, replacementCleanup);
          useGraphInteractionStore.getState().startInteraction(graphPath, {
            type: "selecting",
            session: { groupId: second.groupId, panelInstanceId: "new-pane" },
          });
          successor = useGraphInteractionStore.getState().interactions[graphPath];
          useGestureStore.getState().clearGesture(true);
        };
        registerCanvasInteractionCleanup(first, () => {
          if (stage === "callback") replace();
        });
        replaceSecond = registerCanvasInteractionCleanup(second, () => {});
        let armed = stage === "publication";
        const stop = useGraphInteractionStore.subscribe(() => {
          if (!armed) return;
          armed = false;
          replace();
        });
        try {
          clear();
          expect.soft(successor!).toBeDefined();
          expect.soft(useGraphInteractionStore.getState().interactions[graphPath]).toBe(successor!);
          expect.soft(useGestureStore.getState().suppressNextContextMenu).toBe(true);
          expect.soft(replacementCleanup).not.toHaveBeenCalled();
        } finally {
          stop();
        }
        cancelCanvasInteraction(graphPath, second.groupId);
        expect.soft(replacementCleanup).toHaveBeenCalledOnce();
      }
    }
  });

  it("starts the first interaction when the graph has no interaction bucket", () => {
    expect(() =>
      startCanvasInteraction(graphPath, {
        type: "panning",
        session: { groupId, panelInstanceId: "panel-a" },
      }),
    ).not.toThrow();

    expect(getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, groupId).type).toBe(
      "panning",
    );
  });

  it("runs registered selection DOM cleanup before returning the interaction to idle", () => {
    document.body.innerHTML = `<div data-editor-group-id="${groupId}"><div data-selection-preview="true"></div></div>`;
    const canvas = document.querySelector(`[data-editor-group-id="${groupId}"]`)!;
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: "selecting",
      session: { groupId, panelInstanceId: "panel-a" },
    });
    const unregister = registerCanvasInteractionCleanup(
      { graphPath, groupId, interactionType: "selecting" },
      () =>
        canvas
          .querySelectorAll("[data-selection-preview]")
          .forEach((element) => element.removeAttribute("data-selection-preview")),
    );

    expect(cancelCanvasInteraction(graphPath, groupId)).toBe("selecting");
    expect(canvas.querySelector("[data-selection-preview]")).toBeNull();
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, groupId)).toEqual({
      type: "idle",
    });
    unregister();
  });

  it("consumes a registered cleanup once so a repeated selection cannot call an old closure", () => {
    const cleanup = vi.fn();
    registerCanvasInteractionCleanup({ graphPath, groupId, interactionType: "selecting" }, cleanup);
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: "selecting",
      session: { groupId, panelInstanceId: "panel-a" },
    });
    cancelCanvasInteraction(graphPath, groupId);

    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: "selecting",
      session: { groupId, panelInstanceId: "panel-a" },
    });
    cancelCanvasInteraction(graphPath, groupId);

    expect(cleanup).toHaveBeenCalledOnce();
  });

  it("cancels the previous pane cleanup before another pane owns the same graph", () => {
    const cleanup = vi.fn();
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: "selecting",
      session: { groupId, panelInstanceId: "panel-a" },
    });
    registerCanvasInteractionCleanup({ graphPath, groupId, interactionType: "selecting" }, cleanup);

    startCanvasInteraction(graphPath, {
      type: "panning",
      session: { groupId: "group-b", panelInstanceId: "panel-a" },
    });

    expect(cleanup).toHaveBeenCalledOnce();
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, groupId)).toEqual({
      type: "idle",
    });
    expect(
      getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, "group-b").type,
    ).toBe("panning");
  });

  it("keeps a replacement cleanup when a consumed registration is unregistered late", () => {
    const scope = { graphPath, groupId, interactionType: "selecting" as const };
    const interaction = {
      type: "selecting" as const,
      session: { groupId, panelInstanceId: "panel-a" },
    };
    const first = vi.fn();
    startCanvasInteraction(graphPath, interaction);
    const unregisterFirst = registerCanvasInteractionCleanup(scope, first);
    cancelCanvasInteraction(graphPath, groupId);

    const replacement = vi.fn();
    startCanvasInteraction(graphPath, interaction);
    const unregisterReplacement = registerCanvasInteractionCleanup(scope, replacement);
    unregisterFirst();
    cancelCanvasInteraction(graphPath, groupId);

    expect(first).toHaveBeenCalledOnce();
    expect(replacement).toHaveBeenCalledOnce();
    unregisterReplacement();
  });

  it("does not run an unmounted cleanup for a later interaction in the same scope", () => {
    const cleanup = vi.fn();
    const unregister = registerCanvasInteractionCleanup(
      { graphPath, groupId, interactionType: "selecting" },
      cleanup,
    );
    unregister();
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: "selecting",
      session: { groupId, panelInstanceId: "panel-a" },
    });

    cancelCanvasInteraction(graphPath, groupId);

    expect(cleanup).not.toHaveBeenCalled();
  });

  it("clears renderer cleanup and interaction through one graph lifecycle API", () => {
    const cleanup = vi.fn();
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: "draggingNodes",
      session: { groupId, panelInstanceId: "panel-a" },
    });
    registerCanvasInteractionCleanup(
      { graphPath, groupId, interactionType: "draggingNodes" },
      cleanup,
    );

    clearCanvasInteractionGraph(graphPath);

    expect(cleanup).toHaveBeenCalledOnce();
    expect(useGraphInteractionStore.getState().interactions[graphPath]).toBeUndefined();
  });

  it("clears every registered graph during project reset", () => {
    const first = vi.fn();
    const second = vi.fn();
    registerCanvasInteractionCleanup(
      { graphPath: "events/one", groupId, interactionType: "selecting" },
      first,
    );
    registerCanvasInteractionCleanup(
      { graphPath: "events/two", groupId, interactionType: "selecting" },
      second,
    );
    useGraphInteractionStore.setState({
      interactions: {
        "events/one": {
          type: "selecting",
          session: { groupId, panelInstanceId: "panel-a" },
        },
        "events/two": {
          type: "selecting",
          session: { groupId, panelInstanceId: "panel-a" },
        },
      },
    });

    clearCanvasInteractionProject();

    expect(first).toHaveBeenCalledOnce();
    expect(second).toHaveBeenCalledOnce();
    expect(useGraphInteractionStore.getState()).toMatchObject({
      interactions: {},
    });
  });

  it("does not run a cleanup registered for another pane", () => {
    let called = false;
    const unregister = registerCanvasInteractionCleanup(
      { graphPath, groupId: "group-b", interactionType: "selecting" },
      () => {
        called = true;
      },
    );
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: "selecting",
      session: { groupId, panelInstanceId: "panel-a" },
    });
    cancelCanvasInteraction(graphPath, groupId);
    expect(called).toBe(false);
    unregister();
  });
});

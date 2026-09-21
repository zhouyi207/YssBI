import { afterEach, describe, expect, it } from "vitest";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { captureProjectReadContext, useProjectIOStore } from "./projectIOStore";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { useDocumentStateStore } from "@/features/core/resource/documentStateStore";
import { buildGraphResourceMeta } from "@/features/core/resource/resourceTypes";
import { markResourceLoaded } from "@/features/core/resource/documentStateActions";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { makeEditorProjectionFixture } from "@/tests/helpers/editorProjectionFixtures";

afterEach(() => {
  clearProjectLifecycle();
  useProjectIOStore.setState({ projectInstanceId: null });
});

describe("project read context", () => {
  it("reuses a ready cached graph without publishing another loading-state update", async () => {
    const path = "events/Cached";
    const fixture = makeEditorProjectionFixture({ graphPath: path });
    useResourceStore.getState().setResources([buildGraphResourceMeta("event", path, "Cached")]);
    useGraphProjectionStore.getState().replaceProjection(path, fixture.projection);
    markResourceLoaded({ id: path, kind: "event" });
    useProjectIOStore.setState({ graphLoadStatus: { [path]: "ready" } });
    const state = useProjectIOStore.getState();
    let notifications = 0;
    const unsubscribe = useProjectIOStore.subscribe(() => notifications++);
    try {
      await expect(state.loadGraph(path)).resolves.toBe(true);
      await expect(state.loadGraph(path)).resolves.toBe(true);
      expect(useProjectIOStore.getState()).toBe(state);
      expect(notifications).toBe(0);
    } finally {
      unsubscribe();
      useProjectIOStore.setState({ graphLoadStatus: {} });
      useGraphProjectionStore.setState({ graphEntities: {} });
      useResourceStore.getState().clear();
      useDocumentStateStore.getState().clear();
    }
  });

  it("rejects reads while activation and the displayed projection disagree", () => {
    expect(captureProjectReadContext(null)).toBeNull();
    startProjectLifecycle("project-a");
    useProjectIOStore.setState({ projectInstanceId: "project-a" });
    const prior = captureProjectReadContext("project-a")!;
    expect(prior.isCurrent()).toBe(true);

    startProjectLifecycle("project-b");
    expect(prior.isCurrent()).toBe(false);
    expect(captureProjectReadContext("project-a")).toBeNull();
    expect(captureProjectReadContext("project-b")).toBeNull();

    useProjectIOStore.setState({ projectInstanceId: "project-b" });
    const current = captureProjectReadContext("project-b")!;
    expect(current.projectInstanceId).toBe("project-b");
    expect(current.isCurrent()).toBe(true);
    useProjectIOStore.setState({ projectInstanceId: null });
    expect(current.isCurrent()).toBe(false);
  });

  it("invalidates pending reads after clearing and restarting the same project identity", async () => {
    startProjectLifecycle("project-a");
    useProjectIOStore.setState({ projectInstanceId: "project-a" });
    const prior = captureProjectReadContext("project-a")!;
    let settle!: () => void;
    const pending = new Promise<void>((resolve) => {
      settle = resolve;
    }).then(() => prior.isCurrent());

    clearProjectLifecycle();
    expect(captureProjectReadContext("project-a")).toBeNull();
    startProjectLifecycle("project-a");
    expect(captureProjectReadContext("project-a")?.isCurrent()).toBe(true);
    settle();
    expect(await pending).toBe(false);
  });
});

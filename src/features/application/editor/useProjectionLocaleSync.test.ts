// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useProjectionLocaleSync } from "./useProjectionLocaleSync";
import { resetGraphProjectionLifecycle } from "@/features/application/graphProjection/graphProjectionLifecycle";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import {
  buildGraphResourceMeta,
  markResourceLoaded,
  useDocumentStateStore,
  useResourceStore,
} from "@/features/core/resource";
import { editorViewportScope, useViewportStore } from "@/features/core/viewport";
import { viewportScopeKey } from "@/features/core/viewport/viewportScope";
import { GraphProjectionService } from "@/services/nodeSystem/graphProjectionService";
import {
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const localeState = vi.hoisted(() => ({ language: "zh-CN" }));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    i18n: {
      language: localeState.language,
      resolvedLanguage: localeState.language,
    },
  }),
}));

vi.mock("@/services/nodeSystem/graphProjectionService", () => ({
  GraphProjectionService: {
    loadGraph: vi.fn(),
    hydrateGraph: vi.fn(),
  },
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function Harness() {
  useProjectionLocaleSync();
  return null;
}

describe("useProjectionLocaleSync", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    localeState.language = "zh-CN";
    resetGraphProjectionLifecycle();
    clearProjectLifecycle();
    startProjectLifecycle("project-instance-1");
    useGraphProjectionStore.setState({ graphEntities: {} });
    useResourceStore.getState().clear();
    useDocumentStateStore.getState().clear();
    useViewportStore.getState().clear();
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    clearProjectLifecycle();
    host.remove();
  });

  it("rehydrates each loaded graph once and preserves canvas viewport state", async () => {
    const eventPath = "events/Main.yssbi-event";
    const functionPath = "functions/Compute.yssbi-function";
    const unloadedPath = "events/Closed.yssbi-event";
    const currentEvent = makeEditorProjectionFixture({
      graphPath: eventPath,
      title: "当前事件",
    });
    const currentFunction = makeEditorProjectionFixture({
      graphPath: functionPath,
      title: "当前函数",
    });
    const localizedEvent = makeEditorProjectionFixture({
      graphPath: eventPath,
      title: "Localized event",
    });
    const localizedFunction = makeEditorProjectionFixture({
      graphPath: functionPath,
      title: "Localized function",
    });
    useGraphProjectionStore.getState().replaceProjection(eventPath, currentEvent.projection);
    useGraphProjectionStore.getState().replaceProjection(functionPath, currentFunction.projection);
    useResourceStore.getState().setSnapshot({
      resources: [
        buildGraphResourceMeta("event", eventPath, "Main"),
        buildGraphResourceMeta("function", functionPath, "Compute"),
        buildGraphResourceMeta("event", unloadedPath, "Closed"),
      ],
    });
    markResourceLoaded({ id: eventPath, kind: "event" });
    markResourceLoaded({ id: functionPath, kind: "function" });
    const viewportScope = editorViewportScope("default_editor", eventPath);
    useViewportStore.getState().setViewport(viewportScope, { x: 120, y: -30, scale: 1.5 });
    vi.mocked(GraphProjectionService.hydrateGraph).mockImplementation(
      async (projectInstanceId, graphPath, locale) => {
        expect(projectInstanceId).toBe("project-instance-1");
        expect(locale).toBe("en-US");
        return makeGraphEditorSession(
          graphPath === eventPath ? localizedEvent.projection : localizedFunction.projection,
        );
      },
    );

    await act(async () => root.render(createElement(Harness)));
    expect(GraphProjectionService.hydrateGraph).not.toHaveBeenCalled();

    localeState.language = "en-US";
    await act(async () => root.render(createElement(Harness)));
    await vi.waitFor(() => {
      expect(GraphProjectionService.hydrateGraph).toHaveBeenCalledTimes(2);
    });

    expect(GraphProjectionService.hydrateGraph).toHaveBeenCalledWith(
      "project-instance-1",
      eventPath,
      "en-US",
    );
    expect(GraphProjectionService.hydrateGraph).toHaveBeenCalledWith(
      "project-instance-1",
      functionPath,
      "en-US",
    );
    expect(GraphProjectionService.hydrateGraph).not.toHaveBeenCalledWith(
      "project-instance-1",
      unloadedPath,
      "en-US",
    );
    expect(useGraphProjectionStore.getState().graphEntities[eventPath]).toMatchObject({
      nodes: { "local-node": { display: { title: "Localized event" } } },
    });
    expect(useGraphProjectionStore.getState().graphEntities[functionPath]).toMatchObject({
      nodes: { "local-node": { display: { title: "Localized function" } } },
    });
    expect(useViewportStore.getState().viewports[viewportScopeKey(viewportScope)]).toEqual({
      x: 120,
      y: -30,
      scale: 1.5,
    });
  });

  it("ignores an older locale response after a newer language request starts", async () => {
    const graphPath = "events/Main.yssbi-event";
    const current = makeEditorProjectionFixture({ graphPath, title: "当前" });
    const olderLocale = makeEditorProjectionFixture({
      graphPath,
      title: "English",
    });
    const latestLocale = makeEditorProjectionFixture({
      graphPath,
      title: "中文",
    });
    const pendingEnglish = deferred<ReturnType<typeof makeGraphEditorSession>>();
    useGraphProjectionStore.getState().replaceProjection(graphPath, current.projection);
    useResourceStore.getState().setSnapshot({
      resources: [buildGraphResourceMeta("event", graphPath, "Main")],
    });
    markResourceLoaded({ id: graphPath, kind: "event" });
    vi.mocked(GraphProjectionService.hydrateGraph)
      .mockReturnValueOnce(pendingEnglish.promise)
      .mockResolvedValueOnce(makeGraphEditorSession(latestLocale.projection));

    await act(async () => root.render(createElement(Harness)));
    localeState.language = "en-US";
    await act(async () => root.render(createElement(Harness)));
    await vi.waitFor(() => {
      expect(GraphProjectionService.hydrateGraph).toHaveBeenCalledTimes(1);
    });
    localeState.language = "zh-CN";
    await act(async () => root.render(createElement(Harness)));
    await vi.waitFor(() => {
      expect(GraphProjectionService.hydrateGraph).toHaveBeenCalledTimes(2);
    });
    pendingEnglish.resolve(makeGraphEditorSession(olderLocale.projection));
    await act(async () => {
      await pendingEnglish.promise;
      await Promise.resolve();
    });

    expect(GraphProjectionService.hydrateGraph).toHaveBeenNthCalledWith(
      1,
      "project-instance-1",
      graphPath,
      "en-US",
    );
    expect(GraphProjectionService.hydrateGraph).toHaveBeenNthCalledWith(
      2,
      "project-instance-1",
      graphPath,
      "zh-CN",
    );
    expect(useGraphProjectionStore.getState().graphEntities[graphPath]).toMatchObject({
      nodes: { "local-node": { display: { title: "中文" } } },
    });
  });
});

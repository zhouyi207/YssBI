import { useSyncExternalStore } from "react";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import {
  commitViewport,
  getViewport,
  resetLiveViewports,
  setViewportLive,
  subscribeToViewport,
} from "./viewportSession";
import { useViewportStore } from "./useViewportStore";
import type { EditorViewport } from "./editorViewport";
import type { ViewportScope } from "./viewportScope";

export interface ViewportUiSnapshot {
  readonly viewports: DeepReadonly<Record<string, EditorViewport>>;
}

export interface ViewportUiCapability {
  readonly getSnapshot: () => DeepReadonly<ViewportUiSnapshot>;
  readonly subscribe: (listener: () => void) => () => void;
  readonly setViewport: (
    scope: ViewportScope,
    updater: Partial<EditorViewport> | ((previous: EditorViewport) => EditorViewport),
  ) => void;
  readonly getViewport: (scope: ViewportScope) => DeepReadonly<EditorViewport>;
  readonly setViewportLive: (
    scope: ViewportScope,
    updater: Partial<EditorViewport> | ((previous: EditorViewport) => EditorViewport),
  ) => void;
  readonly commitViewport: (scope: ViewportScope) => void;
  readonly subscribeToViewport: (
    scope: ViewportScope,
    listener: (viewport: DeepReadonly<EditorViewport>) => void,
  ) => () => void;
  readonly resetLiveViewports: (scope?: ViewportScope) => void;
  readonly clear: () => void;
}

function cloneViewport(viewport: EditorViewport): DeepReadonly<EditorViewport> {
  return Object.freeze({ ...viewport });
}

function buildSnapshot(): DeepReadonly<ViewportUiSnapshot> {
  const viewports = Object.fromEntries(
    Object.entries(useViewportStore.getState().viewports).map(([key, viewport]) => [
      key,
      cloneViewport(viewport),
    ]),
  );
  return Object.freeze({ viewports });
}

let currentSnapshot = buildSnapshot();
const listeners = new Set<() => void>();

function refreshSnapshot(): void {
  currentSnapshot = buildSnapshot();
  for (const listener of listeners) listener();
}

useViewportStore.subscribe(refreshSnapshot);

export function getViewportUiSnapshot(): DeepReadonly<ViewportUiSnapshot> {
  return currentSnapshot;
}

export function subscribeViewportUi(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useViewportUi<T>(selector: (snapshot: DeepReadonly<ViewportUiSnapshot>) => T): T {
  const snapshot = useSyncExternalStore(
    subscribeViewportUi,
    getViewportUiSnapshot,
    getViewportUiSnapshot,
  );
  return selector(snapshot);
}

export const viewportUi: ViewportUiCapability = {
  getSnapshot: getViewportUiSnapshot,
  subscribe: subscribeViewportUi,
  setViewport: (scope, updater) => useViewportStore.getState().setViewport(scope, updater),
  getViewport: (scope) => cloneViewport(getViewport(scope)),
  setViewportLive: (scope, updater) => setViewportLive(scope, updater),
  commitViewport,
  subscribeToViewport: (scope, listener) =>
    subscribeToViewport(scope, (viewport) => listener(cloneViewport(viewport))),
  resetLiveViewports,
  clear: () => useViewportStore.getState().clear(),
};

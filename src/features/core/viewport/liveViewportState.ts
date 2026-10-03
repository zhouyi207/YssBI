import { createStore, type StoreApi } from "zustand/vanilla";
import {
  captureProjectLifecycleState,
  isProjectLifecycleStateCurrent,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import type { EditorViewport } from "./editorViewport";
import { parseViewportScopeKey, viewportScopeKey, type ViewportScope } from "./viewportScope";

interface LiveViewport {
  store: StoreApi<EditorViewport | undefined>;
  subscribers: number;
}

// One store per pane keeps pointer frames local instead of notifying every open graph.
const liveViewports = new Map<string, LiveViewport>();

function viewportEntry(key: string): LiveViewport {
  let entry = liveViewports.get(key);
  if (!entry) {
    entry = { store: createStore<EditorViewport | undefined>(() => undefined), subscribers: 0 };
    liveViewports.set(key, entry);
  }
  return entry;
}

export function readLiveViewport(scope: ViewportScope): EditorViewport | undefined {
  return liveViewports.get(viewportScopeKey(scope))?.store.getState();
}

export function writeLiveViewport(scope: ViewportScope, viewport: EditorViewport): void {
  const store = viewportEntry(viewportScopeKey(scope)).store;
  writeViewport(store, viewport);
}

function writeViewport(
  store: StoreApi<EditorViewport | undefined>,
  viewport: EditorViewport,
): void {
  const previous = store.getState();
  if (previous?.x === viewport.x && previous.y === viewport.y && previous.scale === viewport.scale)
    return;
  store.setState(viewport, true);
}

export function subscribeLiveViewport(
  scope: ViewportScope,
  listener: (viewport: EditorViewport | undefined) => void,
): () => void {
  const key = viewportScopeKey(scope);
  const entry = viewportEntry(key);
  entry.subscribers++;
  const unsubscribe = entry.store.subscribe(listener);
  let subscribed = true;
  return () => {
    if (!subscribed) return;
    subscribed = false;
    unsubscribe();
    entry.subscribers--;
    releaseEmptyViewport(key, entry);
  };
}

function releaseEmptyViewport(key: string, entry: LiveViewport): void {
  if (
    entry.subscribers === 0 &&
    entry.store.getState() === undefined &&
    liveViewports.get(key) === entry
  )
    liveViewports.delete(key);
}

function resetViewportEntry(key: string, entry: LiveViewport): void {
  entry.store.setState(undefined, true);
  releaseEmptyViewport(key, entry);
}

function captureViewportEntries(scope?: ViewportScope | Pick<ViewportScope, "graphPath">) {
  if (scope && "groupId" in scope) {
    const key = viewportScopeKey(scope);
    const entry = liveViewports.get(key);
    return entry ? [{ key, entry, viewport: entry.store.getState() }] : [];
  }
  return [...liveViewports]
    .filter(([key]) => !scope || parseViewportScopeKey(key)?.graphPath === scope.graphPath)
    .map(([key, entry]) => ({ key, entry, viewport: entry.store.getState() }));
}

function ownsViewportEntry({
  key,
  entry,
  viewport,
}: ReturnType<typeof captureViewportEntries>[number]) {
  return liveViewports.get(key) === entry && entry.store.getState() === viewport;
}

export function synchronizeLiveViewports(
  viewports: Readonly<Record<string, EditorViewport>>,
  previous: Readonly<Record<string, EditorViewport>>,
  isCurrent: () => boolean,
): void {
  if (viewports === previous) return;
  const owner = captureProjectLifecycleState();
  for (const captured of captureViewportEntries()) {
    if (!isCurrent() || !isProjectLifecycleStateCurrent(owner)) return;
    if (!ownsViewportEntry(captured)) continue;
    const { key, entry } = captured;
    if (viewports[key] === previous[key]) continue;
    const viewport = viewports[key];
    if (viewport) writeViewport(entry.store, viewport);
    else resetViewportEntry(key, entry);
  }
}

export function resetLiveViewports(scope?: ViewportScope | Pick<ViewportScope, "graphPath">): void {
  prepareLiveViewportReset(scope)();
}

/** Capture before committed state publishes; its listeners may start newer live gestures. */
export function prepareLiveViewportReset(
  scope?: ViewportScope | Pick<ViewportScope, "graphPath">,
): () => void {
  const owner = captureProjectLifecycleState();
  const entries = captureViewportEntries(scope);
  return () => {
    for (const captured of entries) {
      if (!isProjectLifecycleStateCurrent(owner)) return;
      if (ownsViewportEntry(captured)) resetViewportEntry(captured.key, captured.entry);
    }
  };
}

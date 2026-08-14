import type { SerializedGridviewComponent } from 'dockview-react';
import { useWorkbenchStore, workbenchGridPort, type WorkbenchUIState } from '@/features/core/workbench';
import { editorDockviewPort } from './dockviewEditorPort';
import type { DockviewLayout } from './types';

const KEY_PREFIX = 'yssbi-dockview-layout:v1';
let windowScope = 'main';
let timer: ReturnType<typeof setTimeout> | null = null;
let hydrationGeneration = 0;

interface PersistedDockviewLayout {
  version: 1;
  workbench: SerializedGridviewComponent;
  editor: DockviewLayout;
  preferences: WorkbenchUIState;
}

export function setDockviewLayoutWindowScope(label: string): void {
  windowScope = label || 'main';
}

export function dockviewLayoutStorageKey(label = windowScope): string {
  return `${KEY_PREFIX}:${label}`;
}

function preferences(): WorkbenchUIState {
  const state = useWorkbenchStore.getState();
  return {
    sidebarCurrentTab: state.sidebarCurrentTab,
    sidebarUserHidden: state.sidebarUserHidden,
    panelActiveView: state.panelActiveView,
    panelUserHidden: state.panelUserHidden,
    detailUserHidden: state.detailUserHidden,
    isSettingsOpen: false,
    isNodeDocumentationOpen: false,
    zenMode: false,
  };
}

export async function persistDockviewLayoutNow(): Promise<void> {
  const workbench = workbenchGridPort.serialize();
  if (!workbench || !editorDockviewPort.isReady) return;
  const value: PersistedDockviewLayout = {
    version: 1,
    workbench,
    editor: await editorDockviewPort.serialize(),
    preferences: preferences(),
  };
  localStorage.setItem(dockviewLayoutStorageKey(), JSON.stringify(value));
}

export function persistDockviewLayoutDebounced(delayMs = 250): void {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => {
    timer = null;
    void persistDockviewLayoutNow();
  }, delayMs);
}

export function invalidateDockviewLayoutHydration(): void {
  hydrationGeneration += 1;
}

export async function hydrateDockviewLayout(): Promise<boolean> {
  const generation = ++hydrationGeneration;
  const raw = localStorage.getItem(dockviewLayoutStorageKey());
  if (!raw) return false;
  try {
    const value = JSON.parse(raw) as Partial<PersistedDockviewLayout>;
    if (value.version !== 1 || !value.workbench || !value.editor) return false;
    workbenchGridPort.restore(value.workbench);
    await editorDockviewPort.restore(value.editor);
    if (generation !== hydrationGeneration) return false;
    if (value.preferences) useWorkbenchStore.setState(value.preferences);
    return true;
  } catch {
    localStorage.removeItem(dockviewLayoutStorageKey());
    return false;
  }
}

export function clearPersistedDockviewLayout(): void {
  localStorage.removeItem(dockviewLayoutStorageKey());
}

import type { SerializedGridviewComponent } from 'dockview-react';
import { useWorkbenchStore, workbenchGridPort, type WorkbenchUIState } from '@/features/core/workbench';
import { editorDockviewPort } from './dockviewEditorPort';
import { panelDockviewPort } from './panelDockviewPort';
import type { DockviewLayout } from './types';

const KEY_PREFIX = 'yssbi-dockview-layout';
let windowScope = 'main';
let timer: ReturnType<typeof setTimeout> | null = null;
let hydrationGeneration = 0;

interface PersistedDockviewLayout {
  workbench: SerializedGridviewComponent;
  editor: DockviewLayout;
  shell: DockviewLayout;
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
    detailUserHidden: state.detailUserHidden,
    isSettingsOpen: false,
    isNodeDocumentationOpen: false,
    zenMode: false,
  };
}

export async function persistDockviewLayoutNow(): Promise<void> {
  const workbench = workbenchGridPort.serialize();
  if (!workbench || !editorDockviewPort.isReady || !panelDockviewPort.isReady) return;
  const [editor, shell] = await Promise.all([
    editorDockviewPort.serialize(),
    panelDockviewPort.serialize(),
  ]);
  const value: PersistedDockviewLayout = {
    workbench,
    editor,
    shell,
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
    if (!value.workbench || !value.editor || !value.shell) return false;

    editorDockviewPort.unbind();
    panelDockviewPort.unbind();
    workbenchGridPort.restore(value.workbench);
    await panelDockviewPort.whenReady();

    editorDockviewPort.unbind();
    await panelDockviewPort.restore(value.shell);
    await editorDockviewPort.whenReady();
    await editorDockviewPort.restore(value.editor);
    if (generation !== hydrationGeneration) return false;

    if (value.preferences) {
      const current = useWorkbenchStore.getState();
      useWorkbenchStore.setState({
        sidebarCurrentTab: value.preferences.sidebarCurrentTab ?? current.sidebarCurrentTab,
        sidebarUserHidden: value.preferences.sidebarUserHidden ?? current.sidebarUserHidden,
        detailUserHidden: value.preferences.detailUserHidden ?? current.detailUserHidden,
        isSettingsOpen: value.preferences.isSettingsOpen ?? current.isSettingsOpen,
        isNodeDocumentationOpen:
          value.preferences.isNodeDocumentationOpen ?? current.isNodeDocumentationOpen,
        zenMode: value.preferences.zenMode ?? current.zenMode,
      });
    }
    return true;
  } catch {
    localStorage.removeItem(dockviewLayoutStorageKey());
    return false;
  }
}

export function clearPersistedDockviewLayout(): void {
  localStorage.removeItem(dockviewLayoutStorageKey());
}

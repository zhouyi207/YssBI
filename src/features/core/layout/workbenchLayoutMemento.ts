import type { SidebarTabId } from './layoutStore';
import type { WorkbenchPartId } from './workbenchLayoutDefaults';
import type { EditorGridMemento } from './editorGridMemento';
import type { PanelViewId } from './panelPartModel';
import { logger } from '@/utils/appLogger';

const WORKBENCH_LAYOUT_STORAGE_KEY = 'yssbi-workbench-layout';
let workbenchLayoutWindowScope = 'main';

/** Resolve the workbench memento key for a Tauri window label. */
export function workbenchLayoutStorageKey(windowLabel = 'main'): string {
  return windowLabel === 'main'
    ? WORKBENCH_LAYOUT_STORAGE_KEY
    : `${WORKBENCH_LAYOUT_STORAGE_KEY}:${encodeURIComponent(windowLabel)}`;
}

/** Select the window scope consumed by the persistence and hydration pipeline. */
export function setWorkbenchLayoutWindowScope(windowLabel: string): void {
  workbenchLayoutWindowScope = windowLabel;
}

export interface WorkbenchPartMemento {
  pixelSize?: number;
  visible?: boolean;
  currentTab?: SidebarTabId;
  userHidden?: boolean;
  maximized?: boolean;
  restoredPixelSize?: number;
  activePanelView?: PanelViewId;
}

/** Unified workbench memento: chrome parts + editor grid topology. */
export interface WorkbenchLayoutMemento {
  parts: Partial<Record<WorkbenchPartId, WorkbenchPartMemento>>;
  editorGrid?: EditorGridMemento | null;
}

export function loadWorkbenchLayoutMemento(): WorkbenchLayoutMemento | null {
  if (typeof localStorage === 'undefined') return null;

  try {
    const raw = localStorage.getItem(workbenchLayoutStorageKey(workbenchLayoutWindowScope));
    if (!raw) return null;
    const parsed = JSON.parse(raw) as WorkbenchLayoutMemento;
    if (!parsed || typeof parsed !== 'object' || !parsed.parts) return null;
    return parsed;
  } catch (error) {
    logger.app.warn(
      `Failed to load workbench layout: ${error instanceof Error ? error.message : String(error)}`,
      'WorkbenchLayout',
    );
    return null;
  }
}

export function saveWorkbenchLayoutMemento(
  memento: WorkbenchLayoutMemento,
): void {
  if (typeof localStorage === 'undefined') return;

  try {
    localStorage.setItem(
      workbenchLayoutStorageKey(workbenchLayoutWindowScope),
      JSON.stringify(memento),
    );
  } catch (error) {
    logger.app.error(
      `Failed to save workbench layout: ${error instanceof Error ? error.message : String(error)}`,
      'WorkbenchLayout',
    );
  }
}

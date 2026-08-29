import { workbenchDockviewRead } from '@/features/core/dockview/workbenchRead';
import { isAppModalOpen } from '@/features/core/keyboard';
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { uiStore } from '@/features/core/ui/UIStore';

export interface EditorCommandTarget {
  readonly panelInstanceId: string;
  readonly groupId: string;
  readonly resourceRef: string;
  readonly resourceKind: 'event' | 'function' | 'worksheet';
}

const projectIdentityByTarget = new WeakMap<EditorCommandTarget, ProjectIdentitySnapshot>();

const SHORTCUT_CONSUMER_SELECTOR = [
  'input',
  'textarea',
  'select',
  'dialog',
  'menu',
  '[contenteditable]:not([contenteditable="false"])',
  '[role="dialog"]',
  '[aria-modal="true"]',
  '[role="menu"]',
  '[role="listbox"]',
  '[role="combobox"]',
  '[popover]',
  '[data-slot="dialog-content"]',
  '[data-slot="dropdown-menu-content"]',
  '[data-slot="dropdown-menu-sub-content"]',
  '[data-slot="select-content"]',
  '[data-slot="popover-content"]',
].join(',');

export function captureActiveEditorCommandTarget(): EditorCommandTarget | null {
  const panel = workbenchDockviewRead.getActiveEditorPanel();
  if (!panel || panel.metadata.role !== 'editor') return null;

  let projectIdentity: ProjectIdentitySnapshot;
  try {
    projectIdentity = captureProjectIdentity();
  } catch {
    return null;
  }

  const target: EditorCommandTarget = Object.freeze({
    panelInstanceId: panel.panelInstanceId,
    groupId: panel.groupId,
    resourceRef: panel.metadata.resourceRef,
    resourceKind: panel.metadata.resourceKind,
  });
  projectIdentityByTarget.set(target, projectIdentity);
  return target;
}

export function isEditorCommandTargetCurrent(target: EditorCommandTarget): boolean {
  const projectIdentity = projectIdentityByTarget.get(target);
  if (!projectIdentity || !isCurrentProjectIdentity(projectIdentity)) return false;

  const panel = workbenchDockviewRead.getActiveEditorPanel();
  return panel?.metadata.role === 'editor'
    && panel.panelInstanceId === target.panelInstanceId
    && panel.groupId === target.groupId
    && panel.metadata.resourceRef === target.resourceRef
    && panel.metadata.resourceKind === target.resourceKind;
}

function eventPath(event: KeyboardEvent): readonly EventTarget[] {
  try {
    return [event.target, ...event.composedPath()].filter(
      (target): target is EventTarget => target !== null,
    );
  } catch {
    return event.target ? [event.target] : [];
  }
}

function targetElement(target: EventTarget): Element | null {
  if (target instanceof Element) return target;
  if (target instanceof Node) return target.parentElement;
  return null;
}

function consumesEditorShortcut(target: EventTarget): boolean {
  const element = targetElement(target);
  if (!element) return false;
  if (element instanceof HTMLElement && element.isContentEditable) return true;
  return element.closest(SHORTCUT_CONSUMER_SELECTOR) !== null;
}

export function shouldIgnoreEditorShortcutEvent(event: KeyboardEvent): boolean {
  if (uiStore.getState().modals.length > 0 || isAppModalOpen()) return true;
  return eventPath(event).some(consumesEditorShortcut);
}

import { workbenchLayoutRead } from "@/modules/workbench/public";
import { isAppModalOpen } from "@/features/core/keyboard";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { uiStore } from "@/features/core/ui/UIStore";

export interface EditorCommandTarget {
  readonly panelInstanceId: string;
  readonly groupId: string;
  readonly resourceRef: string;
  readonly resourceKind: "event" | "function" | "chart";
}

const targetOwnership = new WeakMap<
  EditorCommandTarget,
  {
    readonly project: ProjectIdentitySnapshot;
    readonly panelScoped: boolean;
  }
>();

const SHORTCUT_CONSUMER_SELECTOR = [
  "input",
  "textarea",
  "select",
  "dialog",
  "menu",
  '[contenteditable]:not([contenteditable="false"])',
  '[role="dialog"]',
  '[aria-modal="true"]',
  '[role="menu"]',
  '[role="listbox"]',
  '[role="combobox"]',
  "[popover]",
  '[data-slot="dialog-content"]',
  '[data-slot="dropdown-menu-content"]',
  '[data-slot="dropdown-menu-sub-content"]',
  '[data-slot="select-content"]',
  '[data-slot="popover-content"]',
].join(",");

export function captureActiveEditorCommandTarget(): EditorCommandTarget | null {
  const panel = workbenchLayoutRead.getActiveEditorPanel();
  return captureTarget(panel);
}

/** Explicit canvas actions belong to that visible panel, not the global active group. */
export function captureEditorCommandTarget(panelInstanceId: string): EditorCommandTarget | null {
  const panel = workbenchLayoutRead.getPanel(panelInstanceId);
  if (!panel?.visible) return null;
  return captureTarget(panel, true);
}

function captureTarget(
  panel: ReturnType<typeof workbenchLayoutRead.getPanel>,
  panelScoped = false,
): EditorCommandTarget | null {
  if (!panel || panel.metadata.role !== "editor" || panel.metadata.resourceKind === "database")
    return null;

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
  targetOwnership.set(target, { project: projectIdentity, panelScoped });
  return target;
}

export function isEditorCommandTargetCurrent(target: EditorCommandTarget): boolean {
  const ownership = targetOwnership.get(target);
  if (!ownership || !isCurrentProjectIdentity(ownership.project)) return false;

  const scoped = ownership.panelScoped;
  const panel = scoped
    ? workbenchLayoutRead.getPanel(target.panelInstanceId)
    : workbenchLayoutRead.getActiveEditorPanel();
  return (
    (!scoped || panel?.visible === true) &&
    panel?.metadata.role === "editor" &&
    panel.panelInstanceId === target.panelInstanceId &&
    panel.groupId === target.groupId &&
    panel.metadata.resourceRef === target.resourceRef &&
    panel.metadata.resourceKind === target.resourceKind
  );
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
  if (uiStore.getState().modals.length > 0 || uiStore.getState().progress || isAppModalOpen())
    return true;
  return eventPath(event).some(consumesEditorShortcut);
}

/** Resolve keyboard ownership from the DOM, including portaled/shadow-root event paths. */
export function keyboardPanelInstanceId(event: KeyboardEvent): string | null {
  const targets = [...eventPath(event)];
  let focused = document.activeElement;
  while (focused?.shadowRoot?.activeElement) focused = focused.shadowRoot.activeElement;
  if (focused) targets.push(focused);
  for (const target of targets) {
    const panel = targetElement(target)?.closest("[data-panel-instance-id]");
    const id = panel?.getAttribute("data-panel-instance-id");
    if (id) return id;
  }
  return null;
}

export function captureEditorShortcutTarget(event: KeyboardEvent): EditorCommandTarget | null {
  const id = keyboardPanelInstanceId(event);
  return id ? captureEditorCommandTarget(id) : null;
}

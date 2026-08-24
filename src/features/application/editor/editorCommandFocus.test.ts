// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { WorkbenchPanelInfo } from '@/features/core/dockview/workbenchDockviewPort';

const mocks = vi.hoisted(() => ({
  activePanel: null as WorkbenchPanelInfo | null,
  project: { projectInstanceId: 'project-a', epoch: 1 },
  modalCount: 0,
  renderedModalOpen: false,
}));

vi.mock('@/features/core/dockview/workbenchDockviewPort', () => ({
  workbenchDockviewPort: {
    getActiveEditorPanel: () =>
      mocks.activePanel?.metadata.role === 'editor' ? mocks.activePanel : undefined,
  },
}));

vi.mock('@/features/core/projectLifecycle/projectLifecycleAuthority', () => ({
  captureProjectIdentity: () => ({ ...mocks.project }),
  isCurrentProjectIdentity: (
    identity: Readonly<{ projectInstanceId: string; epoch: number }>,
  ) => identity.projectInstanceId === mocks.project.projectInstanceId
    && identity.epoch === mocks.project.epoch,
}));

vi.mock('@/features/core/ui/UIStore', () => ({
  uiStore: {
    getState: () => ({
      modals: Array.from({ length: mocks.modalCount }, (_, index) => ({ id: `${index}` })),
    }),
  },
}));

vi.mock('@/features/core/keyboard', () => ({
  isAppModalOpen: () => mocks.renderedModalOpen,
}));

import {
  captureActiveEditorCommandTarget,
  isEditorCommandTargetCurrent,
  shouldIgnoreEditorShortcutEvent,
} from './editorCommandFocus';

function editorPanel(overrides: Partial<WorkbenchPanelInfo> = {}): WorkbenchPanelInfo {
  return {
    panelInstanceId: 'editor-a',
    groupId: 'group-a',
    component: 'GraphEditor',
    title: 'Main',
    metadata: {
      role: 'editor',
      resourceRef: 'events/Main.yssbi-event',
      resourceKind: 'event',
    },
    active: true,
    location: { type: 'grid' },
    ...overrides,
  };
}

function toolPanel(): WorkbenchPanelInfo {
  return {
    panelInstanceId: 'logs-a',
    groupId: 'group-a',
    component: 'Logs',
    title: 'Logs',
    metadata: { role: 'view', viewId: 'logs' },
    active: true,
    location: { type: 'grid' },
  };
}

function keyEvent(target: Element): KeyboardEvent {
  let root = target;
  while (root.parentElement) root = root.parentElement;
  document.body.appendChild(root);
  const event = new KeyboardEvent('keydown', { bubbles: true, composed: true });
  target.dispatchEvent(event);
  return event;
}

function descendantOf(parent: HTMLElement): HTMLElement {
  const child = document.createElement('span');
  parent.appendChild(child);
  return child;
}

beforeEach(() => {
  document.body.replaceChildren();
  mocks.activePanel = editorPanel();
  mocks.project = { projectInstanceId: 'project-a', epoch: 1 };
  mocks.modalCount = 0;
  mocks.renderedModalOpen = false;
});

describe('editor command focus', () => {
  it('captures only the physical active root editor and denies a later tool activation', () => {
    mocks.activePanel = toolPanel();
    expect(captureActiveEditorCommandTarget()).toBeNull();

    mocks.activePanel = editorPanel();
    const target = captureActiveEditorCommandTarget();
    expect(target).toEqual({
      panelInstanceId: 'editor-a',
      groupId: 'group-a',
      resourceRef: 'events/Main.yssbi-event',
      resourceKind: 'event',
    });
    expect(target && Object.keys(target)).toEqual([
      'panelInstanceId',
      'groupId',
      'resourceRef',
      'resourceKind',
    ]);
    expect(target && isEditorCommandTargetCurrent(target)).toBe(true);

    mocks.activePanel = toolPanel();
    expect(target && isEditorCommandTargetCurrent(target)).toBe(false);
  });

  it.each([
    ['panel', () => { mocks.activePanel = editorPanel({ panelInstanceId: 'editor-b' }); }],
    ['group', () => { mocks.activePanel = editorPanel({ groupId: 'group-b' }); }],
    ['resource path', () => {
      mocks.activePanel = editorPanel({
        metadata: {
          role: 'editor',
          resourceRef: 'events/Other.yssbi-event',
          resourceKind: 'event',
        },
      });
    }],
    ['resource kind', () => {
      mocks.activePanel = editorPanel({
        metadata: {
          role: 'editor',
          resourceRef: 'events/Main.yssbi-event',
          resourceKind: 'function',
        },
      });
    }],
    ['project generation', () => { mocks.project.epoch += 1; }],
  ] as const)('rejects a stale %s command target', (_label, makeStale) => {
    const target = captureActiveEditorCommandTarget();
    expect(target).not.toBeNull();
    if (!target) return;
    expect(isEditorCommandTargetCurrent(target)).toBe(true);

    makeStale();

    expect(isEditorCommandTargetCurrent(target)).toBe(false);
  });

  it.each([
    ['input', () => document.createElement('input')],
    ['textarea', () => document.createElement('textarea')],
    ['contenteditable', () => {
      const editable = document.createElement('div');
      editable.contentEditable = 'true';
      return descendantOf(editable);
    }],
    ['dialog', () => {
      const dialog = document.createElement('div');
      dialog.setAttribute('role', 'dialog');
      return descendantOf(dialog);
    }],
    ['menu', () => {
      const menu = document.createElement('div');
      menu.setAttribute('role', 'menu');
      return descendantOf(menu);
    }],
    ['listbox', () => {
      const listbox = document.createElement('div');
      listbox.setAttribute('role', 'listbox');
      return descendantOf(listbox);
    }],
    ['combobox', () => {
      const combobox = document.createElement('div');
      combobox.setAttribute('role', 'combobox');
      return descendantOf(combobox);
    }],
    ['popover', () => {
      const popover = document.createElement('div');
      popover.dataset.slot = 'popover-content';
      return descendantOf(popover);
    }],
  ] as const)('ignores shortcuts from a %s target or descendant', (_label, createTarget) => {
    const target = createTarget();
    expect(shouldIgnoreEditorShortcutEvent(keyEvent(target))).toBe(true);
  });

  it('uses the real composed path for a shadow-root popover descendant', () => {
    const host = document.createElement('div');
    document.body.appendChild(host);
    const shadowRoot = host.attachShadow({ mode: 'open' });
    const popover = document.createElement('div');
    popover.dataset.slot = 'popover-content';
    const popoverChild = descendantOf(popover);
    shadowRoot.appendChild(popover);
    let observedPath: readonly EventTarget[] = [];
    let ignored = false;
    document.addEventListener('keydown', (event) => {
      observedPath = event.composedPath();
      ignored = shouldIgnoreEditorShortcutEvent(event as KeyboardEvent);
    }, { once: true });

    popoverChild.dispatchEvent(new KeyboardEvent('keydown', {
      bubbles: true,
      composed: true,
    }));

    expect(observedPath).toContain(popover);
    expect(observedPath).toContain(host);
    expect(ignored).toBe(true);
  });

  it('allows a clean canvas target but denies application modal state', () => {
    const canvas = document.createElement('div');
    canvas.dataset.editorCanvas = 'true';
    const event = keyEvent(canvas);

    expect(shouldIgnoreEditorShortcutEvent(event)).toBe(false);

    mocks.modalCount = 1;
    expect(shouldIgnoreEditorShortcutEvent(event)).toBe(true);

    mocks.modalCount = 0;
    mocks.renderedModalOpen = true;
    expect(shouldIgnoreEditorShortcutEvent(event)).toBe(true);
  });
});

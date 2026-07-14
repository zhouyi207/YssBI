import { describe, expect, it } from 'vitest';
import { acceptsDragStart } from './dragSurface';
import { isDragToOpenWindowOperation } from './dragToOpenWindowPolicy';
import { shouldOpenAuxiliaryWindowOnDragEnd } from './evaluateDragToOpenWindow';
import { isPointInsideFocusedWindow, resolveAuxiliaryWindowBounds } from './screenGeometry';
import { workbenchDragTransfer } from './workbenchDragTransfer';

describe('workbenchDragTransfer', () => {
  it('tracks in-memory drag session', () => {
    workbenchDragTransfer.clearData();
    expect(workbenchDragTransfer.hasData()).toBe(false);
    workbenchDragTransfer.setData({ kind: 'logs-panel' });
    expect(workbenchDragTransfer.getData()).toEqual({ kind: 'logs-panel' });
    workbenchDragTransfer.clearData();
    expect(workbenchDragTransfer.hasData()).toBe(false);
  });
});

describe('dragSurface', () => {
  it('rejects interactive children in header-row mode', () => {
    const handle = {
      contains: (node: unknown) => node === button,
    };
    const button = {
      closest: (selector: string) => (selector.includes('button') ? button : null),
    };
    expect(acceptsDragStart(
      { target: button, currentTarget: handle } as unknown as Pick<DragEvent, 'target' | 'currentTarget'>,
      'header-row',
    )).toBe(false);
  });
});

describe('dragToOpenWindowPolicy', () => {
  it('matches VS Code dragToOpenWindow + Alt toggle', () => {
    expect(isDragToOpenWindowOperation({ altKey: false }, true)).toBe(true);
    expect(isDragToOpenWindowOperation({ altKey: true }, true)).toBe(false);
    expect(isDragToOpenWindowOperation({ altKey: false }, false)).toBe(false);
    expect(isDragToOpenWindowOperation({ altKey: true }, false)).toBe(true);
  });
});

describe('screenGeometry', () => {
  it('resolves auxiliary bounds with title bar offset', () => {
    const bounds = resolveAuxiliaryWindowBounds(
      { x: 200, y: 200 },
      { offsetWidth: 100, offsetHeight: 20 },
    );
    expect(bounds).toEqual({ x: 150, y: 160 });
  });

  it('clamps bounds to display origin', () => {
    const bounds = resolveAuxiliaryWindowBounds(
      { x: 10, y: 10 },
      { offsetWidth: 100, offsetHeight: 20 },
      { display: { x: 50, y: 40 } },
    );
    expect(bounds).toEqual({ x: 50, y: 40 });
  });
});

describe('evaluateDragToOpenWindow', () => {
  const dragElement = { tagName: 'DIV' } as HTMLElement;

  it('rejects when drag target is not the handle', () => {
    const child = { tagName: 'SPAN' } as HTMLElement;
    expect(shouldOpenAuxiliaryWindowOnDragEnd({
      event: { target: child },
      dragElement,
      isNewWindowOperation: true,
      cursorPoint: { x: 9999, y: 9999 },
    })).toBe(false);
  });

  it('rejects when Alt disables new-window operation', () => {
    expect(shouldOpenAuxiliaryWindowOnDragEnd({
      event: { target: dragElement },
      dragElement,
      isNewWindowOperation: false,
      cursorPoint: { x: 9999, y: 9999 },
    })).toBe(false);
  });

  it('rejects when cursor is still inside focused window', () => {
    const targetWindow = {
      document: { visibilityState: 'visible', hasFocus: () => true },
      screenX: 0,
      screenY: 0,
      outerWidth: 1200,
      outerHeight: 800,
    } as Window;

    expect(isPointInsideFocusedWindow({ x: 100, y: 100 }, targetWindow)).toBe(true);
    expect(shouldOpenAuxiliaryWindowOnDragEnd({
      event: { target: dragElement },
      dragElement,
      isNewWindowOperation: true,
      cursorPoint: { x: 100, y: 100 },
      targetWindow,
    })).toBe(false);
  });
});

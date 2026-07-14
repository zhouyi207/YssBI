import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { createInitialWorkbenchNodes } from './workbenchLayoutDefaults';
import {
  findWorkbenchChromePartAtPointer,
  isPointerOverWorkbenchEditorSurface,
  isSidebarItemDropAllowedAtPointer,
  resolveWorkbenchDropSurfaceFlags,
  WORKBENCH_CHROME_PART_ATTR,
  WORKBENCH_EDITOR_SURFACE_ATTR,
} from './workbenchSidebarDropSurface';

function mockElementsFromPoint(factory: (x: number, y: number) => Element[]) {
  vi.stubGlobal('document', {
    elementsFromPoint: vi.fn((x: number, y: number) => factory(x, y)),
  });
}

describe('workbenchSidebarDropSurface', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('marks chrome parts and editor descendants from layout node ids', () => {
    const nodes = createInitialWorkbenchNodes();

    expect(resolveWorkbenchDropSurfaceFlags('sidebar', nodes)).toEqual({
      chromePart: 'sidebar',
    });
    expect(resolveWorkbenchDropSurfaceFlags('panel', nodes)).toEqual({
      chromePart: 'panel',
    });
    expect(resolveWorkbenchDropSurfaceFlags('detail', nodes)).toEqual({
      chromePart: 'detail',
    });
    expect(resolveWorkbenchDropSurfaceFlags('default_editor', nodes)).toEqual({
      editorSurface: true,
    });
    expect(resolveWorkbenchDropSurfaceFlags('center', nodes)).toEqual({});
  });

  describe('pointer hit tests', () => {
    beforeEach(() => {
      const sidebar = {
        closest: (selector: string) => (
          selector === `[${WORKBENCH_CHROME_PART_ATTR}]` ? sidebar : null
        ),
        getAttribute: () => 'sidebar',
      };
      const editor = {
        closest: (selector: string) => (
          selector === `[${WORKBENCH_EDITOR_SURFACE_ATTR}]` ? editor : null
        ),
      };

      mockElementsFromPoint((x) => {
        if (x === 10) return [sidebar as unknown as Element];
        if (x === 150) return [editor as unknown as Element];
        return [];
      });
    });

    it('rejects sidebar chrome and accepts editor surface for sidebar item drops', () => {
      expect(findWorkbenchChromePartAtPointer(10, 10)).toBe('sidebar');
      expect(isPointerOverWorkbenchEditorSurface(10, 10)).toBe(false);
      expect(isSidebarItemDropAllowedAtPointer(10, 10)).toBe(false);

      expect(findWorkbenchChromePartAtPointer(150, 10)).toBeNull();
      expect(isPointerOverWorkbenchEditorSurface(150, 10)).toBe(true);
      expect(isSidebarItemDropAllowedAtPointer(150, 10)).toBe(true);
    });
  });

  it('rejects panel chrome for sidebar item drops', () => {
    const panel = {
      closest: (selector: string) => (
        selector === `[${WORKBENCH_CHROME_PART_ATTR}]` ? panel : null
      ),
      getAttribute: () => 'panel',
    };

    mockElementsFromPoint(() => [panel as unknown as Element]);

    expect(findWorkbenchChromePartAtPointer(20, 20)).toBe('panel');
    expect(isSidebarItemDropAllowedAtPointer(20, 20)).toBe(false);
  });

  it('rejects detail chrome for sidebar item drops', () => {
    const detail = {
      closest: (selector: string) => (
        selector === `[${WORKBENCH_CHROME_PART_ATTR}]` ? detail : null
      ),
      getAttribute: () => 'detail',
    };

    mockElementsFromPoint(() => [detail as unknown as Element]);

    expect(findWorkbenchChromePartAtPointer(30, 30)).toBe('detail');
    expect(isSidebarItemDropAllowedAtPointer(30, 30)).toBe(false);
  });
});

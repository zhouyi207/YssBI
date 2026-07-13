import { describe, expect, it } from 'vitest';
import type { LayoutNode } from '@/shared/types/ui';
import {
  isWorkbenchPartUserHidden,
  shouldRestoreWorkbenchPartOnSashDrag,
} from './workbenchPartVisibility';

const hiddenSidebar = (userHidden: boolean): LayoutNode => ({
  id: 'sidebar',
  type: 'component',
  parentId: 'root',
  data: { component: 'Sidebar', visible: false, userHidden },
});

describe('workbenchPartVisibility', () => {
  it('treats userHidden chrome parts as non-restorable on sash drag', () => {
    expect(shouldRestoreWorkbenchPartOnSashDrag(hiddenSidebar(true))).toBe(false);
    expect(isWorkbenchPartUserHidden(hiddenSidebar(true))).toBe(true);
  });

  it('allows sash restore for collapsed chrome without userHidden', () => {
    expect(shouldRestoreWorkbenchPartOnSashDrag(hiddenSidebar(false))).toBe(true);
  });
});

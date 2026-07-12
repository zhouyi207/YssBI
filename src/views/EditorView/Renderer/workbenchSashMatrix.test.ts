import { describe, expect, it } from 'vitest';
import { createInitialWorkbenchNodes, EDITOR_AREA_ID, PANEL_PART_ID } from '@/features/core/layout/workbenchLayoutDefaults';
import { isEditorGridSash } from '@/features/core/layout/editorGridLayout';
import { layoutNodeFlexStyle } from '@/views/EditorView/Renderer/sashResizeLogic';
import { resolveSashResizeTarget } from '@/views/EditorView/Renderer/sashResizeLogic';

/** Two-layer sash scenarios: workbench chrome vs editor grid. */
describe('workbench sash matrix', () => {
  const nodes = createInitialWorkbenchNodes();

  it('sidebar sash targets fixed pixel part', () => {
    const target = resolveSashResizeTarget(
      'row',
      nodes.sidebar,
      nodes.center,
      260,
      800,
    );
    expect(target?.nodeId).toBe('sidebar');
  });

  it('editor↔panel sash is workbench chrome, not editor grid', () => {
    expect(isEditorGridSash(EDITOR_AREA_ID, PANEL_PART_ID, nodes)).toBe(false);
  });

  it('panel maximize style shrinks editor_area strip (bottom dock)', () => {
    const style = layoutNodeFlexStyle(nodes[EDITOR_AREA_ID], { panelMaximized: true, panelPosition: 'bottom' });
    expect(style.flex).toBe('0 0 80px');
    expect(style.minHeight).toBe(80);
  });

  it('panel maximize style shrinks editor_area strip (side dock)', () => {
    const style = layoutNodeFlexStyle(nodes[EDITOR_AREA_ID], { panelMaximized: true, panelPosition: 'left' });
    expect(style.flex).toBe('0 0 80px');
    expect(style.minWidth).toBe(80);
  });

  it('editor group sash inside editor_area is editor grid', () => {
    const splitNodes = {
      ...nodes,
      editor_group_b: {
        id: 'editor_group_b',
        type: 'component' as const,
        parentId: EDITOR_AREA_ID,
        data: { component: 'GraphEditor', tabs: [] },
      },
    };
    expect(isEditorGridSash('default_editor', 'editor_group_b', splitNodes)).toBe(true);
  });
});

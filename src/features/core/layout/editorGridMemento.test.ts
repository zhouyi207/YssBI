import { describe, expect, it } from 'vitest';
import {
  applyEditorGridMementoWithRepair,
  repairEditorGridIntegrity,
  type EditorGridMemento,
} from './editorGridMemento';
import { createInitialWorkbenchNodes, DEFAULT_EDITOR_GROUP_ID, EDITOR_AREA_ID } from './workbenchLayoutDefaults';

describe('editorGridMemento repair', () => {
  it('recreates default_editor when memento children reference it without a snapshot', () => {
    const initial = createInitialWorkbenchNodes();
    delete initial[DEFAULT_EDITOR_GROUP_ID];

    const memento: EditorGridMemento = {
      activeEditorGroupId: DEFAULT_EDITOR_GROUP_ID,
      nodes: [
        {
          id: EDITOR_AREA_ID,
          type: 'row',
          parentId: 'center',
          children: [DEFAULT_EDITOR_GROUP_ID],
          size: 1,
        },
      ],
    };

    const next = applyEditorGridMementoWithRepair(initial, memento);
    expect(next[EDITOR_AREA_ID]).toBeDefined();
    expect(next[DEFAULT_EDITOR_GROUP_ID]?.data?.component).toBe('GraphEditor');
  });

  it('repairEditorGridIntegrity restores missing editor_area', () => {
    const nodes = createInitialWorkbenchNodes();
    delete nodes[EDITOR_AREA_ID];

    const repaired = repairEditorGridIntegrity(nodes);
    expect(repaired[EDITOR_AREA_ID]?.children).toContain(DEFAULT_EDITOR_GROUP_ID);
  });
});

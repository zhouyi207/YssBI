import { beforeEach, describe, expect, it } from 'vitest';

import { getPaneSelection, useEditorPaneStateStore } from './editorPaneStateStore';

describe('editor pane selection snapshots', () => {
  beforeEach(() => {
    useEditorPaneStateStore.getState().reset();
  });

  it('keeps the empty selection snapshot stable for an uninitialized panel', () => {
    const first = getPaneSelection('panel-a');
    const second = getPaneSelection('panel-a');

    expect(second).toBe(first);
  });
});

import { describe, expect, it } from 'vitest';
import { buildSidebarDragData } from '@/features/application/sidebar/buildSidebarDragData';
import { isSidebarSpawnDrag } from '@/features/core/dnd';
import { isSidebarSpawnDropAllowed } from './sidebarSpawnDropPolicy';

describe('sidebarSpawnDropPolicy', () => {
  it('rejects sidebar spawn drops without a pointer', () => {
    const payload = buildSidebarDragData('graphs/main', 'Main', 'event');
    expect(isSidebarSpawnDropAllowed(payload, null)).toBe(false);
  });

  it('rejects non-sidebar drag payloads', () => {
    expect(
      isSidebarSpawnDropAllowed(
        { type: 'tab', tabId: 'a', sourceNodeId: 'g' },
        { x: 1, y: 1 },
      ),
    ).toBe(false);
  });

  it('recognizes variable sidebar spawn payloads with an exact backend descriptor', () => {
    const descriptor = {
      kind: 'resourceBound' as const,
      nodeTypeId: 'variable.get',
      resourcePath: 'variables/v1',
      resourceRevision: 2,
      createArgs: { kind: 'variable' as const },
    };
    const payload = buildSidebarDragData('v1', 'X', 'variable', descriptor);

    expect(payload).not.toBeNull();
    expect(isSidebarSpawnDrag(payload)).toBe(true);
    expect(payload?.type).toBe('node-template');
    if (payload?.type !== 'node-template') {
      throw new Error('Expected a node-template sidebar spawn payload');
    }
    expect(payload.template.descriptor).toBe(descriptor);
  });
});

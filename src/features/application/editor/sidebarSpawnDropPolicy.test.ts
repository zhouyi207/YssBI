import { describe, expect, it } from 'vitest';
import { buildSidebarDragData } from '@/features/application/sidebar/buildSidebarDragData';
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
});

import { describe, expect, it } from 'vitest';
import type { GraphInteractionState } from '@/features/core/graphInteraction/graphInteractionStore';
import { selectionInteractionForScope } from './useSelectionBoxPreview';

const selecting = {
  type: 'selecting' as const,
  session: {
    groupId: 'group-a',
    startX: 0,
    startY: 0,
    currentX: 20,
    currentY: 20,
    preserveSelection: false,
  },
};

describe('selectionInteractionForScope', () => {
  it('requires both graph path and initiating group id', () => {
    const state = {
      interactions: {
        'events/current': selecting,
        'events/other': { ...selecting, session: { ...selecting.session, groupId: 'group-b' } },
      },
    } as Pick<GraphInteractionState, 'interactions'>;

    expect(selectionInteractionForScope(state, 'events/current', 'group-a')).toBe(selecting);
    expect(selectionInteractionForScope(state, 'events/current', 'group-b')).toBeNull();
    expect(selectionInteractionForScope(state, 'events/other', 'group-a')).toBeNull();
  });
});

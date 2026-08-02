import { Children, isValidElement, type ReactElement } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { GraphTraceDetails } from '../observability/GraphTraceDetails';
import { EventDetailPanel } from './EventDetailPanel';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

function findByType(root: ReactElement, type: unknown): ReactElement | undefined {
  let match: ReactElement | undefined;
  function visit(node: unknown) {
    if (!isValidElement(node) || match) return;
    if (node.type === type) {
      match = node;
      return;
    }
    Children.forEach((node.props as { children?: unknown }).children, visit);
  }
  visit(root);
  return match;
}

describe('EventDetailPanel', () => {
  it('embeds read-only developer traces for the event graph', () => {
    const element = EventDetailPanel({
      event: { path: 'events/Main.yssbi-event', name: 'Main' },
      onUpdate: vi.fn(),
    }) as ReactElement;

    const traceDetails = findByType(element, GraphTraceDetails);
    expect(traceDetails).toBeDefined();
    expect(traceDetails?.props).toMatchObject({ graphPath: 'events/Main.yssbi-event' });
  });
});

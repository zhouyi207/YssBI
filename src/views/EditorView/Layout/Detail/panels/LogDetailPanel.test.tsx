import { Children, isValidElement, type ReactElement } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { DetailCollapsibleSection } from '../shared/DetailCollapsibleSection';
import { LogDetailPanel } from './LogDetailPanel';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

function findByType(root: ReactElement, type: unknown): ReactElement | undefined {
  let match: ReactElement | undefined;
  function visit(node: unknown): void {
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

describe('LogDetailPanel', () => {
  it('renders the message as a separate expanded detail section', () => {
    const panel = LogDetailPanel({
      log: {
        timestamp: '2026-07-14 17:28:00',
        level: 'error',
        log_type: 'application',
        message: 'First line\nSecond line',
      },
    });

    const messageSection = findByType(panel, DetailCollapsibleSection);
    expect(messageSection?.props.title).toBe('detail.fields.message');
    expect(messageSection?.props.defaultOpen).toBe(true);
  });
});

// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LogLevel, LogType } from '@/shared/types/ui';
import { LogDetailPanel } from './LogDetailPanel';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key,
  }),
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe('LogDetailPanel', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('locally restores selection for rendered metadata and multiline message content', () => {
    act(() => {
      root.render(
        <div className="select-none">
          <LogDetailPanel
            log={{
              timestamp: '2026-07-14 17:28:00',
              level: LogLevel.Error,
              log_type: LogType.Graph,
              source: 'events/on-start',
              message: 'First line\nSecond line',
            }}
          />
        </div>,
      );
    });

    const inheritedBoundary = host.querySelector('.select-none');
    expect(inheritedBoundary).not.toBeNull();

    const cards = Array.from(inheritedBoundary?.querySelectorAll('[data-slot="card"]') ?? []);
    const metadataCard = cards.find(
      (card) => card.textContent?.includes('2026-07-14 17:28:00')
        && card.textContent.includes('error')
        && card.textContent.includes('GRAPH')
        && card.textContent.includes('events/on-start'),
    );
    expect(metadataCard).toBeDefined();
    expect(metadataCard?.classList.contains('select-text')).toBe(true);

    const messageCard = cards.find((card) => card.textContent?.includes('First line\nSecond line'));
    const messageBoundary = messageCard?.querySelector('[data-slot="card-content"].select-text');
    const message = messageBoundary?.querySelector('pre');
    expect(messageBoundary).not.toBeNull();
    expect(message?.classList.contains('whitespace-pre-wrap')).toBe(true);
    expect(message?.classList.contains('break-words')).toBe(true);
    expect(message?.textContent).toBe('First line\nSecond line');
  });
});

// @vitest-environment happy-dom

import { act, useState } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { Tooltip, TooltipProvider, TooltipTrigger } from './tooltip';

function ControlledTooltip({ onOpenChange }: { onOpenChange: (open: boolean) => void }) {
  const [open, setOpen] = useState(true);

  return (
    <Tooltip
      open={open}
      onOpenChange={(nextOpen) => {
        setOpen(nextOpen);
        onOpenChange(nextOpen);
      }}
    >
      <TooltipTrigger data-testid="active-trigger">Active</TooltipTrigger>
    </Tooltip>
  );
}

describe('Tooltip window drag behavior', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeAll(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  });

  afterAll(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = false;
  });

  beforeEach(() => {
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('closes only the active tooltip and guards opening until the window drag ends', () => {
    const activeOnOpenChange = vi.fn();
    const inactiveOnOpenChange = vi.fn();

    act(() => {
      root.render(
        <TooltipProvider>
          <ControlledTooltip onOpenChange={activeOnOpenChange} />
          <Tooltip onOpenChange={inactiveOnOpenChange}>
            <TooltipTrigger data-testid="inactive-trigger">Inactive</TooltipTrigger>
          </Tooltip>
        </TooltipProvider>,
      );
    });

    const activeTrigger = host.querySelector<HTMLElement>('[data-testid="active-trigger"]');
    const inactiveTrigger = host.querySelector<HTMLElement>('[data-testid="inactive-trigger"]');
    expect(activeTrigger?.dataset.state).not.toBe('closed');
    expect(inactiveTrigger?.dataset.state).toBe('closed');

    act(() => window.dispatchEvent(new Event('yssbi-window-drag-start')));

    expect(activeOnOpenChange).toHaveBeenCalledOnce();
    expect(activeOnOpenChange).toHaveBeenLastCalledWith(false);
    expect(inactiveOnOpenChange).not.toHaveBeenCalled();
    expect(activeTrigger?.dataset.state).toBe('closed');

    act(() => inactiveTrigger?.focus());

    expect(inactiveOnOpenChange).not.toHaveBeenCalled();
    expect(inactiveTrigger?.dataset.state).toBe('closed');

    act(() => inactiveTrigger?.blur());
    act(() => window.dispatchEvent(new Event('yssbi-window-drag-end')));
    act(() => inactiveTrigger?.focus());

    expect(inactiveOnOpenChange).toHaveBeenCalledOnce();
    expect(inactiveOnOpenChange).toHaveBeenLastCalledWith(true);
    expect(inactiveTrigger?.dataset.state).not.toBe('closed');
  });
});

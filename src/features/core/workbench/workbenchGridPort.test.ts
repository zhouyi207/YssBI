import type { GridviewApi, SerializedGridviewComponent } from 'dockview-react';
import { describe, expect, it, vi } from 'vitest';

import { createWorkbenchGridPort } from './workbenchGridPort';

function layout(id: string): SerializedGridviewComponent {
  return {
    grid: {
      root: { type: 'leaf', data: { id, component: id } },
      height: 100,
      width: 100,
      orientation: 'HORIZONTAL',
    },
  } as unknown as SerializedGridviewComponent;
}

function fakeApi(initial: SerializedGridviewComponent) {
  let current = structuredClone(initial);
  const listeners = new Set<() => void>();
  const fromJSON = vi.fn((value: SerializedGridviewComponent) => {
    current = structuredClone(value);
  });
  const api = {
    toJSON: () => structuredClone(current),
    fromJSON,
    onDidLayoutChange: (listener: () => void) => {
      listeners.add(listener);
      return { dispose: () => listeners.delete(listener) };
    },
    getPanel: vi.fn(),
  } as unknown as GridviewApi;
  return { api, fromJSON, setCurrent: (value: SerializedGridviewComponent) => { current = structuredClone(value); } };
}

describe('WorkbenchGridPort', () => {
  it('restores the canonical layout captured when the initialized grid binds', () => {
    const canonical = layout('default');
    const fake = fakeApi(canonical);
    const port = createWorkbenchGridPort();
    port.bind(fake.api);
    fake.setCurrent(layout('custom'));

    port.resetToDefault();

    expect(fake.fromJSON).toHaveBeenCalledWith(canonical);
  });

  it('uses the last pending restore or reset command when binding', () => {
    const canonical = layout('default');
    const custom = layout('custom');
    const restoreLast = createWorkbenchGridPort();
    restoreLast.resetToDefault();
    restoreLast.restore(custom);
    const restoredFake = fakeApi(canonical);
    restoreLast.bind(restoredFake.api);
    expect(restoredFake.fromJSON).toHaveBeenCalledWith(custom);

    const resetLast = createWorkbenchGridPort();
    resetLast.restore(custom);
    resetLast.resetToDefault();
    const resetFake = fakeApi(canonical);
    resetLast.bind(resetFake.api);
    expect(resetFake.fromJSON).toHaveBeenCalledWith(canonical);
  });

  it('keeps its default snapshot isolated from Dockview mutations', () => {
    const canonical = layout('default');
    const fake = fakeApi(canonical);
    fake.fromJSON.mockImplementation((value: SerializedGridviewComponent) => {
      (value as unknown as { mutated?: boolean }).mutated = true;
    });
    const port = createWorkbenchGridPort();
    port.bind(fake.api);

    port.resetToDefault();
    port.resetToDefault();

    const secondReset = fake.fromJSON.mock.calls[1]?.[0] as unknown as { mutated?: boolean };
    expect(secondReset.mutated).toBe(true);
    expect(fake.fromJSON.mock.calls[0]?.[0]).not.toBe(fake.fromJSON.mock.calls[1]?.[0]);
  });
});

import type { DockviewApi, SerializedDockview } from 'dockview-react';
import { describe, expect, it, vi } from 'vitest';

import { createDefaultLogsDockviewLayout } from './logsDockviewLayout';
import { createLogsDockviewLayoutController } from './logsDockviewLayoutController';

type Listener = () => void;

type MutableGroup = {
  activeView?: string;
};

function getOnlyGridGroup(layout: SerializedDockview): MutableGroup {
  const root = layout.grid.root;
  if (root.type !== 'branch' || !Array.isArray(root.data) || root.data.length !== 1) {
    throw new Error('default Logs layout must use a top-level branch with one group');
  }
  const child = root.data[0];
  if (child.type !== 'leaf') throw new Error('default Logs layout child must be a group leaf');
  return child.data as MutableGroup;
}

function assertRestorableLayout(layout: SerializedDockview): void {
  const root = layout.grid.root;
  if (root.type !== 'branch' || !Array.isArray(root.data)) {
    throw new Error('Dockview layouts require a top-level branch');
  }
}

function layoutWithActiveDomain(domain: string): SerializedDockview {
  const layout = createDefaultLogsDockviewLayout();
  getOnlyGridGroup(layout).activeView = `logs-domain:${domain}`;
  return layout;
}

function createFakeLogsDockview(
  initialLayout: SerializedDockview,
  order: string[] = [],
) {
  assertRestorableLayout(initialLayout);
  let layout = structuredClone(initialLayout);
  const listeners = new Set<Listener>();
  const fromJSON = vi.fn((next: SerializedDockview) => {
    assertRestorableLayout(next);
    order.push('fromJSON');
    layout = structuredClone(next);
  });
  const toJSON = vi.fn(() => {
    order.push('toJSON');
    return structuredClone(layout);
  });
  const dispose = vi.fn(() => {
    order.push('dispose');
    listeners.clear();
  });
  const onDidLayoutChange = vi.fn((listener: Listener) => {
    listeners.add(listener);
    return { dispose };
  });
  const api = {
    fromJSON,
    toJSON,
    onDidLayoutChange,
  } as unknown as DockviewApi;

  return {
    api,
    dispose,
    fromJSON,
    onDidLayoutChange,
    toJSON,
    setLayout(next: SerializedDockview, notify = true): void {
      layout = structuredClone(next);
      if (notify) [...listeners].forEach((listener) => listener());
    },
  };
}

describe('LogsDockviewLayoutController', () => {
  it('applies each pending restore once and publishes only snapshot changes', () => {
    const defaultLayout = createDefaultLogsDockviewLayout();
    const savedLayout = layoutWithActiveDomain('application');
    const newerLayout = layoutWithActiveDomain('execution');
    const controller = createLogsDockviewLayoutController(defaultLayout);
    const listener = vi.fn();
    controller.subscribe(listener);

    const epoch = controller.beginRestore();
    expect(controller.stageRestore(epoch, savedLayout)).toBe('staged');
    expect(listener).toHaveBeenCalledOnce();

    const fake = createFakeLogsDockview(defaultLayout);
    controller.bind(fake.api);
    controller.bind(fake.api);
    expect(fake.onDidLayoutChange).toHaveBeenCalledOnce();
    expect(fake.fromJSON).toHaveBeenCalledOnce();
    expect(fake.fromJSON).toHaveBeenLastCalledWith(savedLayout);

    fake.setLayout(savedLayout);
    expect(listener).toHaveBeenCalledOnce();
    fake.setLayout(newerLayout);
    fake.setLayout(newerLayout);
    expect(listener).toHaveBeenCalledTimes(2);
    expect(controller.getLatestSnapshot()).toEqual(newerLayout);

    controller.unbind(fake.api);
    expect(listener).toHaveBeenCalledTimes(2);
    controller.bind(fake.api);
    expect(fake.fromJSON).toHaveBeenCalledTimes(2);
    expect(fake.fromJSON).toHaveBeenLastCalledWith(newerLayout);
  });

  it('rejects a staged restore that predates reset', () => {
    const defaultLayout = createDefaultLogsDockviewLayout();
    const savedLayout = layoutWithActiveDomain('graph');
    const controller = createLogsDockviewLayoutController(defaultLayout);
    const epoch = controller.beginRestore();

    controller.resetToDefault();

    expect(controller.stageRestore(epoch, savedLayout)).toBe('stale');
    expect(controller.getLatestSnapshot()).toEqual(defaultLayout);
  });

  it('captures the live layout before unbind notification and disposal', () => {
    const defaultLayout = createDefaultLogsDockviewLayout();
    const newerLayout = layoutWithActiveDomain('ui');
    const order: string[] = [];
    const controller = createLogsDockviewLayoutController(defaultLayout);
    const fake = createFakeLogsDockview(defaultLayout, order);
    controller.bind(fake.api);
    order.length = 0;
    controller.subscribe(() => {
      throw new Error('observer failed');
    });
    controller.subscribe(() => order.push('notify'));
    fake.setLayout(newerLayout, false);

    expect(() => controller.unbind(fake.api)).not.toThrow();

    expect(order).toEqual(['toJSON', 'notify', 'dispose']);
    expect(controller.getLatestSnapshot()).toEqual(newerLayout);
  });

  it('releases the API after capture throws so another API can bind', () => {
    const defaultLayout = createDefaultLogsDockviewLayout();
    const controller = createLogsDockviewLayoutController(defaultLayout);
    const first = createFakeLogsDockview(defaultLayout);
    const captureError = new Error('capture failed');
    controller.bind(first.api);
    first.toJSON.mockImplementation(() => {
      throw captureError;
    });

    expect(() => controller.unbind(first.api)).toThrow(captureError);
    expect(first.dispose).toHaveBeenCalledOnce();

    const second = createFakeLogsDockview(defaultLayout);
    expect(() => controller.bind(second.api)).not.toThrow();
    expect(second.fromJSON).toHaveBeenCalledOnce();
    expect(second.fromJSON).toHaveBeenLastCalledWith(defaultLayout);
  });
});

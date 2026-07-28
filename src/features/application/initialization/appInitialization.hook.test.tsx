// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { initProjectSync } from '@/features/core/dataStore';
import { LoadStatus } from '@/shared/types/ui';
import { useAppInitialization } from './appInitialization.hook';
import type { InitializationState } from './appInitialization.type';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (command: string) => {
    if (command === 'get_editor_schema_command') {
      throw new Error('Command get_editor_schema_command not found');
    }
    throw new Error(`Unexpected command: ${command}`);
  }),
}));

vi.mock('@/features/core/dataStore', () => ({
  initProjectSync: vi.fn(),
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let initializationState: InitializationState;

function Harness(): null {
  initializationState = useAppInitialization();
  return null;
}

describe('useAppInitialization', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(initProjectSync).mockResolvedValue(null);
    initializationState = { status: LoadStatus.Idle, error: null };
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    host.remove();
  });

  it('syncs the project without invoking the removed editor schema command', async () => {
    await act(async () => {
      root.render(<Harness />);
    });

    await vi.waitFor(() => {
      expect(initializationState).toEqual({ status: LoadStatus.Ready, error: null });
    });

    expect(initProjectSync).toHaveBeenCalledOnce();
    expect(invoke).not.toHaveBeenCalled();
  });
});

// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { GroupContext } from '@/features/core/editor';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useIsActiveEditorGroup } from './useIsActiveEditorGroup';

function ActiveProbe({ resultRef }: { resultRef: { current: boolean | null } }) {
  resultRef.current = useIsActiveEditorGroup();
  return null;
}

function ActiveProbeWithArg({
  groupId,
  resultRef,
}: {
  groupId: string;
  resultRef: { current: boolean | null };
}) {
  resultRef.current = useIsActiveEditorGroup(groupId);
  return null;
}

describe('useIsActiveEditorGroup', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    root.unmount();
    host.remove();
  });

  it('returns true when context group matches activeEditorGroupId', () => {
    useLayoutStore.setState({ activeEditorGroupId: 'group-a' });
    const result = { current: null as boolean | null };
    act(() => {
      root.render(
        <GroupContext.Provider value="group-a">
          <ActiveProbe resultRef={result} />
        </GroupContext.Provider>,
      );
    });
    expect(result.current).toBe(true);
  });

  it('returns false for a visible but inactive group', () => {
    useLayoutStore.setState({ activeEditorGroupId: 'group-a' });
    const result = { current: null as boolean | null };
    act(() => {
      root.render(
        <GroupContext.Provider value="group-b">
          <ActiveProbe resultRef={result} />
        </GroupContext.Provider>,
      );
    });
    expect(result.current).toBe(false);
  });

  it('accepts an explicit groupId override', () => {
    useLayoutStore.setState({ activeEditorGroupId: 'group-b' });
    const result = { current: null as boolean | null };
    act(() => {
      root.render(
        <GroupContext.Provider value="group-a">
          <ActiveProbeWithArg groupId="group-b" resultRef={result} />
        </GroupContext.Provider>,
      );
    });
    expect(result.current).toBe(true);
  });
});

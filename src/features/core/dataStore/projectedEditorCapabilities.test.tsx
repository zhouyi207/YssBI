// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { beforeEach, describe, expect, it } from 'vitest';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { useRepeatablePinRemovable } from '@/features/core/pin/useRepeatablePinRemovable';
import { useGraphDataStore } from './graphDataStore';
import { canCopyNode, canDeleteNode } from './graphNodeSelectors';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const graphPath = 'functions/projected-capabilities';
const nodeId = 'managed-node';

function installClipboardNode(canCopy: boolean | undefined, managed: boolean) {
  const fixture = makeEditorProjectionFixture({ graphPath, nodeId });
  const capabilities = {
    ...fixture.projection.nodes[0].capabilities,
    managed,
    canCopy: canCopy ?? true,
  };
  if (canCopy === undefined) {
    delete (capabilities as { canCopy?: boolean }).canCopy;
  }
  fixture.projection.nodes[0].capabilities = capabilities;
  useGraphDataStore.getState().replaceProjection(graphPath, fixture.projection, 1);
}

function installProjectedCapabilities() {
  const fixture = makeEditorProjectionFixture({ graphPath, nodeId });
  const node = fixture.projection.nodes[0];
  node.capabilities = {
    ...node.capabilities,
    managed: true,
    canCopy: false,
    canDelete: false,
    hasDynamicPorts: true,
  };
  node.ports[1] = {
    ...node.ports[1],
    canRemove: true,
  };
  useGraphDataStore.getState().replaceProjection(graphPath, fixture.projection, 1);
  return fixture;
}

describe('projected active-editor capabilities', () => {
  beforeEach(() => {
    useGraphDataStore.setState({ graphEntities: {} });
  });

  it('protects Rust-managed nodes without a frontend node registry', () => {
    installProjectedCapabilities();

    expect(canDeleteNode(graphPath, nodeId)).toBe(false);
  });

  it('does not copy a node whose Rust projection disables copying', () => {
    installProjectedCapabilities();

    expect(canCopyNode(graphPath, nodeId)).toBe(false);
  });

  it.each([
    { label: 'claims it is copyable', canCopy: true },
    { label: 'omits canCopy', canCopy: undefined },
  ])('gives managed capability precedence when a node $label', ({ canCopy }) => {
    installClipboardNode(canCopy, true);

    expect(canCopyNode(graphPath, nodeId)).toBe(false);
  });

  it('allows copying an ordinary projected node', () => {
    installClipboardNode(true, false);

    expect(canCopyNode(graphPath, nodeId)).toBe(true);
  });

  it('uses the projected port removal capability without a frontend node definition', async () => {
    const fixture = installProjectedCapabilities();
    let removable = false;
    let host: HTMLDivElement;
    let root: Root;

    function Harness(): null {
      removable = useRepeatablePinRemovable(nodeId, fixture.inputKey, graphPath);
      return null;
    }

    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);

    await act(async () => {
      root.render(<Harness />);
    });

    expect(removable).toBe(true);

    await act(async () => root.unmount());
    host.remove();
  });
});

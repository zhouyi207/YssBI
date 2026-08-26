// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TooltipProvider } from '@/components/ui/tooltip';
import type { GraphEntityBucket } from '@/features/core/dataStore/graphEntityAccess';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useEditorStore } from '@/features/core/editor';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import type { DiagnosticDto } from '@/shared/types/dto/editorProjection';
import { DiagnosticsPanel } from './DiagnosticsPanel';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

const graphPath = 'events/Main.yssbi-event';

function diagnostic(nodeId: string, code: string, message: string): DiagnosticDto {
  return {
    code,
    message,
    severity: code === 'node.error' ? 'error' : 'warning',
    blocking: code === 'node.error',
    location: { kind: 'node', nodeId },
    related: [],
  };
}

const bucket = {
  graphNodes: ['node-a', 'node-b'],
  nodes: {
    'node-a': {
      id: 'node-a',
      title: 'Node A',
      diagnostics: [diagnostic('node-a', 'node.error', 'A is invalid')],
    },
    'node-b': {
      id: 'node-b',
      title: 'Node B',
      diagnostics: [diagnostic('node-b', 'node.warning', 'B needs review')],
    },
  },
} as unknown as GraphEntityBucket;

describe('DiagnosticsPanel', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    useGraphDataStore.setState({ graphEntities: {} });
    useEditorStore.getState().clearDetailFocus();
    useGraphSessionStore.getState().reset();
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('lists diagnostics for every node in the focused graph', () => {
    useGraphDataStore.setState({ graphEntities: { [graphPath]: bucket } });
    useGraphSessionStore.getState().setFocusedSession('group-1', graphPath);

    act(() => {
      root.render(<TooltipProvider><DiagnosticsPanel /></TooltipProvider>);
    });

    expect(host.querySelectorAll('[data-diagnostics-row]')).toHaveLength(2);
    expect(host.textContent).toContain('Node A');
    expect(host.textContent).toContain('A is invalid');
    expect(host.textContent).toContain('Node B');
    expect(host.textContent).toContain('B needs review');

    const firstRow = host.querySelector<HTMLButtonElement>('[data-diagnostics-row]');
    expect(firstRow).not.toBeNull();
    act(() => firstRow?.click());
    expect(useEditorStore.getState().detailFocus).toEqual({
      kind: 'node',
      id: 'node-a',
      graphPath,
    });
  });

  it('renders a shared header without exposing the focused graph path', () => {
    useGraphSessionStore.getState().setFocusedSession('group-1', graphPath);

    act(() => {
      root.render(<TooltipProvider><DiagnosticsPanel /></TooltipProvider>);
    });

    const header = host.querySelector('[data-diagnostics-panel-header]');
    expect(header?.textContent).toContain('panel.diagnostics');
    expect(header?.textContent).toContain('panel.diagnosticsCount');
    expect(header?.textContent).not.toContain(graphPath);
  });

  it('shows an empty state when the focused graph has no node diagnostics', () => {
    useGraphSessionStore.getState().setFocusedSession('group-1', graphPath);
    useGraphDataStore.setState({
      graphEntities: {
        [graphPath]: {
          ...bucket,
          nodes: {
            'node-a': { ...bucket.nodes['node-a'], diagnostics: [] },
            'node-b': { ...bucket.nodes['node-b'], diagnostics: [] },
          },
        },
      },
    });

    act(() => {
      root.render(<TooltipProvider><DiagnosticsPanel /></TooltipProvider>);
    });

    expect(host.querySelectorAll('[data-diagnostics-row]')).toHaveLength(0);
    expect(host.textContent).toContain('panel.diagnosticsEmpty');
  });
});

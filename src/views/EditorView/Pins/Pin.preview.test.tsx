// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterAll, afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { markResourceLoaded, useDocumentStateStore } from '@/features/core/resource';
import { useExecutionStore } from '@/features/core/execution';
import { PinPreviewGenerationService } from '@/services/nodeSystem/pinPreviewGenerationService';
import { ProjectService } from '@/services/project/projectService';
import { TooltipProvider } from '@/components/ui/tooltip';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { Pin } from './Pin';

const katexWarningSpy = vi.hoisted(() => {
  const warn = console.warn.bind(console);
  return vi.spyOn(console, 'warn').mockImplementation((message, ...args) => {
    const quirksWarning = "Warning: KaTeX doesn't work in quirks mode. Make sure your website has a suitable doctype.";
    if (message !== quirksWarning) warn(message, ...args);
  });
});

vi.mock('@/features/application/window', () => ({
  openPresentationWindow: vi.fn(),
  presentationWindowPayload: vi.fn(() => ({})),
  presentationWindowPayloadFromDescriptor: vi.fn(() => ({})),
}));

vi.mock('react-i18next', async (importOriginal) => ({
  ...(await importOriginal<typeof import('react-i18next')>()),
  useTranslation: () => ({ t: (key: string) => key }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

const graphPath = 'events/Main.yssbi-event';

describe('Pin preview production path', () => {
  afterAll(() => katexWarningSpy.mockRestore());
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.restoreAllMocks();
    clearProjectLifecycle();
    startProjectLifecycle('project-session-1');
    useGraphDataStore.setState({ graphEntities: {} });
    useGraphSessionStore.getState().reset();
    useDocumentStateStore.getState().clear();
    useExecutionStore.setState({
      graphs: {},
      playbackGraphPath: null,
      isPlaying: false,
    });
    vi.spyOn(PinPreviewGenerationService, 'allocate').mockResolvedValue(1);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.querySelector('[data-yssbi-overlay-root]')?.remove();
    vi.restoreAllMocks();
  });

  it('routes the top-level Event data-output View action through application preview to service', async () => {
    const fixture = makeEditorProjectionFixture({ graphPath });
    expect(useGraphDataStore.getState().replaceProjection(
      graphPath,
      fixture.projection,
      1,
    ).applied).toBe(true);
    markResourceLoaded({ id: graphPath, kind: 'event' });
    useGraphSessionStore.getState().setFocusedSession('editor-a', graphPath);
    const pin = useGraphDataStore.getState().getGraphPin(graphPath, fixture.outputKey);
    if (!pin) throw new Error('expected projected output pin');
    const commandError = { code: 'test_stop', message: 'stop after preview invoke' };
    const execute = vi.spyOn(ProjectService, 'executeGraphDocument').mockRejectedValue(commandError);

    act(() => root.render(
      <TooltipProvider>
        <Pin {...pin} graphPath={graphPath} />
      </TooltipProvider>,
    ));
    const pinElement = container.querySelector(`[data-pin-id="${fixture.outputKey}"]`);
    if (!pinElement) throw new Error('expected rendered pin');
    act(() => {
      pinElement.dispatchEvent(new MouseEvent('contextmenu', {
        bubbles: true,
        cancelable: true,
        clientX: 10,
        clientY: 20,
      }));
    });

    const viewItem = [...document.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')]
      .find((item) => item.textContent?.includes('contextMenu.pin.view'));
    expect(viewItem).toBeDefined();
    expect(viewItem?.disabled).toBe(false);

    await act(async () => {
      viewItem?.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }));
      await Promise.resolve();
    });

    expect(execute).toHaveBeenCalledWith(
      'project-session-1',
      graphPath,
      {
        type: 'pinPreview',
        output: { graphPath, port: fixture.outputAddress },
        generation: 1,
      },
      expect.any(Function),
    );
  });

  it('does not enable preview View for a Function output', () => {
    const functionPath = 'functions/Helper.yssbi-function';
    const fixture = makeEditorProjectionFixture({ graphPath: functionPath });
    expect(useGraphDataStore.getState().replaceProjection(
      functionPath,
      fixture.projection,
      1,
    ).applied).toBe(true);
    markResourceLoaded({ id: functionPath, kind: 'function' });
    useGraphSessionStore.getState().setFocusedSession('editor-a', functionPath);
    const pin = useGraphDataStore.getState().getGraphPin(functionPath, fixture.outputKey);
    if (!pin) throw new Error('expected projected function output pin');
    const execute = vi.spyOn(ProjectService, 'executeGraphDocument');

    act(() => root.render(
      <TooltipProvider>
        <Pin {...pin} graphPath={functionPath} />
      </TooltipProvider>,
    ));
    const pinElement = container.querySelector(`[data-pin-id="${fixture.outputKey}"]`);
    if (!pinElement) throw new Error('expected rendered pin');
    act(() => {
      pinElement.dispatchEvent(new MouseEvent('contextmenu', {
        bubbles: true,
        cancelable: true,
        clientX: 10,
        clientY: 20,
      }));
    });

    const viewItem = [...document.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')]
      .find((item) => item.textContent?.includes('contextMenu.pin.view'));
    expect(viewItem?.disabled).toBe(true);
    expect(execute).not.toHaveBeenCalled();
  });
});

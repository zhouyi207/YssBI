// @vitest-environment happy-dom

import { act } from 'react';
import { flushSync } from 'react-dom';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ExecuteEditorMutationOutcome } from '@/features/application/editorMutation/editorMutationCoordinator';
import type { ParameterEditorDto } from '@/shared/types/dto/editorProjection';
import { InlineParameterEditor } from './InlineParameterEditor';

const { setNodeParameters } = vi.hoisted(() => ({
  setNodeParameters: vi.fn(),
}));

vi.mock('@/features/application/editor/setNodeParameters', () => ({
  setNodeParameters,
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const graphPath = 'events/Main.yssbi-event';
const nodeId = 'constant-node';
const appliedOutcome: ExecuteEditorMutationOutcome = {
  status: 'applied',
  result: {} as never,
};
let container: HTMLDivElement;
let root: Root;

function projectedParameter(
  editor: ParameterEditorDto['editor'],
  value: unknown,
  valueType: ParameterEditorDto['valueType'] = null,
): ParameterEditorDto {
  return {
    key: 'value',
    display: { title: 'Value', description: 'Constant value' },
    editor,
    presentation: 'inlineAndDetail',
    valueType,
    multiline: false,
    value,
    configuration: null,
    inheritedValue: null,
    valueSource: null,
    options: null,
  };
}

function renderEditor(parameter: ParameterEditorDto, parentProps: React.HTMLAttributes<HTMLDivElement> = {}) {
  act(() => root.render(
    <div {...parentProps}>
      <InlineParameterEditor
        graphPath={graphPath}
        nodeId={nodeId}
        locale="en-US"
        parameter={parameter}
      />
    </div>,
  ));
}

function input(): HTMLInputElement {
  const element = container.querySelector('input');
  if (!(element instanceof HTMLInputElement)) throw new Error('missing input');
  return element;
}

function changeInput(value: string): void {
  const element = input();
  act(() => {
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
    setter?.call(element, value);
    element.dispatchEvent(new Event('input', { bubbles: true }));
    element.dispatchEvent(new Event('change', { bubbles: true }));
  });
}

function keyDown(key: string): void {
  act(() => input().dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true })));
}

function blurInput(): void {
  act(() => input().dispatchEvent(new FocusEvent('focusout', { bubbles: true })));
}

async function flushPromises(): Promise<void> {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

beforeEach(() => {
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
  setNodeParameters.mockReset();
  setNodeParameters.mockResolvedValue(appliedOutcome);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

describe('InlineParameterEditor', () => {
  it('submits a Boolean toggle immediately through setNodeParameters', async () => {
    renderEditor(projectedParameter('toggle', false, { kind: 'Boolean' }));

    const toggle = container.querySelector('[role="switch"]');
    if (!toggle) throw new Error('missing switch');
    act(() => toggle.dispatchEvent(new MouseEvent('click', { bubbles: true })));
    await flushPromises();

    expect(setNodeParameters).toHaveBeenCalledWith({
      graphPath,
      nodeId,
      locale: 'en-US',
      parameters: { value: true },
    });
  });

  it.each([
    ['number', 12, '34', 34, { kind: 'Int64' }],
    ['text', 'old', 'new', 'new', { kind: 'String' }],
  ] as const)('commits %s on Enter and not per keystroke', async (editor, value, draft, submitted, valueType) => {
    renderEditor(projectedParameter(editor, value, valueType));

    changeInput(draft);
    expect(setNodeParameters).not.toHaveBeenCalled();
    keyDown('Enter');
    await flushPromises();

    expect(setNodeParameters).toHaveBeenCalledOnce();
    expect(setNodeParameters).toHaveBeenCalledWith({
      graphPath,
      nodeId,
      locale: 'en-US',
      parameters: { value: submitted },
    });
  });

  it('commits a numeric draft on blur', async () => {
    renderEditor(projectedParameter('number', 12, { kind: 'Float64' }));

    changeInput('1.5');
    blurInput();
    await flushPromises();

    expect(setNodeParameters).toHaveBeenCalledWith({
      graphPath,
      nodeId,
      locale: 'en-US',
      parameters: { value: 1.5 },
    });
  });

  it('resets an invalid numeric blur and syncs a later projected value', () => {
    const initial = projectedParameter('number', 12, { kind: 'Int64' });
    renderEditor(initial);

    changeInput('1.5');
    blurInput();
    expect(input().value).toBe('12');
    expect(setNodeParameters).not.toHaveBeenCalled();

    renderEditor({ ...initial, value: 24 });
    expect(input().value).toBe('24');
  });

  it.each(['', 'not-a-number', '1.5', '9007199254740992', '-9007199254740992'])(
    'does not submit an empty, invalid, or unsafe Int64 draft: %s',
    async (draft) => {
    renderEditor(projectedParameter('number', 12, { kind: 'Int64' }));

    changeInput(draft);
    keyDown('Enter');
    await flushPromises();

      expect(setNodeParameters).not.toHaveBeenCalled();
    },
  );

  it.each(['9007199254740991', '-9007199254740991'])(
    'submits a safe Int64 boundary: %s',
    async (draft) => {
      renderEditor(projectedParameter('number', 12, { kind: 'Int64' }));

      changeInput(draft);
      keyDown('Enter');
      await flushPromises();

      expect(setNodeParameters).toHaveBeenCalledWith(expect.objectContaining({
        parameters: { value: Number(draft) },
      }));
    },
  );

  it('does not submit an invalid numeric draft on Enter', () => {
    renderEditor(projectedParameter('number', 12, { kind: 'Int64' }));

    changeInput('1.5');
    keyDown('Enter');

    expect(setNodeParameters).not.toHaveBeenCalled();
  });

  it('does not submit a non-finite Float64 draft', async () => {
    renderEditor(projectedParameter('number', 12, { kind: 'Float64' }));

    changeInput('Infinity');
    keyDown('Enter');
    await flushPromises();

    expect(setNodeParameters).not.toHaveBeenCalled();
  });

  it('synchronously blocks Enter from committing an active draft when a newer projection renders', async () => {
    const initial = projectedParameter('text', 'old', { kind: 'String' });
    renderEditor(initial);
    changeInput('draft');

    act(() => {
      flushSync(() => root.render(
        <InlineParameterEditor
          graphPath={graphPath}
          nodeId={nodeId}
          locale="en-US"
          parameter={{ ...initial, value: 'projected' }}
        />,
      ));
      input().dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    });
    expect(input().value).toBe('projected');
    await flushPromises();

    expect(setNodeParameters).not.toHaveBeenCalled();
  });

  it.each(['stale', 'conflict'] as const)(
    'restores the latest projection when mutation resolves %s',
    async (status) => {
      setNodeParameters.mockResolvedValueOnce({ status });
      const initial = projectedParameter('text', 'old', { kind: 'String' });
      renderEditor(initial);

      changeInput('new');
      keyDown('Enter');
      renderEditor({ ...initial, value: 'latest projection' });
      await flushPromises();

      expect(input().value).toBe('latest projection');
    },
  );

  it('restores the projected value when mutation rejects', async () => {
    setNodeParameters.mockRejectedValueOnce(new Error('backend rejected value'));
    renderEditor(projectedParameter('text', 'old', { kind: 'String' }));

    changeInput('new');
    keyDown('Enter');
    await flushPromises();

    expect(input().value).toBe('old');
  });

  it('does not mutate when the committed value equals the projection', async () => {
    renderEditor(projectedParameter('number', 12, { kind: 'Int64' }));

    changeInput('12');
    keyDown('Enter');
    await flushPromises();

    expect(setNodeParameters).not.toHaveBeenCalled();
  });

  it('isolates pointerdown from the node drag handler', () => {
    const onPointerDown = vi.fn();
    renderEditor(projectedParameter('text', 'old', { kind: 'String' }), { onPointerDown });

    act(() => input().dispatchEvent(new PointerEvent('pointerdown', { bubbles: true })));

    expect(onPointerDown).not.toHaveBeenCalled();
  });

  it.each(['Enter', 'Escape'])('isolates %s from the canvas key handler', async (key) => {
    const onKeyDown = vi.fn();
    renderEditor(projectedParameter('text', 'old', { kind: 'String' }), { onKeyDown });
    changeInput('draft');

    keyDown(key);
    await flushPromises();

    expect(onKeyDown).not.toHaveBeenCalled();
    if (key === 'Escape') expect(input().value).toBe('old');
  });
});

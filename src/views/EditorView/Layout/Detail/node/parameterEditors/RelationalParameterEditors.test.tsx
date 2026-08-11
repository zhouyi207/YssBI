// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { SchemaAwareParameterEditorDto } from '@/shared/types/dto/editorProjection';

const { translate } = vi.hoisted(() => ({
  translate: vi.fn((key: string, values?: Record<string, string>) => {
    const messages: Record<string, string> = {
      'detail.parameterEditor.apply': 'Apply',
      'detail.parameterEditor.column': 'Column',
      'detail.parameterEditor.operator': 'Operator',
      'detail.parameterEditor.valueType': 'Value type',
      'detail.parameterEditor.selectColumn': 'Select {{column}}',
      'detail.parameterEditor.moveColumnUp': 'Move {{column}} up',
      'detail.parameterEditor.moveColumnDown': 'Move {{column}} down',
      'detail.parameterEditor.predicateColumn': 'Predicate column',
      'detail.parameterEditor.predicateOperator': 'Predicate operator',
      'detail.parameterEditor.predicateValueType': 'Predicate value type',
      'detail.parameterEditor.predicateValue': 'Predicate value',
    };
    return (messages[key] ?? key).replace('{{column}}', values?.column ?? '');
  }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: translate }),
}));
import {
  FilterPredicateEditor,
  ProjectColumnsEditor,
} from './RelationalParameterEditors';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  translate.mockClear();
  container = document.createElement('div');
  document.body.append(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

function click(element: Element | null): void {
  if (!element) throw new Error('missing test element');
  act(() => element.dispatchEvent(new MouseEvent('click', { bubbles: true })));
}

function input(element: HTMLInputElement | null, value: string): void {
  if (!element) throw new Error('missing test input');
  act(() => {
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
    setter?.call(element, value);
    element.dispatchEvent(new Event('input', { bubbles: true }));
    element.dispatchEvent(new Event('change', { bubbles: true }));
  });
}

function chooseSelectOption(label: string, option: string): void {
  const trigger = container.querySelector(`[aria-label="${label}"]`);
  if (!trigger) throw new Error(`missing ${label} select`);
  act(() => trigger.dispatchEvent(new PointerEvent('pointerdown', {
    bubbles: true,
    button: 0,
    pointerType: 'mouse',
  })));
  const item = [...document.body.querySelectorAll('[role="option"]')]
    .find((candidate) => candidate.textContent === option);
  if (!item) throw new Error(`missing ${option} option`);
  act(() => {
    item.dispatchEvent(new PointerEvent('pointerdown', {
      bubbles: true,
      button: 0,
      pointerType: 'mouse',
    }));
    item.dispatchEvent(new PointerEvent('pointerup', {
      bubbles: true,
      button: 0,
      pointerType: 'mouse',
    }));
    item.dispatchEvent(new MouseEvent('click', { bubbles: true }));
  });
}

describe('ProjectColumnsEditor', () => {
  it('submits an ordered multi-selection in the user selection order', () => {
    const onCommit = vi.fn();
    const editor: Extract<SchemaAwareParameterEditorDto, { kind: 'projectColumns' }> = {
      kind: 'projectColumns',
      available: true,
      unavailableReason: null,
      options: [
        { name: 'a', dataType: 'string' },
        { name: 'b', dataType: 'int64' },
      ],
      value: [],
    };
    act(() => root.render(<ProjectColumnsEditor editor={editor} errors={[]} onCommit={onCommit} />));

    click(container.querySelector('[aria-label="Select b"]'));
    click(container.querySelector('[aria-label="Select a"]'));
    click(container.querySelector('button[type="submit"]'));

    expect(onCommit).toHaveBeenCalledOnce();
    expect(onCommit).toHaveBeenCalledWith(['b', 'a']);
    expect(translate).toHaveBeenCalledWith('detail.parameterEditor.selectColumn', { column: 'b' });
    expect(translate).toHaveBeenCalledWith('detail.parameterEditor.apply');
  });

  it('reorders an existing selection before committing', () => {
    const onCommit = vi.fn();
    const editor: Extract<SchemaAwareParameterEditorDto, { kind: 'projectColumns' }> = {
      kind: 'projectColumns',
      available: true,
      unavailableReason: null,
      options: [
        { name: 'a', dataType: 'string' },
        { name: 'b', dataType: 'int64' },
      ],
      value: ['a', 'b'],
    };
    act(() => root.render(<ProjectColumnsEditor editor={editor} errors={[]} onCommit={onCommit} />));

    click(container.querySelector('[aria-label="Move b up"]'));
    click(container.querySelector('button[type="submit"]'));

    expect(onCommit).toHaveBeenCalledWith(['b', 'a']);
  });

  it('renders the Rust-issued unavailable reason and validation errors', () => {
    const editor: Extract<SchemaAwareParameterEditorDto, { kind: 'projectColumns' }> = {
      kind: 'projectColumns',
      available: false,
      unavailableReason: 'Connect DataFrame input',
      options: [],
      value: [],
    };
    act(() => root.render(
      <ProjectColumnsEditor editor={editor} errors={['Choose at least one column']} onCommit={vi.fn()} />,
    ));

    expect(container.textContent).toContain('Connect DataFrame input');
    expect(container.textContent).toContain('Choose at least one column');
    expect(container.querySelector('button[type="submit"]')).toBeNull();
  });
});

describe('FilterPredicateEditor', () => {
  it.each([
    ['integer', '9007199254740993'],
    ['decimal', '9007199254740993.5'],
  ] as const)('preserves a large %s as a canonical string', (type, value) => {
    const onCommit = vi.fn();
    const editor: Extract<SchemaAwareParameterEditorDto, { kind: 'filterPredicate' }> = {
      kind: 'filterPredicate',
      available: true,
      unavailableReason: null,
      columns: [{
        name: 'amount',
        dataType: type === 'integer' ? 'int64' : 'float64',
        operators: ['equal', 'greaterThan', 'isNull'],
        literalTypes: [type],
      }],
      value: {
        column: 'amount',
        operator: 'greaterThan',
        value: { type, value: '1' },
      },
    };
    act(() => root.render(<FilterPredicateEditor editor={editor} errors={[]} onCommit={onCommit} />));

    input(container.querySelector('input[aria-label="Predicate value"]'), value);
    click(container.querySelector('button[type="submit"]'));

    expect(onCommit).toHaveBeenCalledWith({
      column: 'amount',
      operator: 'greaterThan',
      value: { type, value },
    });
  });

  it('uses only projected operators and omits the literal for a null check', () => {
    const onCommit = vi.fn();
    const editor: Extract<SchemaAwareParameterEditorDto, { kind: 'filterPredicate' }> = {
      kind: 'filterPredicate',
      available: true,
      unavailableReason: null,
      columns: [{
        name: 'created',
        dataType: 'dateTime',
        operators: ['isNull', 'isNotNull'],
        literalTypes: [],
      }],
      value: null,
    };
    act(() => root.render(<FilterPredicateEditor editor={editor} errors={[]} onCommit={onCommit} />));

    expect(container.textContent).not.toContain('greaterThan');
    expect(container.querySelector('input[aria-label="Predicate value"]')).toBeNull();
    click(container.querySelector('button[type="submit"]'));
    expect(onCommit).toHaveBeenCalledWith({ column: 'created', operator: 'isNull' });
  });

  it('does not fabricate or submit an operator for an Unknown column', () => {
    const onCommit = vi.fn();
    const editor: Extract<SchemaAwareParameterEditorDto, { kind: 'filterPredicate' }> = {
      kind: 'filterPredicate',
      available: true,
      unavailableReason: null,
      columns: [{
        name: 'opaque',
        dataType: 'unknown',
        operators: [],
        literalTypes: [],
      }],
      value: null,
    };
    act(() => root.render(<FilterPredicateEditor editor={editor} errors={[]} onCommit={onCommit} />));

    const operator = container.querySelector('[aria-label="Predicate operator"]') as HTMLButtonElement;
    const apply = container.querySelector('button[type="submit"]') as HTMLButtonElement;
    expect(operator.disabled).toBe(true);
    expect(operator.textContent).not.toContain('isNull');
    expect(apply.disabled).toBe(true);

    act(() => container.querySelector('form')?.dispatchEvent(new SubmitEvent('submit', {
      bubbles: true,
      cancelable: true,
    })));
    expect(onCommit).not.toHaveBeenCalled();
  });

  it('submits the displayed false default as a typed Boolean literal', () => {
    const onCommit = vi.fn();
    const editor: Extract<SchemaAwareParameterEditorDto, { kind: 'filterPredicate' }> = {
      kind: 'filterPredicate',
      available: true,
      unavailableReason: null,
      columns: [{
        name: 'active',
        dataType: 'boolean',
        operators: ['equal', 'notEqual'],
        literalTypes: ['boolean'],
      }],
      value: null,
    };
    act(() => root.render(<FilterPredicateEditor editor={editor} errors={[]} onCommit={onCommit} />));

    const value = container.querySelector('[aria-label="Predicate value"]');
    const apply = container.querySelector('button[type="submit"]') as HTMLButtonElement;
    expect(value?.textContent).toContain('false');
    expect(apply.disabled).toBe(false);
    click(apply);

    expect(onCommit).toHaveBeenCalledOnce();
    expect(onCommit).toHaveBeenCalledWith({
      column: 'active',
      operator: 'equal',
      value: { type: 'boolean', value: false },
    });
  });

  it('keeps operator and Boolean defaults consistent when switching to and from Unknown', () => {
    const onCommit = vi.fn();
    const editor: Extract<SchemaAwareParameterEditorDto, { kind: 'filterPredicate' }> = {
      kind: 'filterPredicate',
      available: true,
      unavailableReason: null,
      columns: [
        {
          name: 'active',
          dataType: 'boolean',
          operators: ['equal', 'notEqual'],
          literalTypes: ['boolean'],
        },
        {
          name: 'opaque',
          dataType: 'unknown',
          operators: [],
          literalTypes: [],
        },
      ],
      value: null,
    };
    act(() => root.render(<FilterPredicateEditor editor={editor} errors={[]} onCommit={onCommit} />));

    chooseSelectOption('Predicate column', 'opaque');
    const operator = container.querySelector('[aria-label="Predicate operator"]') as HTMLButtonElement;
    const apply = container.querySelector('button[type="submit"]') as HTMLButtonElement;
    expect(operator.disabled).toBe(true);
    expect(operator.textContent).not.toContain('isNull');
    expect(apply.disabled).toBe(true);
    act(() => container.querySelector('form')?.dispatchEvent(new SubmitEvent('submit', {
      bubbles: true,
      cancelable: true,
    })));
    expect(onCommit).not.toHaveBeenCalled();

    chooseSelectOption('Predicate column', 'active');
    expect(container.querySelector('[aria-label="Predicate operator"]')?.textContent).toContain('equal');
    expect(container.querySelector('[aria-label="Predicate value"]')?.textContent).toContain('false');
    expect((container.querySelector('button[type="submit"]') as HTMLButtonElement).disabled).toBe(false);
    click(container.querySelector('button[type="submit"]'));
    expect(onCommit).toHaveBeenCalledWith({
      column: 'active',
      operator: 'equal',
      value: { type: 'boolean', value: false },
    });
  });
});

import { Children, isValidElement, type ReactElement } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { PinEditor } from '../shared/PinEditor';
import { DetailNameField } from '../shared/DetailForm';
import { GraphLocalVariablesSection } from '../shared/GraphLocalVariablesSection';
import { FunctionDetailPanel } from './FunctionDetailPanel';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

function findAllByType(root: ReactElement, type: unknown): ReactElement[] {
  const matches: ReactElement[] = [];

  function visit(node: unknown) {
    if (!isValidElement(node)) return;
    if (node.type === type) {
      matches.push(node);
    }
    Children.forEach((node.props as { children?: unknown }).children, visit);
  }

  visit(root);
  return matches;
}

describe('FunctionDetailPanel', () => {
  it('routes rename and signature edits through separate callbacks', () => {
    const onRename = vi.fn();
    const onSignatureChange = vi.fn();
    const element = FunctionDetailPanel({
      fn: {
        path: 'function-1',
        name: 'Compute',
        inputs: [{ id: 'input-1', name: 'Value', type: 'int' }],
        outputs: [{ id: 'output-1', name: 'Result', type: 'float' }],
      },
      onRename,
      onSignatureChange,
    }) as ReactElement;

    const nameField = findAllByType(element, DetailNameField)[0];
    (nameField.props as { onCommit: (name: string) => void }).onCommit('Renamed');

    const pinEditors = findAllByType(element, PinEditor);
    (pinEditors[0].props as { onChange: (pins: unknown[]) => void }).onChange([
      { id: 'input-2', name: 'Next', type: 'string' },
    ]);
    (pinEditors[1].props as { onChange: (pins: unknown[]) => void }).onChange([
      { id: 'output-2', name: 'Done', type: 'bool' },
    ]);

    expect(onRename).toHaveBeenCalledWith('Renamed');
    expect(onSignatureChange).toHaveBeenNthCalledWith(1, {
      inputs: [{ id: 'input-2', name: 'Next', type: 'string' }],
    });
    expect(onSignatureChange).toHaveBeenNthCalledWith(2, {
      outputs: [{ id: 'output-2', name: 'Done', type: 'bool' }],
    });
  });

  it('renders local variables section with selection callback', () => {
    const onSelectLocalVariable = vi.fn();
    const element = FunctionDetailPanel({
      fn: {
        path: 'functions/A.yssbi-function',
        name: 'Compute',
        inputs: [],
        outputs: [],
      },
      localVariables: [{ id: 'var-1', name: 'Counter', typeLabel: 'Int64', dataType: { kind: 'Int64' } }],
      onSelectLocalVariable,
      onRename: vi.fn(),
      onSignatureChange: vi.fn(),
    }) as ReactElement;

    const section = findAllByType(element, GraphLocalVariablesSection)[0];
    (section.props as { onSelectVariable: (id: string) => void }).onSelectVariable('var-1');
    expect(onSelectLocalVariable).toHaveBeenCalledWith('var-1');
  });
});

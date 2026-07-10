import { Children, isValidElement, type ReactElement } from 'react';
import { createDataSignaturePin } from '@/shared/types/domain/functionSignaturePin';
import { describe, expect, it, vi } from 'vitest';
import { PinEditor } from '../shared/PinEditor';
import { DetailNameField } from '../shared/DetailForm';
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
        inputs: [createDataSignaturePin('input-1', 'Value', { kind: 'Int64' })],
        outputs: [createDataSignaturePin('output-1', 'Result', { kind: 'Float64' })],
      },
      onRename,
      onSignatureChange,
    }) as ReactElement;

    const nameField = findAllByType(element, DetailNameField)[0];
    (nameField.props as { onCommit: (name: string) => void }).onCommit('Renamed');

    const pinEditors = findAllByType(element, PinEditor);
    (pinEditors[0].props as { onChange: (pins: unknown[]) => void }).onChange([
      createDataSignaturePin('input-2', 'Next', { kind: 'String' }),
    ]);
    (pinEditors[1].props as { onChange: (pins: unknown[]) => void }).onChange([
      createDataSignaturePin('output-2', 'Done', { kind: 'Boolean' }),
    ]);

    expect(onRename).toHaveBeenCalledWith('Renamed');
    expect(onSignatureChange).toHaveBeenNthCalledWith(1, {
      inputs: [createDataSignaturePin('input-2', 'Next', { kind: 'String' })],
    });
    expect(onSignatureChange).toHaveBeenNthCalledWith(2, {
      outputs: [createDataSignaturePin('output-2', 'Done', { kind: 'Boolean' })],
    });
  });
});

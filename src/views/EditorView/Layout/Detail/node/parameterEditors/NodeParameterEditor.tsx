import { useState } from 'react';
import { setNodeParameters } from '@/features/application/editor/setNodeParameters';
import type {
  DiagnosticDto,
  FilterPredicateDto,
  ParameterEditorDto,
} from '@/shared/types/dto/editorProjection';
import { DetailReadonlyField } from '../../shared/DetailForm';
import {
  FilterPredicateEditor,
  ProjectColumnsEditor,
} from './RelationalParameterEditors';

interface NodeParameterEditorProps {
  graphPath: string;
  nodeId: string;
  locale: string;
  parameter: ParameterEditorDto;
  diagnostics: readonly DiagnosticDto[];
  formatFallback(value: unknown): string;
}

export function NodeParameterEditor({
  graphPath,
  nodeId,
  locale,
  parameter,
  diagnostics,
  formatFallback,
}: NodeParameterEditorProps) {
  const [pending, setPending] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);
  const errors = diagnostics
    .filter((diagnostic) => diagnostic.location.kind === 'parameter'
      && diagnostic.location.nodeId === nodeId
      && diagnostic.location.key === parameter.key)
    .map((diagnostic) => diagnostic.message);
  if (localError) errors.push(localError);

  const commit = async (value: string[] | FilterPredicateDto) => {
    setPending(true);
    setLocalError(null);
    try {
      await setNodeParameters({
        graphPath,
        nodeId,
        locale,
        parameters: { [parameter.key]: value },
      });
    } catch (error) {
      setLocalError(error instanceof Error ? error.message : String(error));
    } finally {
      setPending(false);
    }
  };

  const configuration = parameter.configuration;
  if (configuration?.kind === 'projectColumns') {
    return (
      <div className="space-y-2">
        <p className="text-xs font-medium text-foreground">{parameter.display.title}</p>
        <ProjectColumnsEditor
          editor={configuration}
          errors={errors}
          disabled={pending}
          onCommit={commit}
        />
      </div>
    );
  }
  if (configuration?.kind === 'filterPredicate') {
    return (
      <div className="space-y-2">
        <p className="text-xs font-medium text-foreground">{parameter.display.title}</p>
        <FilterPredicateEditor
          editor={configuration}
          errors={errors}
          disabled={pending}
          onCommit={commit}
        />
      </div>
    );
  }
  return (
    <DetailReadonlyField label={parameter.display.title}>
      {formatFallback(parameter.value)}
    </DetailReadonlyField>
  );
}

import { useRef, useState } from 'react';
import { Input } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';
import { setNodeParameters } from '@/features/application/editor/setNodeParameters';
import { uiStore } from '@/features/core/ui/UIStore';
import type {
  DiagnosticDto,
  ParameterEditorDto,
} from '@/shared/types/dto/editorProjection';
import { DetailReadonlyField } from '../../shared/DetailForm';
import { DetailFieldRow } from '../../shared/DetailFieldRow';
import { detailInlineInputClass } from '../../shared/detailStyles';
import { parseInlineNumber } from '../../../../Nodes/InlineParameterEditor';
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

function projectedDraft(parameter: ParameterEditorDto): string {
  return parameter.value === null || parameter.value === undefined
    ? ''
    : String(parameter.value);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function mutationOutcomeError(status: 'stale' | 'conflict'): string {
  return status === 'stale'
    ? 'The edit became stale; the latest value was restored'
    : 'The edit conflicted with a newer value; the latest value was restored';
}

interface CommitCallbacks {
  onResolved?(): void;
  onRejected?(): void;
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
  const pendingRef = useRef(false);
  const errors = diagnostics
    .filter((diagnostic) => diagnostic.location.kind === 'parameter'
      && diagnostic.location.nodeId === nodeId
      && diagnostic.location.key === parameter.key)
    .map((diagnostic) => diagnostic.message);
  if (localError) errors.push(localError);

  const commit = async (value: unknown, callbacks: CommitCallbacks = {}) => {
    if (pendingRef.current) return;
    if (Object.is(value, parameter.value)) {
      callbacks.onResolved?.();
      return;
    }
    pendingRef.current = true;
    setPending(true);
    setLocalError(null);
    try {
      const outcome = await setNodeParameters({
        graphPath,
        nodeId,
        locale,
        parameters: { [parameter.key]: value },
      });
      if (outcome.status !== 'applied') throw new Error(mutationOutcomeError(outcome.status));
      callbacks.onResolved?.();
    } catch (error) {
      const message = errorMessage(error);
      setLocalError(message);
      callbacks.onRejected?.();
      uiStore.showToast(message, 'error');
    } finally {
      pendingRef.current = false;
      setPending(false);
    }
  };

  const configuration = parameter.configuration;
  if (parameter.valueSource && parameter.inheritedValue !== undefined) {
    return (
      <ProjectedOverrideEditor
        parameter={parameter}
        pending={pending}
        onCommit={commit}
      />
    );
  }
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
  if (parameter.editor === 'toggle') {
    return (
      <DetailFieldRow label={parameter.display.title}>
        <Switch
          checked={parameter.value === true}
          disabled={pending}
          aria-label={parameter.display.title}
          onCheckedChange={(checked) => void commit(checked)}
        />
      </DetailFieldRow>
    );
  }
  if (parameter.editor === 'number' || parameter.editor === 'text') {
    return (
      <OrdinaryValueEditor
        parameter={parameter}
        pending={pending}
        onCommit={commit}
      />
    );
  }
  return (
    <DetailReadonlyField label={parameter.display.title}>
      {formatFallback(parameter.value)}
    </DetailReadonlyField>
  );
}

function ProjectedOverrideEditor({ parameter, pending, onCommit }: OrdinaryValueEditorProps) {
  const inherited = parameter.inheritedValue;
  const source = parameter.valueSource ?? (parameter.value == null ? 'project' : 'node');
  const effectiveValue = source === 'project' ? inherited : parameter.value;
  const setSource = (next: 'project' | 'node') => {
    onCommit(next === 'project' ? null : inherited);
  };

  return (
    <div className="space-y-2">
      <DetailFieldRow label={parameter.display.title}>
        <select
          aria-label="Setting source"
          className={detailInlineInputClass}
          value={source}
          disabled={pending}
          onChange={(event) => setSource(event.target.value as 'project' | 'node')}
        >
          <option value="project">Inherit project setting</option>
          <option value="node">Node override</option>
        </select>
      </DetailFieldRow>
      {source === 'project' ? (
        <DetailReadonlyField label="Effective value">
          {String(effectiveValue)}
        </DetailReadonlyField>
      ) : parameter.options?.length ? (
        <DetailFieldRow label="Effective value">
          <select
            aria-label={parameter.display.title}
            className={detailInlineInputClass}
            value={String(parameter.value ?? inherited)}
            disabled={pending}
            onChange={(event) => onCommit(event.target.value)}
          >
            {parameter.options.map((option) => <option key={option} value={option}>{option}</option>)}
          </select>
        </DetailFieldRow>
      ) : (
        <OrdinaryValueEditor parameter={parameter} pending={pending} onCommit={onCommit} />
      )}
    </div>
  );
}

interface OrdinaryValueEditorProps {
  parameter: ParameterEditorDto;
  pending: boolean;
  onCommit(value: unknown, callbacks?: CommitCallbacks): void;
}

function OrdinaryValueEditor({ parameter, pending, onCommit }: OrdinaryValueEditorProps) {
  const [draft, setDraft] = useState(() => projectedDraft(parameter));
  const [draftProjection, setDraftProjection] = useState(parameter.value);
  const projectedRef = useRef(parameter);
  projectedRef.current = parameter;
  if (!Object.is(draftProjection, parameter.value)) {
    setDraftProjection(parameter.value);
    setDraft(projectedDraft(parameter));
  }

  const reset = () => {
    setDraft(projectedDraft(projectedRef.current));
  };
  const submit = (value: unknown) => onCommit(value, { onRejected: reset });
  const commitDraft = (resetInvalid = false) => {
    if (parameter.editor === 'number') {
      const parsed = parseInlineNumber(draft, parameter.valueType);
      if (!parsed.ok) {
        uiStore.showToast(parsed.message, 'error');
        if (resetInvalid) reset();
        return;
      }
      submit(parsed.value);
      return;
    }
    submit(draft);
  };
  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement | HTMLTextAreaElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      reset();
    } else if (event.key === 'Enter' && !parameter.multiline) {
      event.preventDefault();
      commitDraft();
    }
  };
  const sharedProps = {
    value: draft,
    disabled: pending,
    'aria-label': parameter.display.title,
    onChange: (event: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => {
      setDraft(event.target.value);
    },
    onBlur: () => commitDraft(true),
    onKeyDown: handleKeyDown,
  };

  return (
    <DetailFieldRow label={parameter.display.title}>
      {parameter.editor === 'text' && parameter.multiline ? (
        <textarea
          {...sharedProps}
          className="min-h-20 w-full rounded-md border border-border bg-input/30 px-3 py-2 text-sm text-foreground outline-none focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30 disabled:cursor-not-allowed disabled:opacity-50"
        />
      ) : (
        <Input
          {...sharedProps}
          type="text"
          inputMode={parameter.editor === 'number' ? 'decimal' : undefined}
          className={detailInlineInputClass}
        />
      )}
    </DetailFieldRow>
  );
}

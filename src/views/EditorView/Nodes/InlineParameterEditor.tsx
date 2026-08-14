import { logger } from "@/utils/appLogger";
import { useRef, useState } from 'react';
import { Input } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';
import { setNodeParameters } from '@/features/application/editor/setNodeParameters';
import type { DataType } from '@/shared/types/domain/dataType';
import type { ParameterEditorDto } from '@/shared/types/dto/editorProjection';

interface InlineParameterEditorProps {
  graphPath: string;
  nodeId: string;
  locale: string;
  parameter: ParameterEditorDto;
}

export function parseInlineNumber(
  draft: string,
  valueType: DataType | null,
): { ok: true; value: number } | { ok: false; message: string } {
  const trimmed = draft.trim();
  if (trimmed.length === 0) return { ok: false, message: 'Enter a number' };

  const value = Number(trimmed);
  if (!Number.isFinite(value)) return { ok: false, message: 'Enter a finite number' };
  if (valueType?.kind === 'Int64') {
    if (!Number.isInteger(value)) return { ok: false, message: 'Enter an integer' };
    if (!Number.isSafeInteger(value)) {
      return { ok: false, message: 'Enter an integer within the supported range' };
    }
  }
  if (valueType?.kind !== 'Int64' && valueType?.kind !== 'Float64') {
    return { ok: false, message: 'Unsupported numeric value type' };
  }
  return { ok: true, value };
}

function projectedDraft(parameter: ParameterEditorDto): string {
  return parameter.value === null || parameter.value === undefined
    ? ''
    : String(parameter.value);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

type FailedMutationOutcome =
  | { status: 'stale' }
  | { status: 'conflict' }
  | { status: 'rejected'; code: string };

function mutationOutcomeError(outcome: FailedMutationOutcome): string {
  if (outcome.status === 'stale') {
    return 'The edit became stale; the latest value was restored';
  }
  if (outcome.status === 'conflict') {
    return 'The edit conflicted with a newer value; the latest value was restored';
  }
  return `The edit was rejected (${outcome.code}); the latest value was restored`;
}

export function InlineParameterEditor({
  graphPath,
  nodeId,
  locale,
  parameter,
}: InlineParameterEditorProps) {
  const [draft, setDraft] = useState(() => projectedDraft(parameter));
  const [draftProjection, setDraftProjection] = useState(parameter.value);
  const [pending, setPending] = useState(false);
  const projectedRef = useRef(parameter);
  const pendingRef = useRef(false);
  projectedRef.current = parameter;
  if (!Object.is(draftProjection, parameter.value)) {
    setDraftProjection(parameter.value);
    setDraft(projectedDraft(parameter));
  }

  const reset = () => {
    setDraft(projectedDraft(projectedRef.current));
  };

  const submit = async (value: unknown) => {
    if (pendingRef.current) return;
    if (Object.is(value, projectedRef.current.value)) return;

    pendingRef.current = true;
    setPending(true);
    try {
      const outcome = await setNodeParameters({
        graphPath,
        nodeId,
        locale,
        parameters: { [parameter.key]: value },
      });
      if (outcome.status !== 'applied' && outcome.status !== 'noop') {
        throw new Error(mutationOutcomeError(outcome));
      }
    } catch (error) {
      reset();
      logger.notify.error(errorMessage(error), "UI");
    } finally {
      pendingRef.current = false;
      setPending(false);
    }
  };

  const commitDraft = (resetInvalid = false) => {
    if (parameter.editor === 'number') {
      const parsed = parseInlineNumber(draft, parameter.valueType);
      if (!parsed.ok) {
        logger.notify.error(parsed.message, "UI");
        if (resetInvalid) reset();
        return;
      }
      void submit(parsed.value);
      return;
    }
    if (parameter.editor === 'text') void submit(draft);
  };

  const isolatePointer = (event: React.PointerEvent) => event.stopPropagation();
  const handleKeyDown = (event: React.KeyboardEvent) => {
    event.stopPropagation();
    if (event.key === 'Escape') {
      event.preventDefault();
      reset();
    } else if (event.key === 'Enter') {
      event.preventDefault();
      commitDraft();
    }
  };

  if (parameter.editor === 'toggle') {
    return (
      <label
        className="flex items-center justify-between gap-2 text-xs"
        onPointerDown={isolatePointer}
        onKeyDown={(event) => event.stopPropagation()}
      >
        <span className="truncate">{parameter.display.title}</span>
        <Switch
          size="sm"
          checked={parameter.value === true}
          disabled={pending}
          aria-label={parameter.display.title}
          onCheckedChange={(checked) => void submit(checked)}
        />
      </label>
    );
  }

  if (parameter.editor !== 'number' && parameter.editor !== 'text') return null;

  return (
    <label
      className="flex items-center gap-2 text-xs"
      onPointerDown={isolatePointer}
      onKeyDown={handleKeyDown}
    >
      <span className="min-w-0 flex-1 truncate">{parameter.display.title}</span>
      <Input
        type="text"
        inputMode={parameter.editor === 'number' ? 'decimal' : undefined}
        className="h-6 w-24 px-2 text-xs"
        value={draft}
        disabled={pending}
        aria-label={parameter.display.title}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={() => commitDraft(true)}
      />
    </label>
  );
}

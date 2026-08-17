import type { TFunction } from 'i18next';
import { useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';
import { setNodeParameters } from '@/features/application/editor/setNodeParameters';
import type { DataType } from '@/shared/types/domain/dataType';
import type { ParameterEditorDto } from '@/shared/types/dto/editorProjection';
import { formatInlineUserError } from '@/features/application/userErrorSummary';

interface InlineParameterEditorProps {
  graphPath: string;
  nodeId: string;
  locale: string;
  parameter: ParameterEditorDto;
}

export type InlineNumberError =
  | 'required'
  | 'notFinite'
  | 'notInteger'
  | 'outOfRange'
  | 'unsupportedType';

export function parseInlineNumber(
  draft: string,
  valueType: DataType | null,
): { ok: true; value: number } | { ok: false; error: InlineNumberError } {
  const trimmed = draft.trim();
  if (trimmed.length === 0) return { ok: false, error: 'required' };

  const value = Number(trimmed);
  if (!Number.isFinite(value)) return { ok: false, error: 'notFinite' };
  if (valueType?.kind === 'Int64') {
    if (!Number.isInteger(value)) return { ok: false, error: 'notInteger' };
    if (!Number.isSafeInteger(value)) return { ok: false, error: 'outOfRange' };
  }
  if (valueType?.kind !== 'Int64' && valueType?.kind !== 'Float64') {
    return { ok: false, error: 'unsupportedType' };
  }
  return { ok: true, value };
}

export function inlineNumberErrorMessage(error: InlineNumberError, t: TFunction): string {
  const keys = {
    required: 'notifications.parameter.enterNumber',
    notFinite: 'notifications.parameter.enterFiniteNumber',
    notInteger: 'notifications.parameter.enterInteger',
    outOfRange: 'notifications.parameter.enterSupportedInteger',
    unsupportedType: 'notifications.parameter.unsupportedNumericType',
  } as const;
  return t(keys[error]);
}

function projectedDraft(parameter: ParameterEditorDto): string {
  return parameter.value === null || parameter.value === undefined
    ? ''
    : String(parameter.value);
}


type FailedMutationOutcome =
  | { status: 'stale' }
  | { status: 'conflict' }
  | { status: 'rejected'; code: string };

function mutationOutcomeError(outcome: FailedMutationOutcome, t: TFunction): string {
  if (outcome.status === 'stale') return t('notifications.parameter.stale');
  if (outcome.status === 'conflict') return t('notifications.parameter.conflict');
  return t('notifications.parameter.rejected', { code: outcome.code });
}

export function InlineParameterEditor({
  graphPath,
  nodeId,
  locale,
  parameter,
}: InlineParameterEditorProps) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState(() => projectedDraft(parameter));
  const [draftProjection, setDraftProjection] = useState(parameter.value);
  const [pending, setPending] = useState(false);
  const [fieldError, setFieldError] = useState<string | null>(null);
  const fieldErrorId = useId();
  const projectedRef = useRef(parameter);
  const pendingRef = useRef(false);
  projectedRef.current = parameter;
  if (!Object.is(draftProjection, parameter.value)) {
    setDraftProjection(parameter.value);
    setDraft(projectedDraft(parameter));
    setFieldError(null);
  }

  const reset = () => {
    setDraft(projectedDraft(projectedRef.current));
  };

  const submit = async (value: unknown) => {
    if (pendingRef.current) return;
    if (Object.is(value, projectedRef.current.value)) return;

    pendingRef.current = true;
    setPending(true);
    setFieldError(null);
    try {
      const outcome = await setNodeParameters({
        graphPath,
        nodeId,
        locale,
        parameters: { [parameter.key]: value },
      });
      if (outcome.status !== 'applied' && outcome.status !== 'noop') {
        reset();
        setFieldError(t('notifications.parameter.updateFailed', {
          error: mutationOutcomeError(outcome, t),
        }));
      }
    } catch (error) {
      reset();
      setFieldError(t('notifications.parameter.updateFailed', {
        error: formatInlineUserError(error, t),
      }));
    } finally {
      pendingRef.current = false;
      setPending(false);
    }
  };

  const commitDraft = (resetInvalid = false) => {
    if (parameter.editor === 'number') {
      const parsed = parseInlineNumber(draft, parameter.valueType);
      if (!parsed.ok) {
        setFieldError(inlineNumberErrorMessage(parsed.error, t));
        if (resetInvalid) reset();
        return;
      }
      setFieldError(null);
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
      setFieldError(null);
    } else if (event.key === 'Enter') {
      event.preventDefault();
      commitDraft();
    }
  };

  if (parameter.editor === 'toggle') {
    return (
      <div
        className="space-y-1"
        onPointerDown={isolatePointer}
        onKeyDown={(event) => event.stopPropagation()}
      >
        <label className="flex items-center justify-between gap-2 text-xs">
          <span className="truncate">{parameter.display.title}</span>
          <Switch
            size="sm"
            checked={parameter.value === true}
            disabled={pending}
            aria-label={parameter.display.title}
            aria-invalid={Boolean(fieldError)}
            aria-describedby={fieldError ? fieldErrorId : undefined}
            onCheckedChange={(checked) => void submit(checked)}
          />
        </label>
        <InlineFieldError id={fieldErrorId} message={fieldError} />
      </div>
    );
  }

  if (parameter.editor !== 'number' && parameter.editor !== 'text') return null;

  return (
    <div className="space-y-1" onPointerDown={isolatePointer} onKeyDown={handleKeyDown}>
      <label className="flex items-center gap-2 text-xs">
        <span className="min-w-0 flex-1 truncate">{parameter.display.title}</span>
        <Input
          type="text"
          inputMode={parameter.editor === 'number' ? 'decimal' : undefined}
          className="h-6 w-24 px-2 text-xs"
          value={draft}
          disabled={pending}
          aria-label={parameter.display.title}
          aria-invalid={Boolean(fieldError)}
          aria-describedby={fieldError ? fieldErrorId : undefined}
          onChange={(event) => {
            setDraft(event.target.value);
            setFieldError(null);
          }}
          onBlur={() => commitDraft(true)}
        />
      </label>
      <InlineFieldError id={fieldErrorId} message={fieldError} />
    </div>
  );
}

function InlineFieldError({ id, message }: { id: string; message: string | null }) {
  if (!message) return null;
  return <p id={id} role="alert" className="text-[10px] leading-tight text-destructive">{message}</p>;
}

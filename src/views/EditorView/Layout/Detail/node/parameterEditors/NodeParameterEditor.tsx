import type { TFunction } from "i18next";
import { useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { setNodeParameters } from "@/features/application/editor/setNodeParameters";
import type { DataType } from "@/shared/types/domain/dataType";
import type { DiagnosticDto, ParameterEditorDto } from "@/shared/types/domain/editorProjection";
import { formatInlineUserError } from "@/features/application/userErrorSummary";
import { DetailReadonlyField } from "../../shared/DetailForm";
import { DetailFieldRow } from "../../shared/DetailFieldRow";
import { detailInlineInputClass } from "../../shared/detailStyles";
import { FilterPredicateEditor, ProjectColumnsEditor } from "./RelationalParameterEditors";

interface NodeParameterEditorProps {
  graphPath: string;
  nodeId: string;
  locale: string;
  parameter: ParameterEditorDto;
  diagnostics: readonly DiagnosticDto[];
  formatFallback(value: unknown): string;
}

function projectedDraft(parameter: ParameterEditorDto): string {
  return parameter.value === null || parameter.value === undefined ? "" : String(parameter.value);
}

type NumberDraftError = "required" | "notFinite" | "notInteger" | "outOfRange" | "unsupportedType";

function parseNumberDraft(
  draft: string,
  valueType: DataType | null,
): { ok: true; value: number } | { ok: false; error: NumberDraftError } {
  const trimmed = draft.trim();
  if (trimmed.length === 0) return { ok: false, error: "required" };
  const value = Number(trimmed);
  if (!Number.isFinite(value)) return { ok: false, error: "notFinite" };
  if (valueType?.kind === "Int64") {
    if (!Number.isInteger(value)) return { ok: false, error: "notInteger" };
    if (!Number.isSafeInteger(value)) return { ok: false, error: "outOfRange" };
  }
  if (valueType?.kind !== "Int64" && valueType?.kind !== "Float64") {
    return { ok: false, error: "unsupportedType" };
  }
  return { ok: true, value };
}

function numberDraftErrorMessage(error: NumberDraftError, t: TFunction): string {
  const keys = {
    required: "notifications.parameter.enterNumber",
    notFinite: "notifications.parameter.enterFiniteNumber",
    notInteger: "notifications.parameter.enterInteger",
    outOfRange: "notifications.parameter.enterSupportedInteger",
    unsupportedType: "notifications.parameter.unsupportedNumericType",
  } as const;
  return t(keys[error]);
}

type FailedMutationOutcome =
  | { status: "stale" }
  | { status: "conflict" }
  | { status: "rejected"; code: string };

function mutationOutcomeError(outcome: FailedMutationOutcome, t: TFunction): string {
  if (outcome.status === "stale") return t("notifications.parameter.stale");
  if (outcome.status === "conflict") return t("notifications.parameter.conflict");
  return t("notifications.parameter.rejected", { code: outcome.code });
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
  const { t } = useTranslation();
  const [pending, setPending] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);
  const fieldErrorId = useId();
  const pendingRef = useRef(false);
  const errors = diagnostics
    .filter(
      (diagnostic) =>
        diagnostic.location.kind === "parameter" &&
        diagnostic.location.nodeId === nodeId &&
        diagnostic.location.key === parameter.key,
    )
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
      if (outcome.status !== "applied" && outcome.status !== "noop") {
        setLocalError(
          t("notifications.parameter.updateFailed", {
            error: mutationOutcomeError(outcome, t),
          }),
        );
        callbacks.onRejected?.();
        return;
      }
      callbacks.onResolved?.();
    } catch (error) {
      setLocalError(
        t("notifications.parameter.updateFailed", {
          error: formatInlineUserError(error, t),
        }),
      );
      callbacks.onRejected?.();
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
        errors={errors}
        onCommit={commit}
      />
    );
  }
  if (configuration?.kind === "projectColumns") {
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
  if (configuration?.kind === "filterPredicate") {
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
  if (parameter.editor === "toggle") {
    return (
      <DetailFieldRow label={parameter.display.title}>
        <div className="space-y-1">
          <Switch
            checked={parameter.value === true}
            disabled={pending}
            aria-label={parameter.display.title}
            aria-invalid={errors.length > 0}
            aria-describedby={errors.length > 0 ? fieldErrorId : undefined}
            onCheckedChange={(checked) => void commit(checked)}
          />
          <ParameterErrorList id={fieldErrorId} errors={errors} />
        </div>
      </DetailFieldRow>
    );
  }
  if (parameter.editor === "number" || parameter.editor === "text") {
    return (
      <OrdinaryValueEditor
        parameter={parameter}
        pending={pending}
        errors={errors}
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

function ProjectedOverrideEditor({
  parameter,
  pending,
  errors,
  onCommit,
}: OrdinaryValueEditorProps) {
  const inherited = parameter.inheritedValue;
  const errorId = useId();
  const source = parameter.valueSource ?? (parameter.value == null ? "project" : "node");
  const effectiveValue = source === "project" ? inherited : parameter.value;
  const setSource = (next: "project" | "node") => {
    onCommit(next === "project" ? null : inherited);
  };

  return (
    <div className="space-y-2">
      <DetailFieldRow label={parameter.display.title}>
        <select
          aria-label="Setting source"
          className={detailInlineInputClass}
          value={source}
          disabled={pending}
          aria-invalid={errors.length > 0}
          aria-describedby={errors.length > 0 ? errorId : undefined}
          onChange={(event) => setSource(event.target.value as "project" | "node")}
        >
          <option value="project">Inherit project setting</option>
          <option value="node">Node override</option>
        </select>
      </DetailFieldRow>
      {source === "project" ? (
        <DetailReadonlyField label="Effective value">{String(effectiveValue)}</DetailReadonlyField>
      ) : parameter.options?.length ? (
        <DetailFieldRow label="Effective value">
          <select
            aria-label={parameter.display.title}
            className={detailInlineInputClass}
            value={String(parameter.value ?? inherited)}
            disabled={pending}
            aria-invalid={errors.length > 0}
            aria-describedby={errors.length > 0 ? errorId : undefined}
            onChange={(event) => onCommit(event.target.value)}
          >
            {parameter.options.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        </DetailFieldRow>
      ) : (
        <OrdinaryValueEditor
          parameter={parameter}
          pending={pending}
          errors={errors}
          errorId={errorId}
          onCommit={onCommit}
        />
      )}
      {source === "project" || parameter.options?.length ? (
        <ParameterErrorList id={errorId} errors={errors} />
      ) : null}
    </div>
  );
}

interface OrdinaryValueEditorProps {
  parameter: ParameterEditorDto;
  pending: boolean;
  errors: readonly string[];
  errorId?: string;
  onCommit(value: unknown, callbacks?: CommitCallbacks): void;
}

function OrdinaryValueEditor({
  parameter,
  pending,
  errors,
  errorId: providedErrorId,
  onCommit,
}: OrdinaryValueEditorProps) {
  const { t } = useTranslation();
  const generatedErrorId = useId();
  const errorId = providedErrorId ?? generatedErrorId;
  const [draft, setDraft] = useState(() => projectedDraft(parameter));
  const [draftProjection, setDraftProjection] = useState(parameter.value);
  const [parseError, setParseError] = useState<string | null>(null);
  const projectedRef = useRef(parameter);
  projectedRef.current = parameter;
  if (!Object.is(draftProjection, parameter.value)) {
    setDraftProjection(parameter.value);
    setDraft(projectedDraft(parameter));
    setParseError(null);
  }

  const reset = () => {
    setDraft(projectedDraft(projectedRef.current));
  };
  const submit = (value: unknown) => onCommit(value, { onRejected: reset });
  const commitDraft = (resetInvalid = false) => {
    if (parameter.editor === "number") {
      const parsed = parseNumberDraft(draft, parameter.valueType);
      if (!parsed.ok) {
        setParseError(numberDraftErrorMessage(parsed.error, t));
        if (resetInvalid) reset();
        return;
      }
      setParseError(null);
      submit(parsed.value);
      return;
    }
    submit(draft);
  };
  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement | HTMLTextAreaElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      reset();
      setParseError(null);
    } else if (event.key === "Enter" && !parameter.multiline) {
      event.preventDefault();
      commitDraft();
    }
  };
  const visibleErrors = parseError ? [...errors, parseError] : errors;
  const sharedProps = {
    value: draft,
    disabled: pending,
    "aria-label": parameter.display.title,
    "aria-invalid": visibleErrors.length > 0,
    "aria-describedby": visibleErrors.length > 0 ? errorId : undefined,
    onChange: (event: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => {
      setDraft(event.target.value);
      setParseError(null);
    },
    onBlur: () => commitDraft(true),
    onKeyDown: handleKeyDown,
  };

  return (
    <DetailFieldRow label={parameter.display.title}>
      <div className="space-y-1">
        {parameter.editor === "text" && parameter.multiline ? (
          <textarea
            {...sharedProps}
            className="min-h-20 w-full rounded-md border border-border bg-input/30 px-3 py-2 text-left text-sm text-foreground outline-none focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30 disabled:cursor-not-allowed disabled:opacity-50"
          />
        ) : (
          <Input
            {...sharedProps}
            type="text"
            inputMode={parameter.editor === "number" ? "decimal" : undefined}
            className={detailInlineInputClass}
          />
        )}
        <ParameterErrorList id={errorId} errors={visibleErrors} />
      </div>
    </DetailFieldRow>
  );
}

function ParameterErrorList({ id, errors }: { id: string; errors: readonly string[] }) {
  if (errors.length === 0) return null;
  return (
    <div id={id} role="alert" className="space-y-1 text-xs text-destructive">
      {errors.map((error, index) => (
        <p key={`${index}-${error}`}>{error}</p>
      ))}
    </div>
  );
}

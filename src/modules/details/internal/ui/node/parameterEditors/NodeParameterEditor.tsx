import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { TFunction } from "i18next";
import { useGraphConstants } from "@/features/application/graphEditing/graphConstantActions";
import { formatGraphDiagnostic } from "@/features/domain/graphDiagnostics/nodeDiagnostics";
import { useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { setNodeParameters } from "@/features/application/editor/setNodeParameters";
import type { ValueType } from "@/shared/types/domain/valueType";
import type { DiagnosticDto, ParameterEditorDto } from "@/shared/types/domain/editorProjection";
import { formatInlineUserError } from "@/features/application/userErrorSummary";
import { DetailTextarea } from "../../shared/DetailForm";
import { DetailText } from "../../shared/DetailText";
import { detailInlineInputClass } from "../../shared/detailStyles";
import { FilterPredicateEditor, ProjectColumnsEditor } from "./RelationalParameterEditors";
import { SemanticDomainEditor } from "./SemanticDomainEditor";

interface NodeParameterEditorProps {
  graphPath: string;
  nodeId: string;
  locale: string;
  parameter: DeepReadonly<ParameterEditorDto>;
  diagnostics: DeepReadonly<DiagnosticDto[]>;
  formatFallback(value: unknown): string;
}

function projectedDraft(parameter: DeepReadonly<ParameterEditorDto>): string {
  return parameter.value === null || parameter.value === undefined ? "" : String(parameter.value);
}

function optionLabel(key: string, option: string, t: TFunction): string {
  if (key === "theil_form" && ["individual", "grouped"].includes(option))
    return t(`theil.${option}`);
  if (key === "column_match" && ["by_name", "by_position"].includes(option))
    return t(`tableComposition.${option}`);
  if (key === "join_type" && ["inner", "left", "right", "full", "semi", "anti"].includes(option))
    return t(`tableComposition.${option}`);
  if (key === "target_type") {
    const labels: Record<string, string> = {
      "core.categorical": "conversion.categorical",
      "core.ordinal": "conversion.ordinal",
    };
    if (labels[option]) return t(labels[option]);
  }
  if (key === "numeric_mode" && ["auto", "integer", "real"].includes(option)) {
    return t(`conversion.${option}`);
  }
  if (key === "datetime_kind" && ["auto", "date", "time", "datetime"].includes(option))
    return t(`conversion.${option}`);
  if (
    key === "datetime_precision" &&
    ["seconds", "milliseconds", "microseconds", "nanoseconds"].includes(option)
  )
    return t(`conversion.${option}`);
  return option;
}

type NumberDraftError = "required" | "notFinite" | "outOfRange" | "unsupportedType";

function parseNumberDraft(
  draft: string,
  valueType: DeepReadonly<ValueType> | null,
): { ok: true; value: number } | { ok: false; error: NumberDraftError } {
  const trimmed = draft.trim();
  if (trimmed.length === 0) return { ok: false, error: "required" };
  const value = Number(trimmed);
  if (!Number.isFinite(value)) return { ok: false, error: "notFinite" };
  if (Number.isInteger(value) && !Number.isSafeInteger(value))
    return { ok: false, error: "outOfRange" };
  if (valueType?.kind !== "Scalar" || valueType.inner !== "Numeric") {
    return { ok: false, error: "unsupportedType" };
  }
  return { ok: true, value };
}

function numberDraftErrorMessage(error: NumberDraftError, t: TFunction): string {
  const keys = {
    required: "notifications.parameter.enterNumber",
    notFinite: "notifications.parameter.enterFiniteNumber",
    outOfRange: "notifications.parameter.enterSupportedInteger",
    unsupportedType: "notifications.parameter.unsupportedNumericType",
  } as const;
  return t(keys[error]);
}

type FailedMutationOutcome =
  | { status: "stale" }
  | { status: "saving" }
  | { status: "rejected"; code: string };

function mutationOutcomeError(outcome: FailedMutationOutcome, t: TFunction): string {
  if (outcome.status === "stale") return t("notifications.parameter.stale");
  if (outcome.status === "saving") return t("notifications.parameter.stale");
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
  const pendingRef = useRef(false);
  const errors = diagnostics.map((diagnostic) => formatGraphDiagnostic(diagnostic, locale));
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

  if (parameter.editor.kind === "graphConstant") {
    return (
      <GraphConstantValueEditor
        graphPath={graphPath}
        parameter={parameter}
        pending={pending}
        errors={errors}
        onCommit={commit}
      />
    );
  }
  return (
    <ParameterValueEditor
      parameter={parameter}
      pending={pending}
      errors={errors}
      onCommit={commit}
      formatFallback={formatFallback}
    />
  );
}

function GraphConstantValueEditor({
  graphPath,
  parameter,
  pending,
  errors,
  onCommit,
}: OrdinaryValueEditorProps & { graphPath: string }) {
  const { t } = useTranslation();
  const { constants } = useGraphConstants(graphPath);
  const errorId = useId();
  return (
    <div className="space-y-1">
      <select
        aria-label={parameter.display.title}
        className={detailInlineInputClass}
        value={String(parameter.value ?? "")}
        disabled={pending}
        aria-invalid={errors.length > 0}
        aria-describedby={errors.length > 0 ? errorId : undefined}
        onChange={(event) => onCommit(event.target.value)}
      >
        {!constants[String(parameter.value ?? "")] && (
          <option value={String(parameter.value ?? "")} disabled>
            {t("detail.constants.choose")}
          </option>
        )}
        {Object.values(constants).map((constant) => (
          <option key={constant.id} value={constant.id}>
            {constant.name}
          </option>
        ))}
      </select>
      <ParameterErrorList id={errorId} errors={errors} />
    </div>
  );
}

function ParameterValueEditor({
  parameter,
  pending,
  errors,
  onCommit: commit,
  formatFallback,
}: OrdinaryValueEditorProps & { formatFallback(value: unknown): string }) {
  const { t } = useTranslation();
  const fieldErrorId = useId();
  const editor = parameter.editor;
  if (parameter.editor.kind === "semanticDomain") {
    return (
      <div className="space-y-1">
        <SemanticDomainEditor value={parameter.value} pending={pending} onCommit={commit} />
        <ParameterErrorList id={fieldErrorId} errors={errors} />
      </div>
    );
  }
  if (editor.kind === "projectColumns") {
    return (
      <ProjectColumnsEditor editor={editor} errors={errors} disabled={pending} onCommit={commit} />
    );
  }
  if (editor.kind === "filterPredicate") {
    return (
      <FilterPredicateEditor editor={editor} errors={errors} disabled={pending} onCommit={commit} />
    );
  }
  if (editor.kind === "select" && editor.options !== null) {
    return (
      <div className="space-y-1">
        <select
          aria-label={parameter.display.title}
          className={detailInlineInputClass}
          value={String(parameter.value ?? "")}
          disabled={pending}
          aria-invalid={errors.length > 0}
          aria-describedby={errors.length > 0 ? fieldErrorId : undefined}
          onChange={(event) => commit(event.target.value)}
        >
          {!editor.options.includes(String(parameter.value ?? "")) && (
            <option value={String(parameter.value ?? "")} disabled>
              {parameter.value == null ? "—" : String(parameter.value)}
            </option>
          )}
          {editor.options.map((option) => (
            <option key={option} value={option}>
              {optionLabel(parameter.key, option, t)}
            </option>
          ))}
        </select>
        <ParameterErrorList id={fieldErrorId} errors={errors} />
      </div>
    );
  }
  if (parameter.editor.kind === "toggle") {
    return (
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
    );
  }
  if (
    parameter.valueType?.kind === "DataSeries" &&
    parameter.valueType.inner.kind === "Scalar" &&
    ["Text", "Numeric"].includes(parameter.valueType.inner.inner)
  ) {
    return (
      <ListValueEditor parameter={parameter} pending={pending} errors={errors} onCommit={commit} />
    );
  }
  if (
    parameter.editor.kind === "number" ||
    parameter.editor.kind === "text" ||
    (parameter.editor.kind === "select" &&
      parameter.valueType?.kind === "Scalar" &&
      parameter.valueType.inner === "Text")
  ) {
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
    <DetailText as="div" tone="muted" className="break-words">
      {formatFallback(parameter.value)}
    </DetailText>
  );
}

interface OrdinaryValueEditorProps {
  parameter: DeepReadonly<ParameterEditorDto>;
  pending: boolean;
  errors: readonly string[];
  onCommit(value: unknown, callbacks?: CommitCallbacks): void;
}

const EMPTY_LIST_VALUES: readonly unknown[] = [];

function ListValueEditor({ parameter, pending, errors, onCommit }: OrdinaryValueEditorProps) {
  const { t } = useTranslation();
  const errorId = useId();
  const values: readonly unknown[] = Array.isArray(parameter.value)
    ? parameter.value
    : EMPTY_LIST_VALUES;
  const latest = useRef(values);
  latest.current = values;
  const [projection, setProjection] = useState(values);
  const [draft, setDraft] = useState<string[]>(() => values.map(String));
  const [parseError, setParseError] = useState<string | null>(null);
  const [page, setPage] = useState(0);
  if (projection !== values) {
    setProjection(values);
    setDraft(values.map(String));
    setParseError(null);
  }
  const numeric =
    parameter.valueType?.kind === "DataSeries" &&
    parameter.valueType.inner.kind === "Scalar" &&
    parameter.valueType.inner.inner === "Numeric";
  const reset = () => {
    setDraft(latest.current.map(String));
    setParseError(null);
  };
  const submit = (next: string[]) => {
    if (pending) return;
    const parsed: (number | string)[] = [];
    for (const value of next) {
      if (numeric) {
        const result = parseNumberDraft(value, { kind: "Scalar", inner: "Numeric" });
        if (!result.ok) {
          setParseError(numberDraftErrorMessage(result.error, t));
          return;
        }
        parsed.push(result.value);
      } else parsed.push(value);
    }
    setParseError(null);
    if (parsed.length !== values.length || parsed.some((value, index) => value !== values[index]))
      onCommit(parsed, { onRejected: reset });
  };
  const change = (next: string[]) => {
    setDraft(next);
    submit(next);
  };
  const lastPage = Math.max(0, Math.ceil(draft.length / 50) - 1);
  const shownPage = Math.min(page, lastPage);
  const visibleErrors = parseError ? [...errors, parseError] : errors;
  return (
    <div
      className="space-y-2"
      onBlur={(event) => {
        if (
          event.relatedTarget instanceof Node &&
          event.currentTarget.contains(event.relatedTarget)
        )
          return;
        submit(draft);
      }}
    >
      <div className="max-h-64 space-y-1 overflow-y-auto">
        {draft.slice(shownPage * 50, shownPage * 50 + 50).map((value, row) => {
          const index = shownPage * 50 + row;
          return (
            <div key={index} className="flex items-center gap-1">
              <Input
                className={detailInlineInputClass}
                value={value}
                disabled={pending}
                aria-label={`${parameter.display.title} ${index + 1}`}
                aria-invalid={visibleErrors.length > 0}
                aria-describedby={visibleErrors.length ? errorId : undefined}
                inputMode={numeric ? "decimal" : undefined}
                onChange={(event) => {
                  setDraft(
                    draft.map((entry, item) => (item === index ? event.target.value : entry)),
                  );
                  setParseError(null);
                }}
                onKeyDown={(event) => {
                  if (event.key === "Escape") {
                    event.preventDefault();
                    reset();
                  }
                  if (event.key === "Enter") {
                    event.preventDefault();
                    submit(draft);
                  }
                }}
              />
              <Button
                type="button"
                variant="ghost"
                size="sm"
                disabled={pending}
                aria-label={`${t("conversion.removeValue")} ${index + 1}`}
                onClick={() => change(draft.filter((_, item) => item !== index))}
              >
                {t("conversion.removeValue")}
              </Button>
            </div>
          );
        })}
      </div>
      {lastPage > 0 && (
        <div className="flex items-center gap-2 text-xs">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={shownPage === 0}
            onClick={() => setPage(shownPage - 1)}
          >
            {t("conversion.previousValues")}
          </Button>
          <span>
            {shownPage + 1}/{lastPage + 1}
          </span>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={shownPage === lastPage}
            onClick={() => setPage(shownPage + 1)}
          >
            {t("conversion.nextValues")}
          </Button>
        </div>
      )}
      <Button
        type="button"
        variant="outline"
        size="sm"
        disabled={pending}
        onClick={() => {
          const next = [...draft, ""];
          setDraft(next);
          setPage(Math.floor((next.length - 1) / 50));
          if (!numeric) submit(next);
        }}
      >
        {t("conversion.addValue")}
      </Button>
      <ParameterErrorList id={errorId} errors={visibleErrors} />
    </div>
  );
}

function OrdinaryValueEditor({ parameter, pending, errors, onCommit }: OrdinaryValueEditorProps) {
  const { t } = useTranslation();
  const errorId = useId();
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
    if (parameter.editor.kind === "number") {
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
    <div className="space-y-1">
      {parameter.editor.kind === "text" && parameter.multiline ? (
        <DetailTextarea {...sharedProps} />
      ) : (
        <Input
          {...sharedProps}
          type="text"
          inputMode={parameter.editor.kind === "number" ? "decimal" : undefined}
          className={detailInlineInputClass}
        />
      )}
      <ParameterErrorList id={errorId} errors={visibleErrors} />
    </div>
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

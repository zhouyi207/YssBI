import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useNodeCreationForm } from "@/features/application/nodeCatalog/useNodeCreationForm";
import { formatInlineUserError } from "@/features/application/userErrorSummary";
import { NodeParameterField } from "./parameterEditors/NodeParameterEditor";

export function NodeCreationForm({
  projectInstanceId,
  nodeTypeId,
  title,
  graphPath,
  locale,
  onCreate,
  onBack,
}: {
  projectInstanceId: string;
  nodeTypeId: string;
  title: string;
  graphPath: string;
  locale: string;
  onCreate(
    parameters: Record<string, unknown>,
    portCounts: Record<string, number>,
  ): Promise<boolean>;
  onBack(): void;
}) {
  const { t } = useTranslation();
  const { form, loadError, errors, pendingKeys, commitParameter, commitPortCount, preparedValues } =
    useNodeCreationForm(projectInstanceId, nodeTypeId, locale);
  const [creating, setCreating] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);
  const submitting = useRef(false);

  const create = async () => {
    if (submitting.current) return;
    submitting.current = true;
    setCreating(true);
    setCreateError(null);
    try {
      const values = await preparedValues();
      if (!values) return;
      if (!(await onCreate(values.values, values.portCounts)))
        setCreateError(t("canvas.nodePalette.createFailed"));
    } catch (error) {
      setCreateError(formatInlineUserError(error, t));
    } finally {
      submitting.current = false;
      setCreating(false);
    }
  };

  return (
    <form
      className="flex min-h-0 flex-col gap-3 p-2"
      onSubmit={(event) => {
        event.preventDefault();
        const invalid = event.currentTarget.querySelector<HTMLElement>('[aria-invalid="true"]');
        if (invalid) {
          invalid.focus();
          return;
        }
        void create();
      }}
    >
      <div className="text-sm font-medium">{title}</div>
      <div className="min-h-0 space-y-4 overflow-y-auto">
        {loadError !== null && (
          <p role="alert" className="text-xs text-destructive">
            {formatInlineUserError(loadError, t)}
          </p>
        )}
        {!form && loadError === null && <p role="status">{t("common.loading")}</p>}
        {form && form.ports.length > 0 && (
          <fieldset className="space-y-2">
            <legend className="mb-2 text-xs font-medium">
              {t("canvas.nodePalette.pinCounts")}
            </legend>
            {form.ports.map((port) => {
              const policy = port.count;
              if (policy.kind === "configurable" && policy.memberTemplates[0] !== port.key)
                return null;
              const title =
                policy.kind === "configurable"
                  ? policy.memberTemplates
                      .map((key) => form.ports.find((member) => member.key === key)?.title ?? key)
                      .join(" / ")
                  : port.title;
              return (
                <div key={port.key} className="space-y-1">
                  <label className="flex items-center justify-between gap-3 text-xs">
                    <span>{title}</span>
                    {policy.kind === "configurable" ? (
                      <PortCountField
                        title={title}
                        count={form.portCounts[port.key]}
                        min={policy.min}
                        max={policy.max}
                        pending={creating || pendingKeys.has(`port:${port.key}`)}
                        onCommit={(value) => commitPortCount(policy.memberTemplates, value)}
                      />
                    ) : (
                      <span className="text-muted-foreground">
                        {policy.kind === "fixed" ? "1" : t("canvas.nodePalette.derivedPins")}
                      </span>
                    )}
                  </label>
                  {errors[`port:${port.key}`] != null && (
                    <p role="alert" className="text-xs text-destructive">
                      {formatInlineUserError(errors[`port:${port.key}`], t)}
                    </p>
                  )}
                </div>
              );
            })}
          </fieldset>
        )}
        {form?.groups.map((group) => (
          <fieldset key={group.key} className="space-y-3">
            <legend className="mb-2 text-xs font-medium">{group.display.title}</legend>
            {group.parameters.map((parameter) => (
              <div key={parameter.key} className="space-y-1">
                <div className="text-xs">{parameter.display.title}</div>
                {parameter.display.description && (
                  <p className="text-xs text-muted-foreground">{parameter.display.description}</p>
                )}
                <NodeParameterField
                  graphPath={graphPath}
                  parameter={parameter}
                  pending={creating || pendingKeys.has(`parameter:${parameter.key}`)}
                  errors={
                    errors[`parameter:${parameter.key}`]
                      ? [formatInlineUserError(errors[`parameter:${parameter.key}`], t)]
                      : []
                  }
                  formatFallback={(value) => (value == null ? "—" : JSON.stringify(value))}
                  onCommit={(value, callbacks) =>
                    commitParameter(parameter.key, value, callbacks?.onResolved)
                  }
                />
              </div>
            ))}
          </fieldset>
        ))}
        {form?.groups.length === 0 && (
          <p className="text-xs text-muted-foreground">{t("canvas.nodePalette.noParameters")}</p>
        )}
      </div>
      {createError && (
        <p role="alert" className="text-xs text-destructive">
          {createError}
        </p>
      )}
      <div className="flex justify-end gap-2">
        <Button type="button" variant="ghost" size="sm" disabled={creating} onClick={onBack}>
          {t("canvas.nodePalette.back")}
        </Button>
        <Button
          type="submit"
          size="sm"
          disabled={!form || creating || Object.keys(errors).length > 0}
        >
          {t("canvas.nodePalette.create")}
        </Button>
      </div>
    </form>
  );
}

function PortCountField({
  title,
  count,
  min,
  max,
  pending,
  onCommit,
}: {
  title: string;
  count: number;
  min: number;
  max: number | null;
  pending: boolean;
  onCommit(count: number): void;
}) {
  const [draft, setDraft] = useState(String(count));
  const [projection, setProjection] = useState(count);
  if (projection !== count) {
    setProjection(count);
    setDraft(String(count));
  }
  const submit = () => {
    const value = Number(draft);
    if (draft !== "" && Number.isInteger(value) && value >= 0 && value <= 65535) onCommit(value);
  };
  return (
    <Input
      className="h-7 w-20"
      aria-label={title}
      type="number"
      required
      min={min}
      max={max ?? 65535}
      step={1}
      disabled={pending}
      value={draft}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={submit}
      onKeyDown={(event) => {
        if (event.key === "Enter") {
          event.preventDefault();
          submit();
        }
      }}
    />
  );
}

import { useEffect, useId, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { useTranslation } from "react-i18next";
import { VscChevronDown, VscClose } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Checkbox } from "@/components/ui/checkbox";
import {
  languageModelConfigSchema,
  REASONING_EFFORTS,
  type LanguageModelConfig,
  type LanguageModelProvider,
} from "@/services/assistant/modelContract";
import { SettingsField } from "./SettingsField";

function encodeParameters(value: LanguageModelConfig["additionalParameters"]) {
  return Object.keys(value).length ? JSON.stringify(value, null, 2) : "";
}

export function LanguageModelEditor({
  model,
  protocol,
  onChange,
  onRemove,
}: {
  model: LanguageModelConfig;
  protocol: LanguageModelProvider["protocol"];
  onChange: (patch: Partial<LanguageModelConfig>) => void;
  onRemove: () => void;
}) {
  const { t } = useTranslation();
  const id = useId();
  const [expanded, setExpanded] = useState(!model.id);
  const [advanced, setAdvanced] = useState(false);
  const [parameters, setParameters] = useState(() => encodeParameters(model.additionalParameters));
  const [invalidParameters, setInvalidParameters] = useState(false);
  const lastParameters = useRef(model.additionalParameters);
  const input = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    if (lastParameters.current !== model.additionalParameters) {
      lastParameters.current = model.additionalParameters;
      setParameters(encodeParameters(model.additionalParameters));
      setInvalidParameters(false);
      input.current?.setCustomValidity("");
    }
  }, [model.additionalParameters]);

  return (
    <Collapsible
      open={expanded}
      onOpenChange={setExpanded}
      className="rounded-lg border border-border bg-card"
      onInvalidCapture={(event) => {
        const inAdvanced =
          event.target instanceof HTMLElement && !!event.target.closest("[data-model-advanced]");
        // Keep mounted fields validated, and reveal them before native validation moves focus.
        flushSync(() => {
          setExpanded(true);
          if (inAdvanced) setAdvanced(true);
        });
      }}
    >
      <div className="flex items-center gap-2 px-2">
        <CollapsibleTrigger asChild>
          <Button
            type="button"
            variant="ghost"
            className="group h-auto min-w-0 flex-1 justify-start gap-3 px-2 py-3 text-left hover:bg-transparent aria-expanded:bg-transparent dark:hover:bg-transparent"
          >
            <VscChevronDown
              aria-hidden
              className="text-muted-foreground transition-transform group-data-[state=closed]:-rotate-90"
            />
            <span className="min-w-0 flex-1">
              <span className="block truncate text-sm font-medium">
                {model.name || model.id || t("settings.models.newModel")}
              </span>
              {model.id && (
                <span className="block truncate pt-1 font-mono text-xs font-normal text-muted-foreground">
                  {model.id}
                </span>
              )}
            </span>
          </Button>
        </CollapsibleTrigger>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="text-muted-foreground hover:text-destructive"
          aria-label={`${t("settings.models.removeModel")} ${model.name || model.id}`}
          onClick={onRemove}
        >
          <VscClose aria-hidden />
        </Button>
      </div>
      <CollapsibleContent
        forceMount
        className="border-t border-border px-4 pb-4 data-[state=closed]:hidden"
      >
        <div className="settings-fields">
          <SettingsField htmlFor={`${id}-model`} label={t("settings.models.modelId")}>
            <Input
              id={`${id}-model`}
              required
              value={model.id}
              onChange={(event) => onChange({ id: event.target.value })}
              placeholder="model-id"
            />
          </SettingsField>
          <SettingsField htmlFor={`${id}-name`} label={t("settings.models.displayName")}>
            <Input
              id={`${id}-name`}
              value={model.name}
              placeholder={model.id}
              onChange={(event) => onChange({ name: event.target.value })}
            />
          </SettingsField>
          <SettingsField htmlFor={`${id}-context`} label={t("settings.models.contextWindow")}>
            <Input
              id={`${id}-context`}
              type="number"
              min={1}
              step={1}
              value={model.contextWindow ?? ""}
              placeholder={t("settings.models.providerDefault")}
              onChange={(event) =>
                onChange({ contextWindow: event.target.value ? Number(event.target.value) : null })
              }
            />
          </SettingsField>
          <SettingsField htmlFor={`${id}-output`} label={t("settings.models.maxOutput")}>
            <Input
              id={`${id}-output`}
              type="number"
              min={1}
              step={1}
              required={protocol === "anthropic"}
              value={model.maxOutputTokens ?? ""}
              placeholder={t(
                protocol === "anthropic"
                  ? "settings.models.required"
                  : "settings.models.providerDefault",
              )}
              onChange={(event) =>
                onChange({
                  maxOutputTokens: event.target.value ? Number(event.target.value) : null,
                })
              }
            />
          </SettingsField>
        </div>
        <fieldset className="space-y-2 border-t border-border pt-3">
          <legend className="text-xs font-medium">{t("settings.models.reasoningEfforts")}</legend>
          <p className="text-xs text-muted-foreground">{t("settings.models.reasoningHint")}</p>
          <div className="flex gap-4">
            {REASONING_EFFORTS.map((effort) => (
              <label key={effort} className="flex items-center gap-2 text-xs">
                <Checkbox
                  checked={model.reasoningEfforts?.includes(effort) ?? false}
                  onCheckedChange={(checked) =>
                    onChange({
                      reasoningEfforts: checked
                        ? REASONING_EFFORTS.filter(
                            (item) => item === effort || model.reasoningEfforts?.includes(item),
                          )
                        : (model.reasoningEfforts ?? []).filter((item) => item !== effort),
                    })
                  }
                />
                {t(`panel.assistantEffort.${effort}`)}
              </label>
            ))}
          </div>
        </fieldset>
        <Collapsible
          open={advanced}
          onOpenChange={setAdvanced}
          className="border-t border-border pt-3"
        >
          <CollapsibleTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              className="group -ml-2 gap-2 text-muted-foreground hover:bg-transparent aria-expanded:bg-transparent dark:hover:bg-transparent"
            >
              <VscChevronDown
                aria-hidden
                className="transition-transform group-data-[state=open]:rotate-180"
              />
              {t("settings.models.advanced")}
            </Button>
          </CollapsibleTrigger>
          <CollapsibleContent forceMount data-model-advanced className="data-[state=closed]:hidden">
            <div className="settings-fields">
              <SettingsField
                htmlFor={`${id}-temperature`}
                label={t("settings.models.temperature")}
                description={t("settings.models.samplingHint")}
              >
                <Input
                  id={`${id}-temperature`}
                  type="number"
                  min={0}
                  max={protocol === "anthropic" ? 1 : 2}
                  step="any"
                  value={model.temperature ?? ""}
                  placeholder={t("settings.models.providerDefault")}
                  aria-describedby={`${id}-temperature-description`}
                  onChange={(event) =>
                    onChange({
                      temperature: event.target.value ? Number(event.target.value) : null,
                    })
                  }
                />
              </SettingsField>
              <SettingsField htmlFor={`${id}-top-p`} label="Top P">
                <Input
                  id={`${id}-top-p`}
                  type="number"
                  min={0}
                  max={1}
                  step="any"
                  value={model.topP ?? ""}
                  placeholder={t("settings.models.providerDefault")}
                  aria-describedby={`${id}-temperature-description`}
                  onChange={(event) =>
                    onChange({ topP: event.target.value ? Number(event.target.value) : null })
                  }
                />
              </SettingsField>
              <SettingsField
                htmlFor={`${id}-parameters`}
                label={t("settings.models.additionalParameters")}
                description={t("settings.models.parametersHint")}
              >
                <Textarea
                  ref={input}
                  id={`${id}-parameters`}
                  className="min-h-32 font-mono text-xs leading-relaxed"
                  spellCheck={false}
                  value={parameters}
                  placeholder="{}"
                  aria-invalid={invalidParameters}
                  aria-describedby={`${id}-parameters-description${invalidParameters ? ` ${id}-parameters-error` : ""}`}
                  onChange={(event) => {
                    const value = event.target.value;
                    setParameters(value);
                    try {
                      const parsed = languageModelConfigSchema.shape.additionalParameters.parse(
                        value.trim() ? JSON.parse(value) : {},
                      );
                      event.target.setCustomValidity("");
                      setInvalidParameters(false);
                      lastParameters.current = parsed;
                      onChange({ additionalParameters: parsed });
                    } catch {
                      event.target.setCustomValidity(t("settings.models.parametersInvalid"));
                      setInvalidParameters(true);
                    }
                  }}
                />
                {invalidParameters && (
                  <p
                    id={`${id}-parameters-error`}
                    role="alert"
                    className="text-xs text-destructive"
                  >
                    {t("settings.models.parametersInvalid")}
                  </p>
                )}
              </SettingsField>
            </div>
          </CollapsibleContent>
        </Collapsible>
      </CollapsibleContent>
    </Collapsible>
  );
}

import { useEffect, useId, useRef, useState, type FormEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { VscAdd, VscRefresh } from "react-icons/vsc";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { modelActions } from "@/features/application/assistant/assistantModels";
import { toErrorReference } from "@/features/application/errorReference";
import { ModelService } from "@/services/assistant/modelService";
import {
  emptyLanguageModel,
  languageModelProtocolSchema,
  providerDisplayName,
  type LanguageModelCatalog,
  type LanguageModelConfig,
  type LanguageModelProvider,
} from "@/services/assistant/modelContract";
import { LanguageModelEditor } from "./LanguageModelEditor";
import { LanguageModelProviderSelect } from "./LanguageModelProviderSelect";
import { SettingsField } from "./SettingsField";
import { SettingsPage } from "./SettingsPage";

const PROTOCOL_NAMES = {
  open_ai_responses: "OpenAI Responses",
  open_ai_chat: "OpenAI Chat Completions",
  anthropic: "Anthropic Messages",
  gemini: "Gemini Interactions",
};

export function ProviderEditor({
  initial,
  initialPresetId,
  presets,
  stored,
  saving,
  notice,
  onOverview,
  onClose,
}: {
  initial: LanguageModelProvider;
  initialPresetId?: string;
  presets: LanguageModelCatalog["presets"];
  stored?: { config: LanguageModelProvider; hasApiKey: boolean };
  saving: boolean;
  notice?: ReactNode;
  onOverview: () => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const id = useId();
  const [config, setConfig] = useState(initial);
  const [presetId, setPresetId] = useState(
    () => initialPresetId ?? presets.find((preset) => preset.name === initial.name)?.id ?? "",
  );
  const [apiKey, setApiKey] = useState("");
  const [discovering, setDiscovering] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const discoveryRequest = useRef(0);
  useEffect(() => {
    ++discoveryRequest.current;
    setDiscovering(false);
    return () => {
      ++discoveryRequest.current;
    };
  }, [
    config.baseUrl,
    config.protocol,
    config.adapter,
    config.authentication,
    apiKey,
    stored?.config.baseUrl,
    stored?.config.protocol,
    stored?.config.adapter,
    stored?.config.authentication,
    stored?.hasApiKey,
  ]);
  const hasStoredKey =
    !!stored?.hasApiKey &&
    config.name === stored.config.name &&
    config.adapter === stored.config.adapter;
  const needsReplacementKey =
    !!stored?.hasApiKey && !hasStoredKey && config.authentication === "api_key";
  const connectionReady =
    !!config.name.trim() &&
    !!config.baseUrl.trim() &&
    (config.authentication === "none" || !!apiKey.trim() || hasStoredKey);
  const addModels = (models: LanguageModelConfig[]) =>
    setConfig((current) => {
      const ids = new Set(current.models.map((model) => model.id));
      return {
        ...current,
        models: [
          ...current.models,
          ...models.filter((model) => {
            if (ids.has(model.id)) return false;
            ids.add(model.id);
            return true;
          }),
        ],
      };
    });
  const setModel = (index: number, patch: Partial<LanguageModelConfig>) =>
    setConfig((current) => ({
      ...current,
      models: current.models.map((model, i) => (i === index ? { ...model, ...patch } : model)),
    }));
  const discover = async () => {
    if (!connectionReady || saving || discovering) return;
    const current = ++discoveryRequest.current;
    setDiscovering(true);
    setError(null);
    try {
      const models = await ModelService.discoverModels(config, apiKey.trim() ? apiKey : null);
      if (current === discoveryRequest.current) addModels(models);
    } catch (failure) {
      if (current === discoveryRequest.current)
        setError(toErrorReference(failure, "assistant_provider_connection_failed").code);
    } finally {
      if (current === discoveryRequest.current) setDiscovering(false);
    }
  };
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const candidate = {
      ...config,
      name: config.name.trim(),
      customName: config.customName?.trim() || null,
      baseUrl: config.baseUrl.trim(),
      models: config.models.map((model) => ({
        ...model,
        id: model.id.trim(),
        name: model.name.trim() || model.id.trim(),
      })),
    };
    if (
      (needsReplacementKey && !apiKey.trim()) ||
      candidate.models.some(
        (model) => !model.id || (candidate.protocol === "anthropic" && !model.maxOutputTokens),
      )
    ) {
      setError("assistant_provider_configuration_invalid");
      return;
    }
    if (await modelActions.save(candidate, apiKey || null)) {
      setApiKey("");
      setError(null);
      setConfig(candidate);
    }
  };

  return (
    <SettingsPage
      breadcrumbs={[
        { label: t("settings.sections.ai"), onSelect: onOverview },
        { label: t("settings.models.providers"), onSelect: onClose },
        { label: providerDisplayName(config) || t("settings.models.addProvider") },
      ]}
      notice={
        notice || error ? (
          <>
            {notice}
            {error && (
              <Alert variant="destructive">
                <AlertDescription>
                  {t(`panel.assistantErrors.${error}`, {
                    defaultValue: t("settings.models.failed"),
                  })}
                </AlertDescription>
              </Alert>
            )}
          </>
        ) : null
      }
    >
      <form onSubmit={(event) => void submit(event)} className="space-y-6">
        <fieldset disabled={saving} className="min-w-0 space-y-6">
          <section className="space-y-2" aria-labelledby={`${id}-connection`}>
            <h3 id={`${id}-connection`} className="text-sm font-semibold">
              {t("settings.models.connection")}
            </h3>
            <div className="settings-fields">
              <SettingsField
                htmlFor={`${id}-custom-name`}
                label={t("settings.models.customName")}
                description={t("settings.models.customNameDescription")}
              >
                <Input
                  id={`${id}-custom-name`}
                  value={config.customName ?? ""}
                  aria-describedby={`${id}-custom-name-description`}
                  onChange={(event) => setConfig({ ...config, customName: event.target.value })}
                />
              </SettingsField>
              <SettingsField htmlFor={`${id}-provider`} label={t("settings.models.providerName")}>
                <LanguageModelProviderSelect
                  id={`${id}-provider`}
                  value={presetId}
                  name={config.name}
                  presets={presets}
                  disabled={saving || !presets.length}
                  onSelect={(preset) => {
                    if (preset.id === presetId) return;
                    setPresetId(preset.id);
                    ++discoveryRequest.current;
                    setDiscovering(false);
                    setApiKey("");
                    setError(null);
                    setConfig((current) => ({
                      id: current.id,
                      name: preset.name,
                      customName: current.customName,
                      protocol: preset.protocol,
                      adapter: preset.adapter,
                      authentication: preset.authentication,
                      baseUrl: preset.baseUrl,
                      models: [],
                    }));
                  }}
                />
              </SettingsField>
              <SettingsField htmlFor={`${id}-protocol`} label={t("settings.models.protocol")}>
                <Select
                  disabled={saving}
                  value={config.protocol}
                  onValueChange={(value) => {
                    const protocol = languageModelProtocolSchema.parse(value);
                    const family =
                      protocol === "anthropic"
                        ? "anthropic"
                        : protocol === "gemini"
                          ? "gemini"
                          : "openai";
                    setConfig({
                      ...config,
                      protocol,
                      adapter: config.adapter.endsWith(`/${family}`)
                        ? config.adapter
                        : family === "gemini"
                          ? "gcp.gemini/gemini"
                          : `${family}/${family}`,
                      authentication: family === "openai" ? config.authentication : "api_key",
                    });
                  }}
                >
                  <SelectTrigger id={`${id}-protocol`}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {languageModelProtocolSchema.options.map((protocol) => (
                      <SelectItem key={protocol} value={protocol}>
                        {PROTOCOL_NAMES[protocol]}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </SettingsField>
              <SettingsField
                htmlFor={`${id}-url`}
                label={t("settings.models.baseUrl")}
                description={t("settings.models.endpointHint")}
              >
                <Input
                  id={`${id}-url`}
                  type="url"
                  required
                  value={config.baseUrl}
                  aria-describedby={`${id}-url-description`}
                  onChange={(event) => setConfig({ ...config, baseUrl: event.target.value })}
                  placeholder="https://api.example.com/v1"
                />
              </SettingsField>
              <SettingsField htmlFor={`${id}-auth`} label={t("settings.models.authentication")}>
                <Select
                  disabled={saving}
                  value={config.authentication}
                  onValueChange={(value) => {
                    setConfig({ ...config, authentication: value === "none" ? "none" : "api_key" });
                    setApiKey("");
                  }}
                >
                  <SelectTrigger id={`${id}-auth`}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="api_key">API Key</SelectItem>
                    <SelectItem
                      value="none"
                      disabled={config.protocol === "anthropic" || config.protocol === "gemini"}
                    >
                      {t("settings.models.noAuthentication")}
                    </SelectItem>
                  </SelectContent>
                </Select>
              </SettingsField>
              {config.authentication === "api_key" && (
                <SettingsField
                  htmlFor={`${id}-key`}
                  label="API Key"
                  description={t(
                    needsReplacementKey
                      ? "settings.models.newProviderKeyHint"
                      : "settings.models.keyHint",
                  )}
                >
                  <Input
                    id={`${id}-key`}
                    type="password"
                    autoComplete="new-password"
                    required={needsReplacementKey}
                    value={apiKey}
                    aria-describedby={`${id}-key-description`}
                    onChange={(event) => setApiKey(event.target.value)}
                    placeholder={hasStoredKey ? "********" : t("settings.models.enterKey")}
                  />
                </SettingsField>
              )}
            </div>
          </section>
          <section
            className="space-y-4 border-t border-border pt-6"
            aria-labelledby={`${id}-models`}
          >
            <div className="flex flex-wrap items-center justify-between gap-3">
              <h3 id={`${id}-models`} className="text-sm font-semibold">
                {t("settings.models.models")}{" "}
                <span className="ml-1 font-normal text-muted-foreground">
                  {config.models.length}
                </span>
              </h3>
              <div className="flex flex-wrap gap-2">
                <Button
                  type="button"
                  variant="outline"
                  className="h-8 gap-2 px-3"
                  disabled={!connectionReady || saving || discovering}
                  onClick={() => void discover()}
                >
                  <VscRefresh
                    aria-hidden
                    className={discovering ? "animate-spin motion-reduce:animate-none" : undefined}
                  />
                  {t(discovering ? "settings.models.discovering" : "settings.models.discover")}
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  className="h-8 gap-2 px-3"
                  onClick={() =>
                    setConfig((current) => ({
                      ...current,
                      models: [...current.models, emptyLanguageModel()],
                    }))
                  }
                >
                  <VscAdd aria-hidden />
                  {t("settings.models.addModel")}
                </Button>
              </div>
            </div>
            <p className="text-xs leading-relaxed text-muted-foreground">
              {t("settings.models.toolsHint")}
            </p>
            <div className="space-y-3">
              {config.models.map((model, index) => (
                <LanguageModelEditor
                  key={index}
                  model={model}
                  protocol={config.protocol}
                  onChange={(patch) => setModel(index, patch)}
                  onRemove={() =>
                    setConfig({ ...config, models: config.models.filter((_, i) => i !== index) })
                  }
                />
              ))}
            </div>
          </section>
        </fieldset>
        <div className="sticky bottom-0 z-10 flex flex-wrap items-center justify-end gap-2 border-t border-border bg-background/95 py-4 backdrop-blur-sm">
          {stored && (
            <Button
              type="button"
              variant="ghost"
              className="mr-auto h-9 px-3 text-destructive hover:text-destructive"
              disabled={saving || discovering}
              onClick={() =>
                void modelActions.remove(config.id).then((removed) => {
                  if (removed) onClose();
                })
              }
            >
              {t("settings.models.removeProvider")}
            </Button>
          )}
          <Button type="button" variant="outline" className="h-9 px-4" onClick={onClose}>
            {t("settings.models.closeEditor")}
          </Button>
          <Button type="submit" className="h-9 px-5" disabled={saving || discovering}>
            {t(saving ? "settings.models.saving" : "settings.models.save")}
          </Button>
        </div>
      </form>
    </SettingsPage>
  );
}

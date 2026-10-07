import { useId, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { VscAdd, VscChevronRight } from "react-icons/vsc";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { modelActions, useAssistantModels } from "@/features/application/assistant/assistantModels";
import {
  modelSelectionKey,
  providerDisplayName,
  type LanguageModelProvider,
} from "@/services/assistant/modelContract";
import { ProviderEditor } from "./LanguageModelProviderEditor";
import { SettingsField } from "./SettingsField";
import { SettingsPage } from "./SettingsPage";

type ModelSettingsPage =
  | { kind: "overview" }
  | { kind: "providers" }
  | { kind: "provider"; initial: LanguageModelProvider; initialPresetId?: string };

export function LanguageModelSettings({ notice }: { notice?: ReactNode }) {
  const { t } = useTranslation();
  const id = useId();
  const snapshot = useAssistantModels();
  const [page, setPage] = useState<ModelSettingsPage>({ kind: "overview" });
  const goOverview = () => setPage({ kind: "overview" });
  const goProviders = () => setPage({ kind: "providers" });
  const catalog = snapshot.catalog;
  const choices =
    catalog?.providers.flatMap(({ config, hasApiKey }) =>
      config.models.map((model) => ({
        key: modelSelectionKey({ providerId: config.id, modelId: model.id }),
        providerId: config.id,
        modelId: model.id,
        name: `${providerDisplayName(config)} · ${model.name}`,
        enabled: config.authentication === "none" || hasApiKey,
      })),
    ) ?? [];

  const addProviderAction = (
    <Button
      type="button"
      variant="outline"
      className="h-8 gap-2 px-3"
      disabled={snapshot.saving || !catalog?.presets.length}
      onClick={() => {
        const preset = catalog?.presets[0];
        if (!preset) return;
        setPage({
          kind: "provider",
          initialPresetId: preset.id,
          initial: {
            id: crypto.randomUUID(),
            name: preset.name,
            customName: null,
            protocol: preset.protocol,
            adapter: preset.adapter,
            authentication: preset.authentication,
            baseUrl: preset.baseUrl,
            models: [],
          },
        });
      }}
    >
      <VscAdd aria-hidden />
      {t("settings.models.addProvider")}
    </Button>
  );

  const pageNotice =
    notice || snapshot.error ? (
      <>
        {notice}
        {snapshot.error && (
          <Alert variant="destructive">
            <AlertDescription className="flex flex-wrap items-center justify-between gap-3">
              <span className="min-w-0 flex-1">
                {t(`panel.assistantErrors.${snapshot.error.code}`, {
                  defaultValue: t("settings.models.failed"),
                })}
              </span>
              {snapshot.error.operation === "load" && (
                <Button
                  type="button"
                  variant="outline"
                  className="h-8 shrink-0 px-3"
                  disabled={snapshot.loading || snapshot.saving}
                  onClick={() => void modelActions.reload()}
                >
                  {t("common.retry")}
                </Button>
              )}
            </AlertDescription>
          </Alert>
        )}
      </>
    ) : null;

  if (page.kind === "provider") {
    return (
      <ProviderEditor
        key={page.initial.id}
        initial={page.initial}
        initialPresetId={page.initialPresetId}
        presets={catalog?.presets ?? []}
        stored={catalog?.providers.find((entry) => entry.config.id === page.initial.id)}
        saving={snapshot.saving}
        notice={pageNotice}
        onOverview={goOverview}
        onClose={goProviders}
      />
    );
  }

  return (
    <SettingsPage
      key={page.kind}
      breadcrumbs={
        page.kind === "providers"
          ? [
              { label: t("settings.sections.ai"), onSelect: goOverview },
              { label: t("settings.models.providers") },
            ]
          : [{ label: t("settings.sections.ai") }]
      }
      actions={page.kind === "providers" ? addProviderAction : undefined}
      notice={pageNotice}
    >
      {page.kind === "providers" ? (
        <section aria-label={t("settings.models.providers")}>
          {catalog?.providers.length ? (
            <div className="divide-y divide-border overflow-hidden rounded-lg border border-border">
              {catalog.providers.map(({ config, hasApiKey }) => (
                <Button
                  key={config.id}
                  type="button"
                  variant="ghost"
                  className="h-auto w-full justify-start gap-3 rounded-none px-4 py-4 text-left"
                  disabled={snapshot.saving}
                  onClick={() => setPage({ kind: "provider", initial: config })}
                  aria-label={`${t("settings.models.edit")} ${providerDisplayName(config)}`}
                >
                  <span className="min-w-0 flex-1 space-y-1">
                    <span className="block truncate text-sm font-medium">
                      {providerDisplayName(config)}
                    </span>
                    <span className="block truncate text-xs font-normal text-muted-foreground">
                      {config.name} ·{" "}
                      {t("settings.models.modelCount", { count: config.models.length })} ·{" "}
                      {t(
                        config.authentication === "none"
                          ? "settings.models.noAuthentication"
                          : hasApiKey
                            ? "settings.models.keyStored"
                            : "settings.models.needsKey",
                      )}
                    </span>
                  </span>
                  <VscChevronRight aria-hidden className="text-muted-foreground" />
                </Button>
              ))}
            </div>
          ) : (
            <p className="py-4 text-sm text-muted-foreground">
              {t("settings.models.noConfiguredProviders")}
            </p>
          )}
        </section>
      ) : (
        <div className="settings-fields">
          <SettingsField
            htmlFor={`${id}-default`}
            label={t("settings.models.defaultModel")}
            description={t("settings.models.defaultModelDescription")}
          >
            <Select
              disabled={snapshot.saving || !choices.length}
              value={modelSelectionKey(catalog?.defaultModel ?? null)}
              onValueChange={(value) => {
                const choice = choices.find((model) => model.key === value);
                if (choice)
                  void modelActions.setDefault({
                    providerId: choice.providerId,
                    modelId: choice.modelId,
                  });
              }}
            >
              <SelectTrigger id={`${id}-default`} aria-describedby={`${id}-default-description`}>
                <SelectValue placeholder={t("settings.models.chooseModel")} />
              </SelectTrigger>
              <SelectContent>
                {choices.map((choice) => (
                  <SelectItem key={choice.key} value={choice.key} disabled={!choice.enabled}>
                    {choice.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </SettingsField>
          <SettingsField
            htmlFor={`${id}-providers`}
            label={t("settings.models.modelProviders")}
            description={t("settings.models.modelProvidersDescription")}
          >
            <Button
              id={`${id}-providers`}
              aria-describedby={`${id}-providers-description`}
              type="button"
              variant="outline"
              className="h-8 gap-2 self-end px-3"
              disabled={snapshot.saving}
              onClick={goProviders}
            >
              {t("settings.models.configure")}
              <VscChevronRight aria-hidden />
            </Button>
          </SettingsField>
        </div>
      )}
    </SettingsPage>
  );
}

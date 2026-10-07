import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useStore } from "zustand";
import { Button } from "@/components/ui/button";
import {
  Combobox,
  ComboboxTrigger,
  ComboboxInput,
  ComboboxContent,
  ComboboxList,
  ComboboxItem,
  ComboboxEmpty,
} from "@/components/ui/combobox";
import {
  useAssistantHarnessActions,
  useAssistantHarnessSnapshot,
} from "@/features/application/assistant/AssistantRuntimeProvider";
import { assistantModels } from "@/features/application/assistant/assistantModels";
import { modelSelectionKey, providerDisplayName } from "@/services/assistant/modelContract";
import { ui } from "@/features/core/ui/ui";

export function AssistantModelPicker() {
  const { t } = useTranslation();
  const catalog = useStore(assistantModels, (state) => state.catalog);
  const selectedModel = useAssistantHarnessSnapshot((state) => state.selectedModel);
  const disabled = useAssistantHarnessSnapshot((state) => !state.sessionId || state.selectingModel);
  const { selectModel } = useAssistantHarnessActions();
  const choices = useMemo(
    () =>
      catalog?.providers.flatMap(({ config, hasApiKey }) =>
        config.models.map((model) => ({
          key: modelSelectionKey({ providerId: config.id, modelId: model.id }),
          selection: { providerId: config.id, modelId: model.id },
          label: `${providerDisplayName(config)} · ${model.name}`,
          enabled: config.authentication === "none" || hasApiKey,
        })),
      ) ?? [],
    [catalog],
  );
  const selected =
    choices.find((choice) => choice.key === modelSelectionKey(selectedModel)) ?? null;
  const label =
    selected?.label ??
    t(selectedModel ? "settings.models.unavailable" : "settings.models.chooseModel");
  return (
    <Combobox
      items={choices}
      value={selected}
      onValueChange={(choice) => {
        if (choice?.enabled) void selectModel(choice.selection);
      }}
      itemToStringLabel={(choice) => choice.label}
      isItemEqualToValue={(a, b) => a.key === b.key}
    >
      <ComboboxTrigger
        disabled={disabled}
        render={<Button type="button" variant="ghost" size="xs" />}
        className="max-w-60 min-w-0 shrink gap-1 px-1"
        aria-label={t("settings.models.chooseModel")}
        title={label}
      >
        <span className="truncate">{label}</span>
      </ComboboxTrigger>
      <ComboboxContent side="top" align="end" className="w-80">
        <ComboboxInput
          showTrigger={false}
          placeholder={t("panel.assistantSearchModels")}
          aria-label={t("panel.assistantSearchModels")}
        />
        <ComboboxEmpty>{t("panel.assistantNoModels")}</ComboboxEmpty>
        <ComboboxList>
          {(choice) => (
            <ComboboxItem
              key={choice.key}
              value={choice}
              disabled={!choice.enabled}
              className="text-xs"
            >
              {choice.label}
              {!choice.enabled && ` · ${t("settings.models.needsKey")}`}
            </ComboboxItem>
          )}
        </ComboboxList>
        <Button
          type="button"
          variant="ghost"
          size="xs"
          className="m-1"
          onClick={() => ui.showSettings()}
        >
          {t("settings.models.manage")}
        </Button>
      </ComboboxContent>
    </Combobox>
  );
}

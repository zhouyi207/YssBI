import { useTranslation } from "react-i18next";
import { useStore } from "zustand";
import { useShallow } from "zustand/react/shallow";
import { VscSettingsGear } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { assistantModels } from "@/features/application/assistant/assistantModels";
import {
  useAssistantHarnessActions,
  useAssistantHarnessSnapshot,
} from "@/features/application/assistant/AssistantRuntimeProvider";
import { modelSelectionKey, providerDisplayName } from "@/services/assistant/modelContract";
import { ui } from "@/features/core/ui/ui";

export function AssistantModelPicker() {
  const { t } = useTranslation();
  const catalog = useStore(assistantModels, (state) => state.catalog);
  const { selectedModel, selectingModel, sessionId } = useAssistantHarnessSnapshot(
    useShallow((state) => ({
      selectedModel: state.selectedModel,
      selectingModel: state.selectingModel,
      sessionId: state.sessionId,
    })),
  );
  const { selectModel } = useAssistantHarnessActions();
  const key = modelSelectionKey(selectedModel);
  const choices =
    catalog?.providers.flatMap(({ config, hasApiKey }) =>
      config.models.map((model) => ({
        key: modelSelectionKey({ providerId: config.id, modelId: model.id }),
        selection: { providerId: config.id, modelId: model.id },
        label: `${providerDisplayName(config)} · ${model.name}`,
        enabled: config.authentication === "none" || hasApiKey,
      })),
    ) ?? [];
  return (
    <div className="flex min-w-0 items-center gap-1 border-b border-border/60 px-2 py-1.5">
      <select
        aria-label={t("settings.models.chooseModel")}
        title={t("settings.models.nextTurn")}
        className="h-7 min-w-0 flex-1 rounded-md bg-transparent px-1 text-xs outline-none focus:ring-1 focus:ring-ring"
        disabled={!sessionId || selectingModel}
        value={key}
        onChange={(event) => {
          const model = choices.find((choice) => choice.key === event.target.value);
          if (model) void selectModel(model.selection);
        }}
      >
        {!choices.some((choice) => choice.key === key) && (
          <option value={key}>
            {t(key ? "settings.models.unavailable" : "settings.models.chooseModel")}
          </option>
        )}
        {choices.map((choice) => (
          <option
            className="bg-background text-foreground"
            key={choice.key}
            value={choice.key}
            disabled={!choice.enabled}
          >
            {choice.label}
            {choice.enabled ? "" : ` · ${t("settings.models.needsKey")}`}
          </option>
        ))}
      </select>
      <Button
        type="button"
        size="icon-xs"
        variant="ghost"
        aria-label={t("settings.models.manage")}
        title={t("settings.models.manage")}
        onClick={() => ui.showSettings()}
      >
        <VscSettingsGear aria-hidden />
      </Button>
    </div>
  );
}

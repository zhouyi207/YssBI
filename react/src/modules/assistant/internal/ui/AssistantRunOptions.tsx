import { useTranslation } from "react-i18next";
import { useStore } from "zustand";
import { VscDiscard, VscLightbulb } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
} from "@/components/ui/select";
import {
  useAssistantHarnessActions,
  useAssistantHarnessSnapshot,
} from "@/features/application/assistant/AssistantRuntimeProvider";
import { assistantModels } from "@/features/application/assistant/assistantModels";
import { reasoningEffortSchema, modelReasoningEfforts } from "@/services/assistant/modelContract";

const triggerClass = "w-auto gap-1 border-0 bg-transparent px-1 text-xs shadow-none";

export function AssistantModePicker() {
  const { t } = useTranslation();
  const options = useAssistantHarnessSnapshot((state) => state.turnOptions);
  const { setTurnOptions } = useAssistantHarnessActions();
  return (
    <Select
      value={options.mode}
      onValueChange={(mode) => {
        if (mode === "ask" || mode === "write") setTurnOptions({ ...options, mode });
      }}
    >
      <SelectTrigger
        size="sm"
        className={triggerClass}
        aria-label={t("panel.assistantMode")}
        title={t(`panel.assistantModeHint.${options.mode}`)}
      >
        <span>{t(`panel.assistantModes.${options.mode}`)}</span>
      </SelectTrigger>
      <SelectContent side="top">
        {(["write", "ask"] as const).map((mode) => (
          <SelectItem key={mode} value={mode}>
            <span className="block text-xs">{t(`panel.assistantModes.${mode}`)}</span>
            <span className="block text-[11px] text-muted-foreground">
              {t(`panel.assistantModeHint.${mode}`)}
            </span>
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

export function AssistantEffortPicker() {
  const { t } = useTranslation();
  const options = useAssistantHarnessSnapshot((state) => state.turnOptions);
  const selected = useAssistantHarnessSnapshot((state) => state.selectedModel);
  const provider = useStore(assistantModels, (state) =>
    state.catalog?.providers.find(({ config }) => config.id === selected?.providerId),
  );
  const model = provider?.config.models.find((model) => model.id === selected?.modelId);
  const defaultEffort = provider?.reasoningDefaults?.[selected?.modelId ?? ""] ?? null;
  const efforts = modelReasoningEfforts(model, defaultEffort);
  const { setTurnOptions } = useAssistantHarnessActions();
  return (
    <div className="flex min-w-0 items-center">
      <Select
        value={options.reasoningEffort ?? defaultEffort ?? ""}
        disabled={!efforts?.length}
        onValueChange={(value) => {
          const effort = reasoningEffortSchema.safeParse(value);
          if (effort.success)
            setTurnOptions({
              ...options,
              reasoningEffort: effort.data === defaultEffort ? null : effort.data,
            });
        }}
      >
        <SelectTrigger
          size="sm"
          className={triggerClass}
          aria-label={t("panel.assistantReasoningEffort")}
          title={t(
            efforts?.length ? "settings.models.nextTurn" : "panel.assistantEffortUnavailable",
          )}
        >
          <VscLightbulb aria-hidden />
          <SelectValue
            placeholder={t(
              model ? "panel.assistantEffortUnknown" : "panel.assistantReasoningEffort",
            )}
          />
        </SelectTrigger>
        <SelectContent side="top">
          {efforts?.map((effort) => (
            <SelectItem key={effort} value={effort}>
              {effort === defaultEffort
                ? t("panel.assistantEffortDefault", { value: t(`panel.assistantEffort.${effort}`) })
                : t(`panel.assistantEffort.${effort}`)}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      {options.reasoningEffort !== null && (
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          aria-label={t("panel.assistantResetEffort")}
          title={t("panel.assistantResetEffort")}
          onClick={() => setTurnOptions({ ...options, reasoningEffort: null })}
        >
          <VscDiscard aria-hidden />
        </Button>
      )}
    </div>
  );
}

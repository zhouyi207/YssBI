import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { useAssistantHarnessSnapshot } from "@/features/application/assistant/AssistantRuntimeProvider";

export function AssistantTokenUsage() {
  const { t, i18n } = useTranslation();
  const usage = useAssistantHarnessSnapshot((state) => state.usage);
  const model = useAssistantHarnessSnapshot(
    (state) => state.messages[state.messages.length - 1]?.model,
  );
  const input = usage.latest?.usage.inputTokens ?? null;
  const capacity = usage.latest?.contextWindow ?? null;
  const ratio = input !== null && capacity !== null ? input / capacity : null;
  const format = (value: number | null) =>
    value === null
      ? t("panel.assistantUsageUnknown")
      : new Intl.NumberFormat(i18n.language).format(value);
  const label =
    ratio === null
      ? t("panel.assistantTokens")
      : t("panel.assistantContextPercent", { value: Math.round(ratio * 100) });
  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button type="button" variant="ghost" size="icon-sm" aria-label={label} title={label}>
          <svg viewBox="0 0 20 20" className="size-4 -rotate-90" aria-hidden>
            <circle
              cx="10"
              cy="10"
              r="7"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              className="text-muted-foreground/25"
            />
            {ratio !== null && (
              <circle
                cx="10"
                cy="10"
                r="7"
                pathLength="100"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeDasharray={`${Math.min(100, ratio * 100)} 100`}
                className={ratio >= 0.9 ? "text-amber-500" : "text-primary"}
              />
            )}
          </svg>
        </Button>
      </PopoverTrigger>
      <PopoverContent side="top" align="end" className="w-72 gap-2 p-3">
        <p className="font-medium">{t("panel.assistantTokens")}</p>
        {model && (
          <p className="truncate text-muted-foreground">
            {model.providerName} · {model.modelName}
          </p>
        )}
        <dl className="grid grid-cols-[1fr_auto] gap-x-3 gap-y-2 tabular-nums">
          <dt>{t("panel.assistantLatestInput")}</dt>
          <dd>
            {format(input)} / {format(capacity)}
          </dd>
          <dt className="col-span-2 mt-1 border-t border-border pt-2 font-medium">
            {t("panel.assistantTurnUsage", { count: usage.calls })}
          </dt>
          {(
            [
              "inputTokens",
              "outputTokens",
              "cachedInputTokens",
              "cacheCreationInputTokens",
              "reasoningTokens",
            ] as const
          ).map((key) => (
            <div key={key} className="contents">
              <dt>{t(`panel.assistantUsageFields.${key}`)}</dt>
              <dd>{format(usage.total[key])}</dd>
            </div>
          ))}
        </dl>
        <p className="text-[11px] leading-5 text-muted-foreground">
          {t("panel.assistantUsageHint")}
        </p>
        {usage.incomplete && (
          <p className="text-[11px] text-muted-foreground">{t("panel.assistantUsagePartial")}</p>
        )}
      </PopoverContent>
    </Popover>
  );
}

import type { HarnessUsageReport, ModelTokenUsage } from "@/services/assistant/harnessContract";

export interface AssistantUsage {
  readonly calls: number;
  readonly incomplete: boolean;
  readonly total: ModelTokenUsage;
  readonly latest: HarnessUsageReport | null;
}
export const EMPTY_USAGE: AssistantUsage = {
  calls: 0,
  incomplete: false,
  latest: null,
  total: {
    inputTokens: null,
    outputTokens: null,
    cachedInputTokens: null,
    cacheCreationInputTokens: null,
    reasoningTokens: null,
  },
};

const add = (a: number | null, b: number | null): number | null =>
  a === null && b === null ? null : (a ?? 0) + (b ?? 0);

/** All calls contribute to reported consumption; only the manager response describes its context. */
export function accumulateUsage(
  previous: AssistantUsage,
  report: HarnessUsageReport,
  worker = false,
): AssistantUsage {
  const { total } = previous;
  const { usage } = report;
  return {
    calls: previous.calls + 1,
    incomplete: previous.incomplete || usage.inputTokens === null || usage.outputTokens === null,
    latest: !worker && report.purpose === "response" ? report : previous.latest,
    total: {
      inputTokens: add(total.inputTokens, usage.inputTokens),
      outputTokens: add(total.outputTokens, usage.outputTokens),
      cachedInputTokens: add(total.cachedInputTokens, usage.cachedInputTokens),
      cacheCreationInputTokens: add(total.cacheCreationInputTokens, usage.cacheCreationInputTokens),
      reasoningTokens: add(total.reasoningTokens, usage.reasoningTokens),
    },
  };
}

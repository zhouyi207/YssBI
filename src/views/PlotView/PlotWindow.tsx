import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { parsePlotPayload, usePresentationWindow } from "@/features/application/presentation";
import { PlotResultView } from "@/features/application/presentation/PlotResultView";
import { PresentationWindowShell } from "@/features/application/window/PresentationWindowShell";

const PLOT_ICON = (
  <svg
    className="h-4 w-4 text-[var(--accent-color)]"
    fill="none"
    stroke="currentColor"
    viewBox="0 0 24 24"
  >
    <path
      strokeLinecap="round"
      strokeLinejoin="round"
      strokeWidth={2}
      d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"
    />
  </svg>
);

export const PlotWindow: React.FC = () => {
  const { t } = useTranslation();
  const { state, windowActions } = usePresentationWindow("plot");

  const plotPayload = useMemo(() => {
    if (state.status !== "ready" || state.payload.mode !== "plot") return null;
    return parsePlotPayload(state.payload.chart, state.payload.data);
  }, [state]);

  const title = state.status === "ready" ? state.descriptor.title : t("plot.title");

  return (
    <PresentationWindowShell
      title={title}
      icon={PLOT_ICON}
      state={state}
      windowActions={windowActions}
      errorMessages={{
        missingResultId: t("info.missingDataKey"),
        notFound: t("plot.noData"),
        loadFailed: t("plot.failedInitialize"),
      }}
      contentClassName="flex min-h-0 flex-1 flex-col p-4"
    >
      <PlotResultView payload={plotPayload} invalidContent={t("plot.invalidData")} />
    </PresentationWindowShell>
  );
};

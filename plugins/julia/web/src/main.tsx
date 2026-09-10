import { createOperationId } from "@/sdk";
import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { createRoot } from "react-dom/client";
import i18n from "i18next";
import { initReactI18next, useTranslation } from "react-i18next";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Button } from "@/components/ui/button";
import { BayesView } from "@/modules/bayes/public";
import { ChartThemeContextProvider } from "@/shared/charts/core/theme";
import { getChartSeriesColors, getChartThemeColors } from "@/shared/theme/chartTheme";
import { resolveThemeTokens } from "@/shared/theme/themeTokens";
import { DEFAULT_DARK_THEME, DEFAULT_LIGHT_THEME } from "@/shared/theme/colorThemePresets";
import { zhCN } from "@/app/i18n/locales/zh-CN";
import { enUS } from "@/app/i18n/locales/en-US";
import { context, ready, request, subscribe } from "@/sdk";
import "./styles.css";

function RuntimePage() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<{
    runtimeState: string;
    environmentState: string;
    processState: string;
  } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [task, setTask] = useState<{ taskId: string; state: string } | null>(null);
  const preparationOperation = useRef<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const refresh = () =>
    void request<typeof status>("dependencies.inspect")
      .then((value) => {
        setStatus(value);
        setError(null);
      })
      .catch(() => setError(t("julia.worker.statusFailed")));
  useEffect(refresh, []);
  useEffect(() => {
    if (!task || ["succeeded", "failed", "cancelled", "outcomeUnknown"].includes(task.state))
      return;
    let stopped = false;
    let polling = false;
    const timer = setInterval(() => {
      if (polling) return;
      polling = true;
      void request<{ taskId: string; state: string; error?: { code: string } }>("tasks.get", {
        taskId: task.taskId,
      })
        .then((value) => {
          if (stopped) return;
          setTask(value);
          if (["succeeded", "failed", "cancelled", "outcomeUnknown"].includes(value.state))
            preparationOperation.current = null;
          if (value.state === "succeeded") refresh();
          if (value.state === "failed" || value.state === "outcomeUnknown")
            setError(t("julia.worker.unavailable"));
        })
        .catch(() => {
          if (!stopped) {
            setError(t("julia.worker.statusFailed"));
          }
        })
        .finally(() => {
          polling = false;
        });
    }, 500);
    return () => {
      stopped = true;
      clearInterval(timer);
    };
  }, [task?.taskId, task?.state]);
  const busy = task && !["succeeded", "failed", "cancelled", "outcomeUnknown"].includes(task.state);
  return (
    <div className="space-y-5 p-4 text-xs">
      <h1 className="font-semibold">{t("plugins.juliaName")}</h1>
      <div role="status" className="space-y-2">
        <p>
          {status?.processState === "running"
            ? t("julia.worker.ready")
            : status?.runtimeState === "missing"
              ? t("julia.status.notInstalled")
              : t("julia.worker.unavailable")}
        </p>
        <p className="leading-relaxed text-muted-foreground">{t("plugins.juliaDescription")}</p>
      </div>
      {error && (
        <p role="alert" className="text-destructive">
          {error}
        </p>
      )}
      <div className="flex flex-col items-start gap-3">
        <Button
          disabled={submitting || !!busy || status?.runtimeState === "missing"}
          variant="outline"
          onClick={() => {
            setError(null);
            setSubmitting(true);
            preparationOperation.current ??= createOperationId();
            void request<{ taskId: string; state: string }>("tasks.start", {
              taskType: "runtime.prepare",
              operationId: preparationOperation.current,
              parameters: {},
              timeoutMs: 600000,
            })
              .then((value) => {
                setTask(value);
                if (["succeeded", "failed", "cancelled", "outcomeUnknown"].includes(value.state))
                  preparationOperation.current = null;
              })
              .catch(() => setError(t("julia.worker.unavailable")))
              .finally(() => setSubmitting(false));
          }}
        >
          {busy ? t("julia.worker.starting") : t("julia.menu.prepare")}
        </Button>
        {busy && (
          <Button
            variant="outline"
            onClick={() =>
              void request("tasks.cancel", { taskId: task.taskId }).catch(() =>
                setError(t("julia.worker.unavailable")),
              )
            }
          >
            {t("common.cancel")}
          </Button>
        )}
        <Button
          variant="outline"
          onClick={() =>
            void request("views.open", { viewId: "analysis" }).catch(() =>
              setError(t("julia.worker.unavailable")),
            )
          }
        >
          {t("plugins.openBayes")}
        </Button>
        <Button variant="ghost" onClick={refresh}>
          {t("plugins.recheck")}
        </Button>
      </div>
    </div>
  );
}
function App() {
  const value = useSyncExternalStore(subscribe, context);
  useEffect(() => {
    document.documentElement.classList.toggle("dark", value.themeMode === "dark");
    for (const [name, color] of Object.entries(value.theme)) {
      if (/^--[a-z0-9-]+$/.test(name)) document.documentElement.style.setProperty(name, color);
    }
    void i18n.changeLanguage(value.language);
  }, [value]);
  const tokens = resolveThemeTokens(
    value.themeMode === "light" ? DEFAULT_LIGHT_THEME : DEFAULT_DARK_THEME,
  );
  return (
    <TooltipProvider>
      <ChartThemeContextProvider
        value={{ colors: getChartThemeColors(tokens), series: getChartSeriesColors(tokens) }}
      >
        {value.viewId === "analysis" ? <BayesView /> : <RuntimePage />}
      </ChartThemeContextProvider>
    </TooltipProvider>
  );
}
void ready.then(async () => {
  await i18n.use(initReactI18next).init({
    resources: { "zh-CN": { translation: zhCN }, "en-US": { translation: enUS } },
    lng: context().language,
    fallbackLng: "en-US",
    interpolation: { escapeValue: false },
  });
  createRoot(document.getElementById("root")!).render(<App />);
});

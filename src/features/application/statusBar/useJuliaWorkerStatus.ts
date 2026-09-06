import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { JuliaRuntimeService, type JuliaWorkerStatus } from "@/services/julia/juliaRuntimeService";

export interface JuliaWorkerStatusViewModel {
  state: "checking" | "starting" | "ready" | "unavailable";
  needsInstall: boolean;
  label: string;
  tooltip: string;
}

export function useJuliaWorkerStatus(refreshKey = 0): JuliaWorkerStatusViewModel {
  const { t } = useTranslation();
  const [status, setStatus] = useState<JuliaWorkerStatus | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let disposed = false;
    let timer: ReturnType<typeof setTimeout> | null = null;

    const refresh = async () => {
      try {
        const next = await JuliaRuntimeService.getWorkerStatus();
        if (disposed) return;
        setStatus(next);
        setFailed(false);
        const delay = next.processState === "starting" ? 1_000 : 10_000;
        timer = setTimeout(refresh, delay);
      } catch {
        if (disposed) return;
        setFailed(true);
        timer = setTimeout(refresh, 10_000);
      }
    };

    void refresh();
    return () => {
      disposed = true;
      if (timer) clearTimeout(timer);
    };
  }, [refreshKey]);

  if (!status && !failed) {
    return {
      state: "checking",
      needsInstall: false,
      label: t("julia.worker.checking"),
      tooltip: t("julia.worker.checkingDetail"),
    };
  }
  if (failed || !status) {
    return {
      state: "unavailable",
      needsInstall: false,
      label: t("julia.worker.unavailable"),
      tooltip: t("julia.worker.statusFailed"),
    };
  }
  if (status.processState === "starting") {
    return {
      state: "starting",
      needsInstall: false,
      label: t("julia.worker.starting"),
      tooltip: t("julia.worker.startingDetail"),
    };
  }
  if (
    status.runtimeState === "ready" &&
    status.environmentState === "ready" &&
    status.processState === "running"
  ) {
    return {
      state: "ready",
      needsInstall: false,
      label: t("julia.worker.ready"),
      tooltip: t("julia.worker.readyDetail"),
    };
  }
  return {
    state: "unavailable",
    needsInstall: status.runtimeState !== "ready",
    label: t("julia.worker.unavailable"),
    tooltip: t("julia.worker.unavailableDetail"),
  };
}

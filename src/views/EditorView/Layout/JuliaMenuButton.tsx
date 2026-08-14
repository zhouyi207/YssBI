import { logger } from "@/utils/appLogger";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { uiStore } from "@/features/core/ui/UIStore";
import {
  JuliaRuntimeService,
  type JuliaWorkerStatus,
} from "@/services/julia/juliaRuntimeService";
import { formatErrorMessage } from "@/shared/utils/formatErrorMessage";

export function JuliaMenuButton({ onOpenBayes }: { onOpenBayes: () => void }) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<JuliaWorkerStatus | null>(null);
  const [loading, setLoading] = useState(false);

  const refreshStatus = useCallback(async () => {
    try {
      setStatus(await JuliaRuntimeService.getWorkerStatus());
    } catch (error) {
      logger.notify.error(formatErrorMessage(error), "UI");
    }
  }, []);

  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus]);

  const install = useCallback(async () => {
    const confirmed = await uiStore.confirm({
      title: t("julia.install.title"),
      message: t("julia.install.message"),
      confirmText: t("julia.install.confirm"),
    });
    if (!confirmed) return;

    setLoading(true);
    uiStore.startProgress({ stage: t("julia.install.preparing"), detail: t("julia.install.preparingDetail") });
    try {
      const nextStatus = await JuliaRuntimeService.install();
      if (nextStatus.state === "ready") {
        logger.notify.info(t("julia.install.success", { version: nextStatus.version }), "UI");
      } else {
        logger.notify.error(nextStatus.message ?? t("julia.install.failed"), "UI");
      }
    } catch (error) {
      logger.notify.error(formatErrorMessage(error), "UI");
    } finally {
      uiStore.finishProgress();
      setLoading(false);
      void refreshStatus();
    }
  }, [refreshStatus, t]);

  const ready = status?.processState === "running";
  const runtimeReady = status?.runtimeState === "ready";
  const statusLabel = loading
    ? t("julia.status.installing")
    : status?.processState === "starting"
      ? t("julia.worker.starting")
      : ready
        ? t("julia.worker.ready")
        : t("julia.worker.unavailable");

  return (
    <DropdownMenu onOpenChange={(open) => open && void refreshStatus()}>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="sm" className="h-7 px-3 text-sm text-muted-foreground hover:text-foreground">
          {t("menubar.extensions")}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="min-w-[220px] py-0">
        <DropdownMenuLabel>{t("menubar.extensions")}</DropdownMenuLabel>
        <DropdownMenuItem onSelect={onOpenBayes}>
          {t("bayes.openWindow")}
        </DropdownMenuItem>
        <DropdownMenuSeparator className="my-0" />
        <DropdownMenuLabel>{t("julia.menu.title")}</DropdownMenuLabel>
        <DropdownMenuItem disabled className="text-xs">
          {statusLabel}
        </DropdownMenuItem>
        <DropdownMenuSeparator className="my-0" />
        {!runtimeReady && (
          <DropdownMenuItem disabled={loading} onSelect={() => void install()}>
            {t("julia.menu.install")}
          </DropdownMenuItem>
        )}
        <DropdownMenuItem onSelect={() => void refreshStatus()}>
          {t("julia.menu.refresh")}
        </DropdownMenuItem>
        {ready && (
          <DropdownMenuItem disabled title={status?.projectDir ?? undefined}>
            {t("julia.menu.managedRuntime")}
          </DropdownMenuItem>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

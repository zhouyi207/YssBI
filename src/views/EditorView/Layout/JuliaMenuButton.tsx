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
  type JuliaRuntimeStatus,
} from "@/services/julia/juliaRuntimeService";
import { formatErrorMessage } from "@/shared/utils/formatErrorMessage";

export function JuliaMenuButton({ onOpenBayes }: { onOpenBayes: () => void }) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<JuliaRuntimeStatus | null>(null);
  const [loading, setLoading] = useState(false);

  const refreshStatus = useCallback(async () => {
    try {
      setStatus(await JuliaRuntimeService.getStatus());
    } catch (error) {
      uiStore.showToast(formatErrorMessage(error), "error");
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
      setStatus(nextStatus);
      if (nextStatus.state === "ready") {
        uiStore.showToast(t("julia.install.success", { version: nextStatus.version }), "success");
      } else {
        uiStore.showToast(nextStatus.message ?? t("julia.install.failed"), "error");
      }
    } catch (error) {
      uiStore.showToast(formatErrorMessage(error), "error");
    } finally {
      uiStore.finishProgress();
      setLoading(false);
      void refreshStatus();
    }
  }, [refreshStatus, t]);

  const ready = status?.state === "ready";
  const statusLabel = loading
    ? t("julia.status.installing")
    : ready
      ? t("julia.status.ready", { version: status?.version })
      : status?.state === "invalid"
        ? t("julia.status.invalid")
        : t("julia.status.notInstalled");

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
        {!ready && (
          <DropdownMenuItem disabled={loading} onSelect={() => void install()}>
            {t("julia.menu.install")}
          </DropdownMenuItem>
        )}
        <DropdownMenuItem onSelect={() => void refreshStatus()}>
          {t("julia.menu.refresh")}
        </DropdownMenuItem>
        {ready && (
          <DropdownMenuItem disabled title={status?.installDir ?? undefined}>
            {t("julia.menu.managedRuntime")}
          </DropdownMenuItem>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

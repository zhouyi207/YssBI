import { useState } from "react";
import { useTranslation } from "react-i18next";
import { VscServerProcess } from "react-icons/vsc";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverDescription,
  PopoverHeader,
  PopoverTitle,
  PopoverTrigger,
} from "@/components/ui/popover";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { installJuliaRuntime } from "@/features/application/julia/installJuliaRuntime";
import { useJuliaWorkerStatus } from "@/features/application/statusBar/useJuliaWorkerStatus";
import { openBayesWindow } from "@/features/application/window";
import { cn } from "@/lib/utils";

function JuliaMark({ className }: { className?: string }) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        "inline-flex size-5 items-center justify-center rounded-[5px] bg-gradient-to-br from-violet-500 via-fuchsia-500 to-rose-400 text-[9px] font-bold tracking-[-0.08em] text-white shadow-sm",
        className,
      )}
    >
      JL
    </span>
  );
}

export function JuliaRuntimeActions() {
  const { t } = useTranslation();
  const [refreshKey, setRefreshKey] = useState(0);
  const [installing, setInstalling] = useState(false);
  const status = useJuliaWorkerStatus(refreshKey);
  const install = async () => {
    setInstalling(true);
    try {
      await installJuliaRuntime(t);
    } finally {
      setInstalling(false);
      setRefreshKey((value) => value + 1);
    }
  };
  return (
    <div
      data-workbench-julia-actions
      className="flex flex-col items-center"
      onPointerDown={(event) => event.stopPropagation()}
      onMouseDown={(event) => event.stopPropagation()}
    >
      <Popover>
        <Tooltip>
          <TooltipTrigger asChild>
            <PopoverTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="icon"
                data-workbench-julia-action
                aria-label={t("julia.runtime.title")}
                className="relative size-10 bg-transparent p-0 text-muted-foreground hover:bg-transparent dark:hover:bg-transparent"
              >
                <span
                  data-workbench-julia-action-surface
                  className="flex size-8 items-center justify-center rounded-md transition-[color,background-color]"
                >
                  <JuliaMark
                    className={cn(
                      status.state === "starting" && "animate-pulse",
                      status.state === "unavailable" && "grayscale",
                    )}
                  />
                </span>
              </Button>
            </PopoverTrigger>
          </TooltipTrigger>
          <TooltipContent side="right">{t("julia.runtime.title")}</TooltipContent>
        </Tooltip>
        <PopoverContent side="right" align="end" className="w-80 gap-3 p-3">
          <PopoverHeader>
            <PopoverTitle>{t("julia.runtime.title")}</PopoverTitle>
            <PopoverDescription>{t("julia.runtime.description")}</PopoverDescription>
          </PopoverHeader>
          <Badge variant={status.state === "ready" ? "success" : "secondary"}>{status.label}</Badge>
          <p className="text-xs text-muted-foreground">{status.tooltip}</p>
          {status.needsInstall ? (
            <Button disabled={installing} onClick={() => void install()}>
              {installing ? t("julia.install.preparing") : t("julia.install.confirm")}
            </Button>
          ) : (
            <Button disabled={status.state === "checking"} onClick={() => void openBayesWindow()}>
              <VscServerProcess />
              {t("julia.runtime.openBayes")}
            </Button>
          )}
        </PopoverContent>
      </Popover>
    </div>
  );
}

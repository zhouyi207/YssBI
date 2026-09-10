import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { usePluginMaintenance } from "@/features/application/plugins/usePluginMaintenance";
import type { InstalledPlugin } from "@/shared/types/plugins/generated";

export function PluginMaintenanceDialog({
  plugin,
  onClose,
}: {
  plugin: InstalledPlugin | null;
  onClose(): void;
}) {
  const { t } = useTranslation();
  const model = usePluginMaintenance(plugin);
  const mib = (value: number) => `${(value / 1024 / 1024).toFixed(1)} MiB`;
  return (
    <Dialog
      open={plugin !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent className="max-h-[85vh] max-w-3xl overflow-y-auto">
        <DialogHeader>
          <DialogTitle>
            {t("plugins.maintenance", { name: plugin?.manifest.name ?? "" })}
          </DialogTitle>
          <DialogDescription>{t("plugins.maintenanceDescription")}</DialogDescription>
        </DialogHeader>
        {model.error ? (
          <p role="alert" className="text-sm text-destructive">
            {t("plugins.operationFailed")}
          </p>
        ) : null}
        <section className="space-y-3">
          <h3 className="text-sm font-medium">{t("plugins.storage")}</h3>
          {model.storage ? (
            <p className="text-sm">
              {t("plugins.storageUsage", {
                used: mib(model.storage.usedBytes),
                budget: mib(model.storage.budgetBytes),
                cache: mib(model.storage.cacheBytes),
              })}
            </p>
          ) : null}
          <p className="text-xs text-muted-foreground">{t("plugins.softBudget")}</p>
          <p className="break-all text-xs text-muted-foreground">
            {(plugin?.manifest.cacheDirectories ?? []).join(", ")}
          </p>
          <div className="flex flex-wrap gap-2">
            <Button
              size="sm"
              variant="outline"
              disabled={model.busy}
              onClick={() => void model.clearCache()}
            >
              {t("plugins.clearCache")}
            </Button>
            <Button
              size="sm"
              variant="outline"
              disabled={model.busy}
              onClick={() => void model.collectGarbage()}
            >
              {t("plugins.collectPackages")}
            </Button>
            <Button
              size="sm"
              variant="outline"
              disabled={model.busy}
              onClick={() => void model.refresh()}
            >
              {t("plugins.refresh")}
            </Button>
          </div>
        </section>
        <section className="space-y-3">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-medium">{t("plugins.taskHistory")}</h3>
            <Button
              size="sm"
              variant="ghost"
              disabled={model.busy}
              onClick={() => void model.clearHistory()}
            >
              {t("plugins.clearHistory")}
            </Button>
          </div>
          <p className="text-xs text-muted-foreground">{t("plugins.historyRetention")}</p>
          <ul className="divide-y divide-border text-xs">
            {model.tasks.map((task) => (
              <li key={task.taskId} className="flex items-center justify-between gap-3 py-2">
                <span className="truncate" title={task.taskId}>
                  {task.taskId}
                </span>
                <span>{t(`plugins.taskStates.${task.state}`)}</span>
              </li>
            ))}
          </ul>
          {model.cursor ? (
            <Button
              size="sm"
              variant="outline"
              disabled={model.busy}
              onClick={() => void model.more()}
            >
              {t("plugins.loadMore")}
            </Button>
          ) : null}
        </section>
        <section className="space-y-3">
          <h3 className="text-sm font-medium">{t("plugins.diagnostics")}</h3>
          <p className="text-xs text-muted-foreground">{t("plugins.diagnosticsDescription")}</p>
          <pre className="max-h-64 overflow-auto whitespace-pre-wrap break-all rounded border border-border p-3 text-xs">
            {model.diagnostics
              .map((entry) => `[${entry.instanceId} ${entry.taskIds.join(", ")}] ${entry.stderr}`)
              .join("") || t("plugins.noDiagnostics")}
          </pre>
          {model.diagnostics.some((entry) => entry.truncated) ? (
            <p className="text-xs text-muted-foreground">{t("plugins.diagnosticsTruncated")}</p>
          ) : null}
        </section>
      </DialogContent>
    </Dialog>
  );
}

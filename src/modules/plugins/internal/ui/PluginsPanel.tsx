import { useTranslation } from "react-i18next";
import { useState } from "react";
import { PluginMaintenanceDialog } from "./PluginMaintenanceDialog";
import { VscExtensions, VscSettingsGear } from "react-icons/vsc";
import { ActivityPanelDocumentView } from "@/modules/workbench/public";
import { useActivityPanelDocument } from "@/features/application/sidebar/useActivityPanelDocument";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { InstalledPlugin } from "@/shared/types/plugins/generated";

export function PluginsPanel({
  plugins,
  loading,
  busy,
  error,
  onRefresh,
  onInstall,
  onOpen,
  onToggle,
  onUninstall,
}: {
  plugins: InstalledPlugin[];
  loading: boolean;
  busy: boolean;
  error: string | null;
  onRefresh(): void;
  onInstall(): void;
  onOpen(plugin: InstalledPlugin): void;
  onToggle(plugin: InstalledPlugin): void;
  onUninstall(plugin: InstalledPlugin): void;
}) {
  const { t } = useTranslation();
  const [maintenance, setMaintenance] = useState<string | null>(null);
  const query = useActivityPanelDocument("plugins", plugins);
  return (
    <>
      <ActivityPanelDocumentView
        panelId="plugins"
        document={query.document}
        error={query.error}
        expanded={query.expanded}
        onExpandedChange={query.setExpanded}
        busy={busy || loading}
        notice={
          error ? (
            <p role="alert" className="px-3 py-2 text-xs text-destructive">
              {error}
            </p>
          ) : undefined
        }
        onRetry={query.refresh}
        actions={{
          install: onInstall,
          refresh: onRefresh,
        }}
        renderItem={(item) => {
          if (item.kind !== "plugin") return null;
          const plugin = plugins.find((entry) => entry.manifest.id === item.id);
          if (!plugin) return null;
          return (
            <article
              className="flex min-w-0 items-center gap-3 px-3 py-2 hover:bg-muted/50"
              data-plugin-item={item.id}
            >
              <span
                aria-hidden
                className="flex size-9 shrink-0 items-center justify-center rounded-sm border border-border bg-background"
              >
                <VscExtensions className="size-6 text-muted-foreground" />
              </span>
              <div className="min-w-0 flex-1 space-y-1 text-xs">
                <button
                  className="block w-full truncate text-left font-medium"
                  title={item.name}
                  onClick={() => onOpen(plugin)}
                >
                  {item.name}
                </button>
                <p className="truncate text-muted-foreground" title={item.description}>
                  {item.description}
                </p>
                <div className="flex h-5 items-center justify-between gap-2">
                  <span className="truncate text-[11px] text-muted-foreground">
                    {item.publisher}
                    {!item.enabled ? " (" + t("plugins.disabled") + ")" : ""}
                  </span>
                  <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        aria-label={t("plugins.manage", { name: item.name })}
                      >
                        <VscSettingsGear aria-hidden />
                      </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end">
                      <DropdownMenuItem onSelect={() => setMaintenance(plugin.manifest.id)}>
                        {t("plugins.manageData")}
                      </DropdownMenuItem>
                      <DropdownMenuItem onSelect={() => onOpen(plugin)} disabled={!plugin.enabled}>
                        {t("plugins.open")}
                      </DropdownMenuItem>
                      <DropdownMenuItem onSelect={() => onToggle(plugin)} disabled={busy}>
                        {t(plugin.enabled ? "plugins.disable" : "plugins.enable")}
                      </DropdownMenuItem>
                      <DropdownMenuItem onSelect={() => onUninstall(plugin)} disabled={busy}>
                        {t("plugins.uninstall")}
                      </DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>
                </div>
              </div>
            </article>
          );
        }}
      />
      <PluginMaintenanceDialog
        plugin={plugins.find((plugin) => plugin.manifest.id === maintenance) ?? null}
        onClose={() => setMaintenance(null)}
      />
    </>
  );
}

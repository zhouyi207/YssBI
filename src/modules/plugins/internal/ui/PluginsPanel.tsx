import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  VscChevronDown,
  VscChevronRight,
  VscExtensions,
  VscPackage,
  VscRefresh,
  VscSettingsGear,
} from "react-icons/vsc";
import { ActivityPanelShell } from "@/modules/workbench/public";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
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
  const [expanded, setExpanded] = useState(true);
  return (
    <ActivityPanelShell>
      <section
        className="flex min-h-0 min-w-0 flex-1 flex-col text-foreground"
        aria-label={t("activityBar.plugins")}
      >
        <header className="flex h-9 shrink-0 items-center justify-between px-3 text-xs">
          <span>{t("activityBar.plugins")}</span>
          <div className="flex gap-1">
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label={t("plugins.installPackage")}
              title={t("plugins.installPackage")}
              disabled={busy}
              onClick={onInstall}
            >
              <VscPackage aria-hidden />
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label={t("plugins.recheck")}
              title={t("plugins.recheck")}
              disabled={busy}
              onClick={onRefresh}
            >
              <VscRefresh aria-hidden />
            </Button>
          </div>
        </header>
        {error && (
          <p role="alert" className="px-3 py-2 text-xs text-destructive">
            {error}
          </p>
        )}
        <ScrollArea className="min-h-0 flex-1" orientation="vertical">
          <button
            type="button"
            className="flex h-7 w-full items-center gap-1 px-2 text-left text-xs hover:bg-muted/50"
            aria-expanded={expanded}
            onClick={() => setExpanded((value) => !value)}
          >
            {expanded ? <VscChevronDown aria-hidden /> : <VscChevronRight aria-hidden />}
            <span className="flex-1">{t("plugins.installed")}</span>
            <span className="rounded-full bg-muted px-1.5 text-[10px]">{plugins.length}</span>
          </button>
          {expanded &&
            (loading ? (
              <p role="status" className="p-4 text-xs text-muted-foreground">
                {t("common.loading")}
              </p>
            ) : (
              plugins.map((plugin) => (
                <article
                  key={plugin.manifest.id}
                  className="flex min-w-0 items-center gap-3 px-3 py-2 hover:bg-muted/50"
                  data-plugin-item={plugin.manifest.id}
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
                      title={plugin.manifest.name}
                      onClick={() => onOpen(plugin)}
                    >
                      {plugin.manifest.name}
                    </button>
                    <p
                      className="truncate text-muted-foreground"
                      title={plugin.manifest.description}
                    >
                      {plugin.manifest.description}
                    </p>
                    <div className="flex h-5 items-center justify-between gap-2">
                      <span className="truncate text-[11px] text-muted-foreground">
                        {plugin.manifest.publisher}
                        {!plugin.enabled ? " (" + t("plugins.disabled") + ")" : ""}
                      </span>
                      <DropdownMenu>
                        <DropdownMenuTrigger asChild>
                          <Button
                            variant="ghost"
                            size="icon-sm"
                            aria-label={t("plugins.manage", { name: plugin.manifest.name })}
                          >
                            <VscSettingsGear aria-hidden />
                          </Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                          <DropdownMenuItem
                            onSelect={() => onOpen(plugin)}
                            disabled={!plugin.enabled}
                          >
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
              ))
            ))}
        </ScrollArea>
      </section>
    </ActivityPanelShell>
  );
}

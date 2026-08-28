import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { VscExtensions, VscServerProcess } from 'react-icons/vsc';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  Popover,
  PopoverContent,
  PopoverDescription,
  PopoverHeader,
  PopoverTitle,
  PopoverTrigger,
} from '@/components/ui/popover';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import {
  BUILT_IN_PLUGIN_MANIFESTS,
  getInstalledPluginManifests,
  JULIA_PLUGIN_ID,
  type PluginManifest,
} from '@/features/application/plugins/pluginCatalog';
import { installJuliaPlugin } from '@/features/application/plugins/installJuliaPlugin';
import {
  useJuliaWorkerStatus,
  type JuliaWorkerStatusViewModel,
} from '@/features/application/statusBar/useJuliaWorkerStatus';
import { openBayesWindow } from '@/features/application/window';
import { usePluginStore } from '@/features/application/viewCapabilities';
import { cn } from '@/lib/utils';

function stopControlPropagation(event: { stopPropagation(): void }): void {
  event.stopPropagation();
}

function JuliaMark({ className }: { className?: string }) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        'inline-flex size-5 items-center justify-center rounded-[5px] bg-gradient-to-br from-violet-500 via-fuchsia-500 to-rose-400 text-[9px] font-bold tracking-[-0.08em] text-white shadow-sm',
        className,
      )}
    >
      JL
    </span>
  );
}

function PluginActivityButton({
  label,
  children,
  dataSlot,
}: {
  label: string;
  children: React.ReactNode;
  dataSlot: string;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <PopoverTrigger asChild>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            data-workbench-plugin-action
            data-workbench-plugin-slot={dataSlot}
            data-workbench-plugin-manager={dataSlot === 'manager' ? true : undefined}
            aria-label={label}
            className="relative size-10 bg-transparent p-0 text-muted-foreground hover:bg-transparent dark:hover:bg-transparent"
          >
            <span
              data-workbench-plugin-action-surface
              aria-hidden="true"
              className="flex size-8 items-center justify-center rounded-md transition-[color,background-color]"
            >
              {children}
            </span>
          </Button>
        </PopoverTrigger>
      </TooltipTrigger>
      <TooltipContent side="right">{label}</TooltipContent>
    </Tooltip>
  );
}

function JuliaPluginPanel({ status }: { status: JuliaWorkerStatusViewModel }) {
  const { t } = useTranslation();
  const statusVariant = status.state === 'ready'
    ? 'success'
    : status.state === 'starting'
      ? 'warning'
      : status.state === 'unavailable'
        ? 'destructive'
        : 'secondary';

  return (
    <div data-workbench-plugin-view="julia" className="-m-0.5 overflow-hidden">
      <PopoverHeader className="border-b border-border/70 bg-muted/20 p-4">
        <div className="flex items-start gap-3">
          <JuliaMark className="mt-0.5 size-8 rounded-lg text-xs" />
          <div className="min-w-0">
            <PopoverTitle>{t('plugins.julia.title')}</PopoverTitle>
            <PopoverDescription>{t('plugins.julia.description')}</PopoverDescription>
          </div>
        </div>
      </PopoverHeader>
      <div className="space-y-3 p-4">
        <div className="flex items-center justify-between gap-3">
          <span className="text-muted-foreground">{t('plugins.julia.status')}</span>
          <Badge variant={statusVariant}>{status.label}</Badge>
        </div>
        <p className="text-muted-foreground">{status.tooltip}</p>
        <Button
          type="button"
          className="w-full"
          onClick={() => void openBayesWindow()}
        >
          <VscServerProcess />
          {t('plugins.julia.openBayes')}
        </Button>
      </div>
    </div>
  );
}

function JuliaPluginActivitySlot() {
  const { t } = useTranslation();
  const status = useJuliaWorkerStatus();

  return (
    <Popover>
      <PluginActivityButton
        label={t('plugins.julia.title')}
        dataSlot={JULIA_PLUGIN_ID}
      >
        <JuliaMark
          className={cn(
            'size-5',
            status.state === 'starting' && 'animate-pulse',
            status.state === 'unavailable' && 'grayscale',
          )}
        />
      </PluginActivityButton>
      <PopoverContent
        side="right"
        align="end"
        className="w-80 gap-0 p-2.5"
      >
        <JuliaPluginPanel status={status} />
      </PopoverContent>
    </Popover>
  );
}

function PluginManagerPanel() {
  const { t } = useTranslation();
  const installedPluginIds = usePluginStore((state) => state.installedPluginIds);
  const uninstallPlugin = usePluginStore((state) => state.uninstallPlugin);
  const [installingPluginId, setInstallingPluginId] = useState<string | null>(null);
  const [query, setQuery] = useState('');

  const install = useCallback(async (manifest: PluginManifest) => {
    if (manifest.id !== JULIA_PLUGIN_ID) {
      return;
    }

    setInstallingPluginId(manifest.id);
    try {
      await installJuliaPlugin(t);
    } finally {
      setInstallingPluginId(null);
    }
  }, [t]);

  const normalizedQuery = query.trim().toLocaleLowerCase();
  const visibleManifests = BUILT_IN_PLUGIN_MANIFESTS.filter((manifest) => {
    if (!normalizedQuery) {
      return true;
    }

    return [manifest.id, t(manifest.titleKey), t(manifest.descriptionKey)]
      .some((value) => value.toLocaleLowerCase().includes(normalizedQuery));
  });

  return (
    <div data-workbench-plugin-manager-content className="-m-0.5 overflow-hidden">
      <PopoverHeader className="border-b border-border/70 bg-muted/20 p-4">
        <PopoverTitle>{t('plugins.manager.title')}</PopoverTitle>
        <PopoverDescription>{t('plugins.manager.description')}</PopoverDescription>
      </PopoverHeader>
      <div className="space-y-3 p-3">
        <Input
          data-workbench-plugin-search
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t('plugins.manager.searchPlaceholder')}
          aria-label={t('plugins.manager.searchPlaceholder')}
        />
        {visibleManifests.length === 0 ? (
          <p className="px-1 py-2 text-muted-foreground">{t('plugins.manager.noResults')}</p>
        ) : null}
        {visibleManifests.map((manifest) => {
          const installed = installedPluginIds.includes(manifest.id);
          const installing = installingPluginId === manifest.id;

          return (
            <div
              key={manifest.id}
              data-workbench-plugin-card={manifest.id}
              className="flex items-center gap-3 rounded-md border border-border/70 bg-background/40 p-2.5"
            >
              <JuliaMark className="size-7 rounded-md" />
              <div className="min-w-0 flex-1">
                <p className="text-xs font-medium text-foreground">{t(manifest.titleKey)}</p>
                <p className="mt-0.5 text-[11px] text-muted-foreground">
                  {t(manifest.descriptionKey)}
                </p>
              </div>
              {installed ? (
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={() => uninstallPlugin(manifest.id)}
                >
                  {t('plugins.manager.remove')}
                </Button>
              ) : (
                <Button
                  type="button"
                  size="sm"
                  disabled={installing}
                  onClick={() => void install(manifest)}
                >
                  {installing ? t('plugins.manager.installing') : t('plugins.manager.install')}
                </Button>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function PluginManagerActivitySlot() {
  const { t } = useTranslation();

  return (
    <Popover>
      <PluginActivityButton
        label={t('plugins.manager.title')}
        dataSlot="manager"
      >
        <VscExtensions size={18} />
      </PluginActivityButton>
      <PopoverContent
        side="right"
        align="end"
        className="w-80 gap-0 p-2.5"
      >
        <PluginManagerPanel />
      </PopoverContent>
    </Popover>
  );
}

export function PluginActivityActions() {
  const installedPluginIds = usePluginStore((state) => state.installedPluginIds);
  const installedManifests = getInstalledPluginManifests(installedPluginIds);

  return (
    <div
      data-workbench-plugin-actions
      className="flex flex-col items-center"
      onPointerDown={stopControlPropagation}
      onMouseDown={stopControlPropagation}
    >
      <PluginManagerActivitySlot />
      {installedManifests.map((manifest) => (
        manifest.id === JULIA_PLUGIN_ID
          ? <JuliaPluginActivitySlot key={manifest.id} />
          : null
      ))}
    </div>
  );
}

import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  FiChevronDown,
  FiChevronUp,
  FiFilter,
  FiSearch,
  FiTrash2,
} from 'react-icons/fi';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { ToolbarIconButton } from '@/shared/ui/ToolbarIconButton';
import { DIAGNOSTIC_LEVELS, type DiagnosticLevel } from '@/shared/types/domain/diagnostics';
import {
  getLogLevelBackground,
  getLogLevelColor,
} from './logPresentation';
import { useLogWorkspaceContext } from './logWorkspaceContext';

const LOG_FILTER_LEVELS: readonly DiagnosticLevel[] = DIAGNOSTIC_LEVELS;

export function LogPanelToolbar() {
  const { t } = useTranslation();
  const [filterOpen, setFilterOpen] = useState(false);
  const {
    loading,
    filter,
    autoScroll,
    setAutoScroll,
    toggleLevel,
    setSearchText,
    refreshLogs,
    clearLogs,
  } = useLogWorkspaceContext();

  return (
    <div
      className="flex h-full max-h-(--logs-tab-height) min-h-0 shrink-0 items-center gap-0.5"
      onPointerDown={(event) => event.stopPropagation()}
      onMouseDown={(event) => event.stopPropagation()}
    >
      <ToolbarIconButton
        type="button"
        variant="ghost"
        size="icon-sm"
        onClick={refreshLogs}
        disabled={loading}
        aria-label={t('log.refresh')}
        tooltip={t('log.refresh')}
      >
        <svg
          className={loading ? 'animate-spin' : undefined}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
          aria-hidden
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
          />
        </svg>
      </ToolbarIconButton>

      <ToolbarIconButton
        type="button"
        variant="ghost"
        size="icon-sm"
        onClick={() => setAutoScroll(!autoScroll)}
        aria-label={autoScroll ? t('log.autoScrollEnabled') : t('log.autoScrollDisabled')}
        aria-pressed={autoScroll}
        className={autoScroll ? 'text-primary' : 'text-muted-foreground'}
        tooltip={autoScroll ? t('log.autoScrollEnabled') : t('log.autoScrollDisabled')}
      >
        {autoScroll ? <FiChevronDown /> : <FiChevronUp />}
      </ToolbarIconButton>

      <Popover open={filterOpen} onOpenChange={setFilterOpen}>
        <Tooltip>
          <PopoverTrigger asChild>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                className={filterOpen ? 'text-primary' : 'text-muted-foreground'}
                aria-label={t('log.filter')}
                aria-pressed={filterOpen}
              >
                <FiFilter />
              </Button>
            </TooltipTrigger>
          </PopoverTrigger>
          <TooltipContent side="bottom">{t('log.filter')}</TooltipContent>
        </Tooltip>
        <PopoverContent
          align="end"
          className="w-72.5 gap-3 p-3"
          onClick={(event) => event.stopPropagation()}
        >
          <div className="relative">
            <FiSearch
              className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground"
              aria-hidden
            />
            <Input
              type="text"
              placeholder={t('log.searchPlaceholder')}
              aria-label={t('log.searchPlaceholder')}
              value={filter.searchText}
              onChange={(event) => setSearchText(event.target.value)}
              className="h-8 pl-9 text-xs"
            />
          </div>
          <div>
            <div className="mb-2 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
              {t('log.level')}
            </div>
            <div className="flex flex-nowrap gap-1.5">
              {LOG_FILTER_LEVELS.map((level) => {
                const enabled = filter.levels.has(level);
                return (
                  <Button
                    type="button"
                    variant={enabled ? 'secondary' : 'outline'}
                    size="sm"
                    key={level}
                    onClick={() => toggleLevel(level)}
                    aria-pressed={enabled}
                    className={`h-6 px-2 text-[10px] ${enabled
                      ? `${getLogLevelBackground(level)} ${getLogLevelColor(level)} border-current`
                      : 'text-muted-foreground'}`}
                  >
                    {level.toUpperCase()}
                  </Button>
                );
              })}
            </div>
          </div>
        </PopoverContent>
      </Popover>

      <ToolbarIconButton
        type="button"
        variant="ghost"
        size="icon-sm"
        onClick={clearLogs}
        aria-label={t('log.clear')}
        className="text-muted-foreground hover:text-destructive"
        tooltip={t('log.clear')}
      >
        <FiTrash2 />
      </ToolbarIconButton>
    </div>
  );
}

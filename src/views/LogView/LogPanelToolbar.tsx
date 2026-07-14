import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { FiTrash2, FiFilter, FiSearch, FiChevronDown, FiChevronUp, FiX } from 'react-icons/fi';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { ToolbarIconButton } from '@/shared/ui/ToolbarIconButton';
import type { LogLevel } from '@/shared/types/ui';
import {
  getLogLevelBackground,
  getLogLevelColor,
} from './logPresentation';
import { useLogPanelContext } from './logPanelContext';

export function LogPanelToolbar() {
  const { t } = useTranslation();
  const {
    loading,
    filter,
    isFilterOpen,
    setIsFilterOpen,
    autoScroll,
    setAutoScroll,
    filterButtonRef,
    filterPopoverRef,
    popoverPosition,
    toggleLevel,
    setSearchText,
    refreshLogs,
    clearLogs,
    handleClose,
    variant,
  } = useLogPanelContext();

  const getLevelColor = getLogLevelColor;
  const getLevelBgColor = getLogLevelBackground;

  return (
    <div
      className="flex shrink-0 items-center gap-0.5"
      onPointerDown={(e) => e.stopPropagation()}
      onMouseDown={(e) => e.stopPropagation()}
    >
      <ToolbarIconButton
        type="button"
        variant="ghost"
        size="icon-sm"
        onClick={() => refreshLogs()}
        disabled={loading}
        tooltip={t('log.refresh')}
      >
        <svg
          className={`h-3.5 w-3.5 ${loading ? 'animate-spin' : ''}`}
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
        className={autoScroll ? 'text-[var(--accent-color)]' : 'text-muted-foreground'}
        tooltip={autoScroll ? t('log.autoScrollEnabled') : t('log.autoScrollDisabled')}
      >
        {autoScroll ? <FiChevronDown size={14} /> : <FiChevronUp size={14} />}
      </ToolbarIconButton>

      <div className="relative">
        <ToolbarIconButton
          type="button"
          variant="ghost"
          size="icon-sm"
          ref={filterButtonRef}
          onClick={() => setIsFilterOpen(!isFilterOpen)}
          className={isFilterOpen ? 'text-[var(--accent-color)]' : 'text-muted-foreground'}
          tooltip={t('log.filter')}
        >
          <FiFilter size={14} />
        </ToolbarIconButton>

        {isFilterOpen
          && createPortal(
            <Card
              ref={filterPopoverRef}
              className="fixed z-[200] w-[280px] space-y-3 border-border/60 p-3 shadow-xl"
              style={{ top: popoverPosition.top, left: popoverPosition.left }}
              onClick={(e) => e.stopPropagation()}
            >
              <div className="relative">
                <FiSearch className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" size={14} />
                <Input
                  type="text"
                  placeholder={t('log.searchPlaceholder')}
                  value={filter?.searchText ?? ''}
                  onChange={(e) => setSearchText(e.target.value)}
                  className="h-8 pl-9 text-xs"
                />
              </div>
              <div>
                <div className="mb-2 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                  {t('log.level')}
                </div>
                <div className="flex flex-wrap gap-1.5">
                  {(['error', 'warn', 'info', 'debug', 'trace'] as LogLevel[]).map((level) => (
                    <Button
                      type="button"
                      variant={filter?.levels?.has(level) ? 'secondary' : 'outline'}
                      size="sm"
                      key={level}
                      onClick={() => toggleLevel(level)}
                      className={`h-6 px-2 text-[10px] ${filter?.levels?.has(level) ? `${getLevelBgColor(level)} ${getLevelColor(level)} border-current` : 'text-muted-foreground'}`}
                    >
                      {level.toUpperCase()}
                    </Button>
                  ))}
                </div>
              </div>
            </Card>,
            document.body,
          )}
      </div>

      <ToolbarIconButton
        type="button"
        variant="ghost"
        size="icon-sm"
        onClick={clearLogs}
        className="text-muted-foreground hover:text-destructive"
        tooltip={t('log.clear')}
      >
        <FiTrash2 size={14} />
      </ToolbarIconButton>

      <ToolbarIconButton
        type="button"
        variant="ghost"
        size="icon-sm"
        onClick={handleClose}
        tooltip={variant === 'embedded' ? t('log.closePanel') : t('log.closeWindow')}
      >
        <FiX size={14} />
      </ToolbarIconButton>
    </div>
  );
}

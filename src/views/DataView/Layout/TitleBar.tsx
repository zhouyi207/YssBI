import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { VscDatabase } from 'react-icons/vsc';
import { logger } from '@/utils/appLogger';
import { useEditStateStore } from '@/features/core/dataStore';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';

export interface DataframeOption {
  label: string;
  value: string;
}

interface TitleBarProps {
  dataframes: DataframeOption[];
  selectedDataframeId: string | null;
  onSelectDataframe: (id: string) => void;
  /** 当前选中单元格（或行/列选时的参考格）的文本预览 */
  selectedCellText: string;
}

const noDragStyle: React.CSSProperties = { WebkitAppRegion: 'no-drag' };

export const TitleBar: React.FC<TitleBarProps> = ({
  dataframes,
  selectedDataframeId,
  onSelectDataframe,
  selectedCellText,
}) => {
  const { t } = useTranslation();
  const editStateByDatabase = useEditStateStore((s) => s.editStateByDatabase);
  const [isMaximized, setIsMaximized] = useState(false);

  const labelWithDirtyMark = (id: string, label: string) =>
    editStateByDatabase[id]?.isModified ? `${label} *` : label;

  useEffect(() => {
    let disposed = false;
    let cleanup: (() => void) | null = null;
    const setup = async () => {
      const win = getCurrentWindow();
      const maximized = await win.isMaximized();
      if (!disposed) setIsMaximized(maximized);
      const unlisten = await win.onResized(async () => {
        if (!disposed) setIsMaximized(await win.isMaximized());
      });
      if (disposed) unlisten();
      else cleanup = unlisten;
    };
    setup().catch((e) => logger.app.error(String(e), 'DataViewTitleBar'));
    return () => {
      disposed = true;
      cleanup?.();
    };
  }, []);

  const handleMinimize = () => getCurrentWindow().minimize();
  const handleMaximize = () => getCurrentWindow().toggleMaximize();
  const handleClose = () => getCurrentWindow().close();

  const selectValue = selectedDataframeId && dataframes.some((o) => o.value === selectedDataframeId)
    ? selectedDataframeId
    : undefined;

  return (
    <div data-tauri-drag-region className="flex h-9 shrink-0 select-none items-center border-b border-border bg-background/95 z-50">
      <div className="flex min-w-0 flex-1 items-center gap-2 px-3" data-tauri-drag-region>
        <span className="flex size-5 shrink-0 items-center justify-center rounded-md bg-[var(--accent-color)]/10 text-[var(--accent-color)]">
          <VscDatabase size={14} />
        </span>
        <span className="shrink-0 text-[13px] font-semibold tracking-tight text-foreground">{t('dataView.title')}</span>
        <div
          className="min-w-0 flex-1 max-w-[min(360px,42vw)] pl-1"
          style={noDragStyle}
          onPointerDown={(e) => e.stopPropagation()}
        >
          {dataframes.length === 0 ? (
            <span className="text-xs text-muted-foreground">{t('dataView.noDataFrame')}</span>
          ) : selectValue === undefined ? (
            <span className="text-xs text-muted-foreground">{t('dataView.loadingProjectData')}</span>
          ) : (
            <Select value={selectValue} onValueChange={onSelectDataframe}>
              <SelectTrigger size="sm" className="h-7 border-border bg-muted/40 text-xs shadow-none">
                <SelectValue placeholder={t('dataView.noDataFrameSelected')} />
              </SelectTrigger>
              <SelectContent position="popper" className="z-[600] max-h-72">
                {dataframes.map((opt) => (
                  <SelectItem key={opt.value} value={opt.value} className="text-xs">
                    {labelWithDirtyMark(opt.value, opt.label)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          )}
        </div>
      </div>
      <div
        className="flex min-w-[10rem] max-w-xl flex-1 items-center px-2"
        style={noDragStyle}
        onPointerDown={(e) => e.stopPropagation()}
      >
        <Input
          readOnly
          value={selectedCellText}
          placeholder={t('dataView.cellPreviewPlaceholder')}
          className="h-7 w-full text-xs shadow-none"
          title={selectedCellText || undefined}
          aria-label={t('dataView.cellPreviewPlaceholder')}
        />
      </div>
      <div className="flex h-full shrink-0 items-center">
        <Button type="button" variant="ghost" size="icon-lg" onClick={handleMinimize} className="h-9 rounded-none text-muted-foreground" title={t('common.minimize')}>
          <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M20 12H4" /></svg>
        </Button>
        <Button type="button" variant="ghost" size="icon-lg" onClick={handleMaximize} className="h-9 rounded-none text-muted-foreground" title={isMaximized ? t('common.restore') : t('common.maximize')}>
          <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" strokeWidth={2} /></svg>
        </Button>
        <Button type="button" variant="ghost" size="icon-lg" onClick={handleClose} className="h-9 w-11 rounded-none text-muted-foreground hover:bg-red-600 hover:text-white" title={t('common.close')}>
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" /></svg>
        </Button>
      </div>
    </div>
  );
};

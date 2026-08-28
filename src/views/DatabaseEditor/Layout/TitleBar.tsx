import React from 'react';
import { useTranslation } from 'react-i18next';
import { VscDatabase } from 'react-icons/vsc';
import { Input } from '@/components/ui/input';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { useCurrentWindowActions } from '@/features/application/window';
import { useCustomTitleBar } from '@/features/application/window/useWindowDecorations';
import { WindowChromeControls } from '@/shared/ui/WindowChromeControls';
import { WindowTitleBar, WindowTitleBarActions } from '@/shared/ui/WindowTitleBar';
import { TAURI_NO_DRAG_STYLE, stopTauriDragPropagation } from '@/shared/platform/tauriWebview';

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

export const TitleBar: React.FC<TitleBarProps> = ({
  dataframes,
  selectedDataframeId,
  onSelectDataframe,
  selectedCellText,
}) => {
  const { t } = useTranslation();
  const windowActions = useCurrentWindowActions();
  const showCustomChrome = useCustomTitleBar();

  const selectValue = selectedDataframeId && dataframes.some((o) => o.value === selectedDataframeId)
    ? selectedDataframeId
    : undefined;

  const toolbarBody = (
    <>
      <div
        className="flex min-w-0 flex-1 items-center gap-2 px-3"
        {...(showCustomChrome ? { 'data-tauri-drag-region': true } : {})}
      >
        <span className="flex size-5 shrink-0 items-center justify-center rounded-md bg-[var(--accent-color)]/10 text-[var(--accent-color)]">
          <VscDatabase size={14} />
        </span>
        <span className="shrink-0 text-[13px] font-semibold tracking-tight text-foreground">{t('databaseEditor.title')}</span>
        <div
          className="min-w-0 flex-1 max-w-[min(360px,42vw)] pl-1"
          style={TAURI_NO_DRAG_STYLE}
          onPointerDown={stopTauriDragPropagation}
        >
          {dataframes.length === 0 ? (
            <span className="text-xs text-muted-foreground">{t('databaseEditor.noDataFrame')}</span>
          ) : selectValue === undefined ? (
            <span className="text-xs text-muted-foreground">{t('databaseEditor.loadingProjectData')}</span>
          ) : (
            <Select value={selectValue} onValueChange={onSelectDataframe}>
              <SelectTrigger size="sm" className="h-7 border-border bg-muted/40 text-xs shadow-none">
                <SelectValue placeholder={t('databaseEditor.noDataFrameSelected')} />
              </SelectTrigger>
              <SelectContent position="popper" className="z-[600] max-h-72">
                {dataframes.map((opt) => (
                  <SelectItem key={opt.value} value={opt.value} className="text-xs">
                    {opt.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          )}
        </div>
      </div>
      <div
        className="flex min-w-[10rem] max-w-xl flex-1 items-center px-2"
        style={TAURI_NO_DRAG_STYLE}
        onPointerDown={stopTauriDragPropagation}
      >
        <Tooltip>
          <TooltipTrigger asChild>
            <Input
              readOnly
              value={selectedCellText}
              placeholder={t('databaseEditor.cellPreviewPlaceholder')}
              className="h-7 w-full text-xs shadow-none"
              aria-label={t('databaseEditor.cellPreviewPlaceholder')}
            />
          </TooltipTrigger>
          {selectedCellText ? <TooltipContent side="bottom">{selectedCellText}</TooltipContent> : null}
        </Tooltip>
      </div>
    </>
  );

  if (!showCustomChrome) {
    return (
      <div className="z-50 flex h-10 shrink-0 items-stretch border-b border-border bg-[var(--workbench-bg)] shadow-xl">
        {toolbarBody}
      </div>
    );
  }

  return (
    <WindowTitleBar className="z-50">
      {toolbarBody}
      <WindowTitleBarActions>
        <WindowChromeControls
          maximized={windowActions.maximized}
          minimize={windowActions.minimize}
          toggleMaximize={windowActions.toggleMaximize}
          close={windowActions.close}
        />
      </WindowTitleBarActions>
    </WindowTitleBar>
  );
};

import React from 'react';
import { useTranslation } from 'react-i18next';
import { VscDatabase } from 'react-icons/vsc';
import { useEditStateStore } from '@/features/core/dataStore';
import { Input } from '@/components/ui/input';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { useWindowMaximized } from '@/features/application/window';
import { WindowChromeControls } from '@/shared/ui/WindowChromeControls';
import { WindowTitleBar, WindowTitleBarActions } from '@/shared/ui/WindowTitleBar';

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

const noDragStyle = { WebkitAppRegion: 'no-drag' } as React.CSSProperties;

export const TitleBar: React.FC<TitleBarProps> = ({
  dataframes,
  selectedDataframeId,
  onSelectDataframe,
  selectedCellText,
}) => {
  const { t } = useTranslation();
  const editStateByDatabase = useEditStateStore((s) => s.editStateByDatabase);
  const isMaximized = useWindowMaximized('DatabaseEditorTitleBar');

  const labelWithDirtyMark = (id: string, label: string) =>
    editStateByDatabase[id]?.isModified ? `${label} *` : label;

  const selectValue = selectedDataframeId && dataframes.some((o) => o.value === selectedDataframeId)
    ? selectedDataframeId
    : undefined;

  return (
    <WindowTitleBar className="z-50">
      <div className="flex min-w-0 flex-1 items-center gap-2 px-3" data-tauri-drag-region>
        <span className="flex size-5 shrink-0 items-center justify-center rounded-md bg-[var(--accent-color)]/10 text-[var(--accent-color)]">
          <VscDatabase size={14} />
        </span>
        <span className="shrink-0 text-[13px] font-semibold tracking-tight text-foreground">{t('databaseEditor.title')}</span>
        <div
          className="min-w-0 flex-1 max-w-[min(360px,42vw)] pl-1"
          style={noDragStyle}
          onPointerDown={(e) => e.stopPropagation()}
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
      <WindowTitleBarActions>
        <WindowChromeControls isMaximized={isMaximized} />
      </WindowTitleBarActions>
    </WindowTitleBar>
  );
};

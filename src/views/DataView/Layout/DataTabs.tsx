import React from 'react';
import { VscAdd, VscCheck, VscClose, VscDatabase } from 'react-icons/vsc';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { cn } from '@/lib/utils';

export interface DataframeOption {
  label: string;
  value: string;
}

interface DataTab {
  id: string;
  label: string;
  isModified: boolean;
}

interface DataTabsProps {
  tabs: DataTab[];
  options: DataframeOption[];
  activeTabId: string | null;
  onSelectTab: (id: string) => void;
  onAddTab: (id: string) => void;
  onCloseTab: (id: string) => void;
}

export const DataTabs: React.FC<DataTabsProps> = ({
  tabs,
  options,
  activeTabId,
  onSelectTab,
  onAddTab,
  onCloseTab,
}) => {
  return (
    <div className="flex h-10 shrink-0 items-center border-b border-border bg-background/95">
      <OverlayScrollbar direction="horizontal" className="flex min-w-0 flex-1 items-stretch">
        <div className="flex h-10 items-end px-2">
          {tabs.map((tab) => {
            const active = tab.id === activeTabId;
            return (
              <button
                key={tab.id}
                type="button"
                className={cn(
                  'group flex h-8 max-w-[220px] items-center gap-2 rounded-t-md border px-3 text-left text-xs transition-colors',
                  active
                    ? 'border-border border-b-card bg-card text-foreground shadow-sm'
                    : 'border-transparent text-muted-foreground hover:bg-muted/70 hover:text-foreground'
                )}
                onClick={() => onSelectTab(tab.id)}
              >
                <VscDatabase className={cn('size-3.5 shrink-0', active && 'text-[var(--accent-color)]')} />
                <span className="min-w-0 flex-1 truncate">{tab.label}</span>
                {tab.isModified && <span className="size-1.5 shrink-0 rounded-full bg-yellow-500" />}
                <span
                  role="button"
                  tabIndex={-1}
                  className="flex size-4 shrink-0 items-center justify-center rounded-sm text-muted-foreground opacity-0 transition-opacity hover:bg-muted hover:text-foreground group-hover:opacity-100"
                  onClick={(event) => {
                    event.stopPropagation();
                    onCloseTab(tab.id);
                  }}
                >
                  <VscClose size={12} />
                </span>
              </button>
            );
          })}
        </div>
      </OverlayScrollbar>

      <div className="flex h-full shrink-0 items-center gap-1 border-l border-border bg-background px-2">
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button type="button" variant="ghost" size="icon-sm" title="添加 DataFrame">
              <VscAdd size={15} />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-64">
            <DropdownMenuLabel>选择 DataFrame</DropdownMenuLabel>
            <DropdownMenuSeparator />
            {options.length === 0 ? (
              <DropdownMenuItem disabled className="text-xs">
                暂无可用 DataFrame
              </DropdownMenuItem>
            ) : (
              options.map((option) => {
                const opened = tabs.some((tab) => tab.id === option.value);
                return (
                  <DropdownMenuItem
                    key={option.value}
                    className="gap-2 text-xs"
                    onSelect={() => onAddTab(option.value)}
                  >
                    <VscDatabase className="size-3.5 text-muted-foreground" />
                    <span className="min-w-0 flex-1 truncate">{option.label}</span>
                    {opened && <VscCheck className="size-3.5 text-[var(--accent-color)]" />}
                  </DropdownMenuItem>
                );
              })
            )}
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  );
};

import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { VscSearch } from 'react-icons/vsc';
import { Input } from '@/components/ui/input';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { openInspectableResult } from '@/features/application/execution/openInspectableResult';
import {
  usePinResultSearch,
  type PinResultSearchEntry,
} from '@/features/application/execution/usePinResultSearch';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { cn } from '@/lib/utils';
import { addGlobalEventListener } from '@/shared/utils/globalEvent';

function formatPinResultLabel(entry: PinResultSearchEntry): string {
  return `${entry.nodeTitle} · ${entry.pinName}`;
}

function PinResultSearchRow({
  entry,
  onSelect,
}: {
  entry: PinResultSearchEntry;
  onSelect: (entry: PinResultSearchEntry) => void;
}) {
  const label = formatPinResultLabel(entry);

  return (
    <button
      type="button"
      title={entry.sourceTitle !== label ? entry.sourceTitle : label}
      className={cn(
        'block w-full truncate rounded-md px-2.5 py-1.5 text-left text-xs text-foreground transition-colors',
        'hover:bg-muted/80 focus-visible:bg-muted/80 focus-visible:outline-none',
      )}
      onClick={() => onSelect(entry)}
    >
      {label}
    </button>
  );
}

interface PinResultSearchProps {
  graphPath: string;
}

export function PinResultSearch({ graphPath }: PinResultSearchProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [shellMotion, setShellMotion] = useState<'idle' | 'expand' | 'collapse'>('idle');
  const [query, setQuery] = useState('');
  const rootRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const { hasResults, entries } = usePinResultSearch(graphPath, query);
  const showPanel = open && shellMotion !== 'expand';

  useEffect(() => {
    if (!open) {
      setQuery('');
      return;
    }
    if (shellMotion === 'expand') return;
    const frame = requestAnimationFrame(() => inputRef.current?.focus());
    return () => cancelAnimationFrame(frame);
  }, [open, shellMotion]);

  useEffect(() => {
    if (!open) return;

    const handlePointerDown = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (!target || rootRef.current?.contains(target)) return;
      setShellMotion('collapse');
      setOpen(false);
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setShellMotion('collapse');
        setOpen(false);
      }
    };

    const cleanupPointerDown = addGlobalEventListener(document, 'mousedown', handlePointerDown);
    const cleanupKeyDown = addGlobalEventListener(document, 'keydown', handleKeyDown);
    return () => {
      cleanupPointerDown();
      cleanupKeyDown();
    };
  }, [open]);

  const handleToggle = () => {
    if (!hasResults) return;
    if (open) {
      setShellMotion('collapse');
      setOpen(false);
      return;
    }
    setShellMotion('expand');
    setOpen(true);
  };

  const handleShellAnimationEnd = () => {
    setShellMotion('idle');
  };

  const handleSelect = (entry: PinResultSearchEntry) => {
    void openInspectableResult(entry.ref, t).then((opened) => {
      if (opened) {
        setShellMotion('collapse');
        setOpen(false);
      }
    });
  };

  const searchButton = (
    <button
      type="button"
      disabled={!hasResults}
      onClick={handleToggle}
      aria-expanded={open}
      className={cn(
        'flex size-7 shrink-0 items-center justify-center rounded-md transition-colors',
        'disabled:cursor-not-allowed disabled:opacity-40',
        hasResults
          ? 'text-blue-400 hover:bg-muted/60 hover:text-blue-300'
          : 'text-[var(--text-secondary)]',
      )}
      aria-label={
        !hasResults
          ? t('canvas.pinResultSearch.empty')
          : open
            ? t('canvas.pinResultSearch.close')
            : t('canvas.pinResultSearch.open')
      }
    >
      <VscSearch size={14} />
    </button>
  );

  return (
    <div ref={rootRef} className="menu-container inline-flex w-fit">
      <div
        className={cn(
          'pin-result-search-shell flex flex-col overflow-hidden rounded-md border border-[var(--border-color)] bg-[var(--panel-bg)]/80 shadow-lg backdrop-blur-sm',
          open && shellMotion === 'idle' && 'is-open',
          shellMotion === 'expand' && 'is-expanding',
          shellMotion === 'collapse' && 'is-collapsing',
        )}
        onAnimationEnd={handleShellAnimationEnd}
        onPointerDown={(event) => event.stopPropagation()}
      >
        <div className="flex items-center p-0.5">
          {open ? (
            searchButton
          ) : (
            <Tooltip>
              <TooltipTrigger asChild>{searchButton}</TooltipTrigger>
              <TooltipContent side="bottom">
                {hasResults ? t('canvas.pinResultSearch.open') : t('canvas.pinResultSearch.empty')}
              </TooltipContent>
            </Tooltip>
          )}

          <div
            className={cn(
              'pin-result-search-input flex min-w-0 flex-1 items-center overflow-hidden',
              open ? 'mr-1 opacity-100 delay-75' : 'pointer-events-none mr-0 w-0 opacity-0',
            )}
          >
            <Input
              ref={inputRef}
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={t('canvas.pinResultSearch.searchPlaceholder')}
              className="h-6 min-w-0 flex-1 border-0 bg-transparent px-1 text-xs shadow-none focus-visible:ring-0"
            />
          </div>
        </div>

        {showPanel ? (
          <div className="pin-result-search-panel border-t border-[var(--border-color)]">
            {entries.length === 0 ? (
              <div className="px-4 py-6 text-center text-xs italic text-muted-foreground">
                {t('canvas.pinResultSearch.noMatches')}
              </div>
            ) : (
              <OverlayScrollbar direction="vertical" className="max-h-64 py-1">
                <div className="flex flex-col gap-0.5 px-1 pb-1">
                  {entries.map((entry) => (
                    <PinResultSearchRow
                      key={entry.id}
                      entry={entry}
                      onSelect={handleSelect}
                    />
                  ))}
                </div>
              </OverlayScrollbar>
            )}
          </div>
        ) : null}
      </div>
    </div>
  );
}

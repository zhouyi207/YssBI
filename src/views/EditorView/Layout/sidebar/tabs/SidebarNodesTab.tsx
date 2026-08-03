import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/input';
import { useLocalizedNodeCatalog } from '@/features/application/nodeCatalog/useLocalizedNodeCatalog';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { SidebarTabPanel } from '../sections/SidebarTabPanel';

export function SidebarNodesTab() {
  const { t } = useTranslation();
  const { status, error, catalog, searchIndex } = useLocalizedNodeCatalog();
  const [query, setQuery] = useState('');
  const items = useMemo(
    () => (query.trim() && searchIndex ? searchIndex.search(query) : catalog?.items ?? []),
    [catalog, query, searchIndex],
  );

  return (
    <SidebarTabPanel>
      <div className="shrink-0 border-b border-border px-2 py-2">
        <Input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t('canvas.nodePalette.searchPlaceholder')}
          className="h-7"
        />
      </div>
      <OverlayScrollbar className="min-h-0 flex-1">
        <div className="space-y-1 p-2">
          {status === 'error' && !catalog ? (
            <p role="alert" className="px-2 py-3 text-sm text-destructive">
              {error ?? t('common.error')}
            </p>
          ) : !catalog ? (
            <p role="status" className="px-2 py-3 text-sm text-muted-foreground">
              {t('common.loading')}
            </p>
          ) : items.length === 0 ? (
            <p className="px-2 py-3 text-center text-sm text-muted-foreground">
              {t('canvas.nodePalette.noMatches')}
            </p>
          ) : items.map((item) => (
            <div
              key={item.creation.kind === 'static'
                ? `static:${item.nodeTypeId}`
                : `${item.creation.kind}:${item.nodeTypeId}:${item.creation.resourcePath}`}
              className="rounded-sm px-2 py-1.5"
            >
              <div className="truncate text-xs text-foreground">{item.title}</div>
              <div className="truncate font-mono text-[10px] text-muted-foreground">
                {item.nodeTypeId}
              </div>
            </div>
          ))}
        </div>
      </OverlayScrollbar>
    </SidebarTabPanel>
  );
}

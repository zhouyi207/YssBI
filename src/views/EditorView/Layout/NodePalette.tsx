import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useLocalizedNodeCatalog } from '@/features/application/nodeCatalog/useLocalizedNodeCatalog';
import type { LocalizedCatalogItem } from '@/features/domain/nodeCatalog/catalogItem';
import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';

export function nodePaletteItemKey(item: LocalizedCatalogItem): string {
  return item.creation.kind === 'resourceBound'
    ? `${item.creation.kind}:${item.nodeTypeId}:${item.creation.resourcePath}`
    : `${item.creation.kind}:${item.nodeTypeId}`;
}

export function NodePalette({
  x,
  y,
  onSelect,
}: {
  x: number;
  y: number;
  onSelect: (descriptor: NodeCreationDescriptor, locale: string) => void;
}) {
  const { t } = useTranslation();
  const { status, error, catalog, searchIndex } = useLocalizedNodeCatalog();
  const [query, setQuery] = useState('');
  const items = useMemo(() => searchIndex?.search(query) ?? [], [query, searchIndex]);

  return (
    <Card
      className="menu-container fixed z-50 flex max-h-112 w-80 min-h-0 flex-col gap-2 overflow-hidden p-2 text-sm shadow-2xl animate-zoom-in"
      style={{ left: x, top: y }}
      onPointerDown={(event) => event.stopPropagation()}
    >
      {status === 'error' && (!catalog || !searchIndex) ? (
        <p role="alert" className="px-2 py-1 text-destructive">
          {error ?? t('common.error')}
        </p>
      ) : !catalog || !searchIndex ? (
        <p role="status" className="px-2 py-1 text-muted-foreground">
          {t('common.loading')}
        </p>
      ) : (
        <>
          <Input
            autoFocus
            className="h-8 shrink-0"
            value={query}
            placeholder={t('canvas.nodePalette.searchPlaceholder')}
            onChange={(event) => setQuery(event.target.value)}
          />
          <OverlayScrollbar className="max-h-80 min-h-0 flex-1">
            <div className="space-y-2 pr-2">
              {items.length === 0 ? (
                <p className="px-2 py-3 text-center text-muted-foreground">
                  {t('canvas.nodePalette.noMatches')}
                </p>
              ) : catalog.categories.map((category) => {
                const categoryItems = items.filter((item) => item.categoryId === category.categoryId);
                if (categoryItems.length === 0) return null;

                return (
                  <section key={category.categoryId} aria-labelledby={`node-palette-${category.categoryId}`}>
                    <h3
                      id={`node-palette-${category.categoryId}`}
                      className="px-2 py-1 text-xs font-medium text-muted-foreground"
                    >
                      {category.title}
                    </h3>
                    <div className="space-y-0.5">
                      {categoryItems.map((item) => (
                        <Button
                          key={nodePaletteItemKey(item)}
                          type="button"
                          variant="ghost"
                          className="h-auto w-full justify-start rounded-sm px-2 py-1.5 text-left font-normal"
                          onClick={() => onSelect(item.creation, catalog.locale)}
                        >
                          {item.title}
                        </Button>
                      ))}
                    </div>
                  </section>
                );
              })}
            </div>
          </OverlayScrollbar>
        </>
      )}
    </Card>
  );
}

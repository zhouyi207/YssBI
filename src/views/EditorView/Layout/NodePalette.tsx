import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useCompatibleNodeCatalog } from '@/features/application/nodeCatalog/useCompatibleNodeCatalog';
import { useLocalizedNodeCatalog } from '@/features/application/nodeCatalog/useLocalizedNodeCatalog';
import type { LocalizedCatalogItem } from '@/features/domain/nodeCatalog/catalogItem';
import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';
import {
  buildLocalizedCatalogTree,
  type LocalizedCatalogTreeNode,
} from '@/features/domain/nodeCatalog/localizedCatalogTree';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Empty, EmptyHeader, EmptyTitle } from '@/components/ui/empty';
import { Input } from '@/components/ui/input';
import { ScrollArea } from '@/components/ui/scroll-area';

export function nodePaletteItemKey(item: LocalizedCatalogItem): string {
  return item.creation.kind === 'resourceBound'
    ? `${item.creation.kind}:${item.nodeTypeId}:${item.creation.resourcePath}`
    : `${item.creation.kind}:${item.nodeTypeId}`;
}

function CatalogCategorySection({
  node,
  depth,
  locale,
  onSelect,
}: {
  node: LocalizedCatalogTreeNode;
  depth: number;
  locale: string;
  onSelect: (descriptor: NodeCreationDescriptor, locale: string) => void;
}) {
  return (
    <section
      data-catalog-category-id={node.category.categoryId}
      data-catalog-depth={depth}
      aria-labelledby={`node-palette-${node.category.categoryId}`}
    >
      <h3
        id={`node-palette-${node.category.categoryId}`}
        className="py-1 text-xs font-medium text-muted-foreground"
        style={{ paddingLeft: `${depth * 12 + 8}px` }}
      >
        {node.category.title}
      </h3>
      <div className="space-y-0.5">
        {node.items.map((item) => (
          <Button
            key={nodePaletteItemKey(item)}
            type="button"
            variant="ghost"
            className="h-auto w-full justify-start rounded-sm py-1.5 text-left font-normal"
            style={{ paddingLeft: `${depth * 12 + 8}px`, paddingRight: '8px' }}
            onClick={() => onSelect(item.creation, locale)}
          >
            {item.title}
          </Button>
        ))}
      </div>
      {node.children.map((child) => (
        <CatalogCategorySection
          key={child.category.categoryId}
          node={child}
          depth={depth + 1}
          locale={locale}
          onSelect={onSelect}
        />
      ))}
    </section>
  );
}

export function NodePalette({
  x,
  y,
  graphPath = null,
  graphRevision = null,
  sourcePort = null,
  onSelect,
}: {
  x: number;
  y: number;
  graphPath?: string | null;
  graphRevision?: number | null;
  sourcePort?: PortAddressDto | null;
  onSelect: (descriptor: NodeCreationDescriptor, locale: string) => void;
}) {
  const { t } = useTranslation();
  const localized = useLocalizedNodeCatalog(sourcePort === null);
  const compatible = useCompatibleNodeCatalog({
    enabled: sourcePort !== null,
    graphPath,
    graphRevision,
    sourcePort,
  });
  const { status, error, catalog, searchIndex } = sourcePort ? compatible : localized;
  const [query, setQuery] = useState('');
  const items = useMemo(() => searchIndex?.search(query) ?? [], [query, searchIndex]);
  const categoryTree = useMemo(
    () => catalog ? buildLocalizedCatalogTree(catalog.categories, items) : [],
    [catalog, items],
  );

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
          <ScrollArea className="max-h-80 min-h-0 flex-1">
            <div className="space-y-2 pr-2">
              {items.length === 0 ? (
                <Empty className="gap-1 rounded-none px-2 py-4">
                  <EmptyHeader>
                    <EmptyTitle className="text-xs font-normal text-muted-foreground">
                      {t('canvas.nodePalette.noMatches')}
                    </EmptyTitle>
                  </EmptyHeader>
                </Empty>
              ) : categoryTree.map((node) => (
                <CatalogCategorySection
                  key={node.category.categoryId}
                  node={node}
                  depth={0}
                  locale={catalog.locale}
                  onSelect={onSelect}
                />
              ))}
            </div>
          </ScrollArea>
        </>
      )}
    </Card>
  );
}

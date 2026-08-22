import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { VscBook, VscClose, VscSearch } from 'react-icons/vsc';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty';
import { Input } from '@/components/ui/input';
import { nodeCatalogErrorText } from '@/features/application/nodeCatalog/nodeCatalogErrorPresentation';
import { useLocalizedNodeCatalog } from '@/features/application/nodeCatalog/useLocalizedNodeCatalog';
import {
  catalogItemKey,
  type LocalizedCatalogItem,
} from '@/features/domain/nodeCatalog/catalogItem';
import { MarkdownRenderer } from '@/shared/ui/MarkdownRenderer';
import { ScrollArea } from '@/components/ui/scroll-area';
import { detailProseClass } from './Detail/shared/detailStyles';

interface NodeDocumentationModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

function normalizeDocumentationQuery(value: string): string {
  return value
    .normalize('NFKD')
    .replace(/\p{Mark}/gu, '')
    .toLowerCase()
    .replace(/[^\p{Letter}\p{Number}]+/gu, ' ')
    .trim();
}

function searchDocumentationItems(
  items: readonly LocalizedCatalogItem[],
  query: string,
): LocalizedCatalogItem[] {
  const terms = normalizeDocumentationQuery(query).split(' ').filter(Boolean);
  if (terms.length === 0) return [...items];
  return items.filter((item) => {
    const text = normalizeDocumentationQuery([item.title, ...item.aliases].join(' '));
    return terms.every((term) => text.includes(term));
  });
}

function MetadataRow({ label, value }: { label: string; value: string | number }) {
  return (
    <div className="grid grid-cols-[8rem_minmax(0,1fr)] gap-3 text-xs">
      <dt className="font-medium text-muted-foreground">{label}</dt>
      <dd className="break-all font-mono text-foreground">{value}</dd>
    </div>
  );
}

function ItemDetails({ item }: { item: LocalizedCatalogItem }) {
  const { t } = useTranslation();

  return (
    <div className="space-y-5 px-5 py-4">
      <div className="space-y-1">
        <h2 className="text-lg font-semibold text-foreground">{item.title}</h2>
        {item.description ? <p className="text-sm text-muted-foreground">{item.description}</p> : null}
      </div>

      <dl className="space-y-2">
        <MetadataRow label={t('nodeDocumentationModal.nodeId')} value={item.nodeTypeId} />
        {item.resourcePath !== undefined ? (
          <MetadataRow label={t('nodeDocumentationModal.resourcePath')} value={item.resourcePath} />
        ) : null}
        {item.resourceRevision !== undefined ? (
          <MetadataRow label={t('nodeDocumentationModal.resourceRevision')} value={item.resourceRevision} />
        ) : null}
      </dl>

      <section className="space-y-2">
        <h3 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          {t('nodeDocumentationModal.ports')}
        </h3>
        {item.ports.length === 0 ? (
          <p className="text-xs text-muted-foreground">{t('nodeDocumentationModal.noPorts')}</p>
        ) : (
          <div className="space-y-1.5">
            {item.ports.map((port) => (
              <div key={port.key} className="flex items-center gap-2 rounded-md border border-border px-3 py-2 text-sm">
                <span className="min-w-0 flex-1">{port.label}</span>
                <code className="text-xs text-muted-foreground">{port.key}</code>
                <Badge variant="secondary">{port.direction}</Badge>
                <Badge variant="outline">{port.kind}</Badge>
              </div>
            ))}
          </div>
        )}
      </section>

      <section className="space-y-2">
        <h3 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          {t('nodeDocumentationModal.parameters')}
        </h3>
        {item.parameters.length === 0 ? (
          <p className="text-xs text-muted-foreground">{t('nodeDocumentationModal.noParameters')}</p>
        ) : (
          <div className="space-y-1.5">
            {item.parameters.map((parameter) => (
              <div key={parameter.key} className="rounded-md border border-border px-3 py-2">
                <div className="flex items-center justify-between gap-2 text-sm">
                  <span>{parameter.title}</span>
                  <code className="text-xs text-muted-foreground">{parameter.key}</code>
                </div>
                {parameter.description ? (
                  <p className="mt-1 text-xs text-muted-foreground">{parameter.description}</p>
                ) : null}
              </div>
            ))}
          </div>
        )}
      </section>

      <section className="space-y-2">
        <h3 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          {t('detail.nodeDoc.documentation')}
        </h3>
        {item.documentation ? (
          <div className={detailProseClass}>
            <MarkdownRenderer markdown={item.documentation} />
          </div>
        ) : (
          <p className="text-sm text-muted-foreground">{t('nodeDocumentationModal.noDocumentation')}</p>
        )}
      </section>
    </div>
  );
}

export function NodeDocumentationModal({ open, onOpenChange }: NodeDocumentationModalProps) {
  const { t } = useTranslation();
  const { status, error, catalog } = useLocalizedNodeCatalog();
  const [query, setQuery] = useState('');
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const items = useMemo(
    () => searchDocumentationItems(catalog?.items ?? [], query),
    [catalog, query],
  );
  const selectedItem = catalog?.items.find((item) => catalogItemKey(item) === selectedKey) ?? null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex h-[min(760px,86vh)] min-h-0 max-w-[min(1120px,92vw)] flex-col gap-0 p-0">
        <DialogHeader className="shrink-0 border-b border-border px-5 py-4">
          <div className="flex items-start justify-between gap-4">
            <div className="min-w-0 space-y-1">
              <DialogTitle className="normal-case tracking-normal">
                {t('nodeDocumentationModal.title')}
              </DialogTitle>
              <DialogDescription>{t('nodeDocumentationModal.description')}</DialogDescription>
            </div>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              onClick={() => onOpenChange(false)}
              aria-label={t('nodeDocumentationModal.close')}
            >
              <VscClose size={18} />
            </Button>
          </div>
        </DialogHeader>

        <div className="flex min-h-0 flex-1">
          <aside className="flex w-72 min-h-0 shrink-0 flex-col gap-3 border-r border-border p-3">
            <Input
              className="h-8 shrink-0"
              value={query}
              placeholder={t('nodeDocumentationModal.searchPlaceholder')}
              onChange={(event) => setQuery(event.target.value)}
            />
            <ScrollArea className="min-h-0 flex-1">
              <div className="space-y-1 pr-2">
                {status === 'error' && !catalog ? (
                  <p role="alert" className="px-2 py-3 text-sm text-destructive">
                    {nodeCatalogErrorText(error, t)}
                  </p>
                ) : !catalog ? (
                  <p role="status" className="px-2 py-3 text-sm text-muted-foreground">
                    {t('common.loading')}
                  </p>
                ) : items.length === 0 ? (
                  <Empty className="min-h-32 gap-2 rounded-none px-2 py-4">
                    <EmptyHeader>
                      <EmptyMedia variant="icon" className="text-muted-foreground">
                        <VscSearch />
                      </EmptyMedia>
                      <EmptyTitle className="text-xs font-normal text-muted-foreground">
                        {t('nodeDocumentationModal.noMatches')}
                      </EmptyTitle>
                    </EmptyHeader>
                  </Empty>
                ) : items.map((item) => {
                  const key = catalogItemKey(item);
                  const selected = key === selectedKey;
                  return (
                    <Button
                      key={key}
                      type="button"
                      variant={selected ? 'secondary' : 'ghost'}
                      className="h-auto w-full justify-start rounded-sm px-2 py-2 text-left font-normal"
                      data-node-documentation-item={key}
                      onClick={() => setSelectedKey(selected ? null : key)}
                    >
                      <span className="min-w-0">
                        <span className="block truncate">{item.title}</span>
                        <span className="block truncate font-mono text-[10px] text-muted-foreground">
                          {item.nodeTypeId}
                        </span>
                      </span>
                    </Button>
                  );
                })}
              </div>
            </ScrollArea>
          </aside>

          <main className="flex min-h-0 min-w-0 flex-1 flex-col">
            <ScrollArea className="min-h-0 flex-1">
              {selectedItem ? (
                <ItemDetails item={selectedItem} />
              ) : (
                <Empty className="min-h-full rounded-none p-6">
                  <EmptyHeader>
                    <EmptyMedia variant="icon" className="size-10 text-muted-foreground">
                      <VscBook className="size-5" />
                    </EmptyMedia>
                    <EmptyTitle>{t('nodeDocumentationModal.selectNode')}</EmptyTitle>
                    <EmptyDescription>{t('nodeDocumentationModal.description')}</EmptyDescription>
                  </EmptyHeader>
                </Empty>
              )}
            </ScrollArea>
          </main>
        </div>
      </DialogContent>
    </Dialog>
  );
}

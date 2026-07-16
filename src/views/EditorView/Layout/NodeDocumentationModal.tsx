import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import ReactMarkdown from 'react-markdown';
import remarkMath from 'remark-math';
import rehypeKatex from 'rehype-katex';
import { VscCheck, VscClose, VscSearch } from 'react-icons/vsc';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { useNodeRegistryStore } from '@/features/core/nodeRegister';
import {
  searchNodeDocumentation,
  type NodeDocumentationSearchResult,
} from '@/features/domain/nodeCatalog';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { detailProseClass } from './Detail/shared/detailStyles';
import 'katex/dist/katex.min.css';

interface NodeDocumentationModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function NodeDocumentationModal({ open, onOpenChange }: NodeDocumentationModalProps) {
  const { t, i18n } = useTranslation();
  const definitions = useNodeRegistryStore((s) => s.definitionsArray);
  const [query, setQuery] = useState('');
  const [selectedNodeType, setSelectedNodeType] = useState<string | null>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);

  const results = useMemo(
    () => searchNodeDocumentation(definitions, query, i18n.language),
    [definitions, query, i18n.language],
  );
  const selected = results.find((result) => result.nodeType === selectedNodeType) ?? null;

  useEffect(() => {
    if (!open) return;
    setQuery('');
    setSelectedNodeType(null);
    const timer = window.setTimeout(() => searchInputRef.current?.focus(), 50);
    return () => window.clearTimeout(timer);
  }, [open]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex h-[min(680px,84vh)] max-w-[min(1024px,92vw)] flex-col gap-0 p-0">
        <DialogHeader className="shrink-0 border-b border-border px-5 py-4">
          <div className="flex items-start justify-between gap-4">
            <div className="min-w-0 space-y-1">
              <DialogTitle className="normal-case tracking-normal">{t('nodeDocumentationModal.title')}</DialogTitle>
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
          <div className="mt-3 flex items-center gap-2 rounded-md border border-border bg-background px-2">
            <VscSearch className="shrink-0 text-muted-foreground" size={16} aria-hidden />
            <Input
              ref={searchInputRef}
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={t('nodeDocumentationModal.searchPlaceholder')}
              className="h-9 border-0 bg-transparent px-0 shadow-none focus-visible:ring-0"
            />
            <kbd className="rounded border border-border bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">F1</kbd>
          </div>
        </DialogHeader>

        <div className="flex min-h-0 flex-1">
          <div
            className={selected
              ? 'h-full w-72 shrink-0 border-r border-border transition-[width] duration-200'
              : 'flex h-full min-w-0 flex-1 transition-[width] duration-200'}
          >
            <OverlayScrollbar direction="vertical" className="h-full">
              {results.length ? (
                <div className="p-2">
                  {results.map((result) => (
                    <SearchResultRow
                      key={result.nodeType}
                      result={result}
                      selected={selected?.nodeType === result.nodeType}
                      onSelect={() => setSelectedNodeType((current) => current === result.nodeType ? null : result.nodeType)}
                    />
                  ))}
                </div>
              ) : (
                <p className="px-4 py-8 text-center text-sm text-muted-foreground">
                  {t('nodeDocumentationModal.noMatches')}
                </p>
              )}
            </OverlayScrollbar>
          </div>

          {selected ? (
            <div className="min-w-0 flex-1">
              <OverlayScrollbar direction="vertical" className="h-full">
                <DocumentationDetail result={selected} />
              </OverlayScrollbar>
            </div>
          ) : null}
        </div>
      </DialogContent>
    </Dialog>
  );
}

function SearchResultRow({
  result,
  selected,
  onSelect,
}: {
  result: NodeDocumentationSearchResult;
  selected: boolean;
  onSelect: () => void;
}) {
  return (
    <Button
      type="button"
      variant="ghost"
      onClick={onSelect}
      aria-current={selected ? 'true' : undefined}
      className={`h-auto w-full justify-start rounded-md px-3 py-2 text-left ${selected ? 'bg-accent text-accent-foreground' : ''}`}
    >
      <span className="flex w-full min-w-0 items-center gap-2">
        <span className="min-w-0 flex-1">
          <span className="block truncate text-sm font-medium">{result.name}</span>
          <span className="mt-0.5 block truncate text-xs text-muted-foreground">
            {result.category.join(' / ') || result.nodeType}
          </span>
        </span>
        {selected ? <VscCheck className="shrink-0" size={16} aria-hidden /> : null}
      </span>
    </Button>
  );
}

function DocumentationDetail({ result }: { result: NodeDocumentationSearchResult }) {
  const { t } = useTranslation();
  const content = result.documentation ?? result.description;

  return (
    <article className="mx-auto max-w-3xl p-6">
      <p className="mb-1 text-xs font-medium uppercase tracking-wide text-muted-foreground">
        {result.category.join(' / ') || result.nodeType}
      </p>
      <h2 className="text-xl font-semibold text-foreground">{result.name}</h2>
      <p className="mt-1 font-mono text-xs text-muted-foreground">{result.nodeType}</p>
      {content ? (
        <div className={`mt-6 ${detailProseClass}`}>
          <ReactMarkdown remarkPlugins={[remarkMath]} rehypePlugins={[rehypeKatex]}>
            {content}
          </ReactMarkdown>
        </div>
      ) : (
        <p className="mt-6 text-sm text-muted-foreground">{t('nodeDocumentationModal.noDocumentation')}</p>
      )}
    </article>
  );
}

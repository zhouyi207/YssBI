import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Card } from '@/components/ui/card';
import { Empty, EmptyHeader, EmptyTitle } from '@/components/ui/empty';
import { ScrollArea } from '@/components/ui/scroll-area';
import { cn } from '@/lib/utils';
import { useDismissableOverlay } from '@/shared/ui/positionedOverlay';

export interface NodeSelectionOption {
  id: string;
  title: string;
}

interface NodeSelectionPaletteProps {
  position: { x: number; y: number };
  nodes: readonly NodeSelectionOption[];
  currentNodeId?: string;
  onSelectNode: (nodeId: string) => void;
  onClose: () => void;
}

export function NodeSelectionPalette({
  position,
  nodes,
  currentNodeId,
  onSelectNode,
  onClose,
}: NodeSelectionPaletteProps) {
  const { t } = useTranslation();
  const paletteRef = useRef<HTMLDivElement>(null);
  const initialIndex = useMemo(() => {
    const currentIndex = currentNodeId
      ? nodes.findIndex((node) => node.id === currentNodeId)
      : -1;
    return currentIndex >= 0 ? currentIndex : 0;
  }, [currentNodeId, nodes]);
  const [activeIndex, setActiveIndex] = useState(initialIndex);

  useDismissableOverlay({ ref: paletteRef, onDismiss: onClose });

  useEffect(() => {
    setActiveIndex(initialIndex);
  }, [initialIndex]);

  useEffect(() => {
    paletteRef.current?.focus();
  }, []);

  const moveActive = (offset: number) => {
    if (nodes.length === 0) return;
    setActiveIndex((current) => Math.max(0, Math.min(nodes.length - 1, current + offset)));
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
      event.preventDefault();
      event.stopPropagation();
      moveActive(event.key === 'ArrowUp' ? -1 : 1);
      return;
    }
    if (event.key === 'Enter') {
      event.preventDefault();
      event.stopPropagation();
      const activeNode = nodes[activeIndex];
      if (activeNode) onSelectNode(activeNode.id);
    }
  };

  return (
    <Card
      ref={paletteRef}
      role="listbox"
      tabIndex={-1}
      aria-label={t('contextMenu.node.selectNode')}
      className="menu-container fixed z-50 flex max-h-96 w-72 min-h-0 flex-col gap-1 overflow-hidden p-1.5 text-sm shadow-2xl outline-none"
      style={{ left: position.x, top: position.y }}
      onKeyDown={handleKeyDown}
      onPointerDown={(event) => event.stopPropagation()}
    >
      <div className="shrink-0 border-b border-border/60 px-1 pb-1.5 text-xs font-medium text-muted-foreground">
        {t('contextMenu.node.selectNode')}
      </div>
      <ScrollArea className="min-h-0 flex-1">
        {nodes.length === 0 ? (
          <Empty className="gap-1 rounded-md px-2 py-4">
            <EmptyHeader>
              <EmptyTitle className="text-xs font-normal text-muted-foreground">
                {t('contextMenu.node.noNodes')}
              </EmptyTitle>
            </EmptyHeader>
          </Empty>
        ) : (
          <div className="flex flex-col gap-0.5 pr-1">
            {nodes.map((node, index) => {
              const active = index === activeIndex;
              return (
                <button
                  key={node.id}
                  type="button"
                  role="option"
                  aria-selected={active}
                  data-node-selection-option
                  data-node-id={node.id}
                  className={cn(
                    'flex min-h-7 w-full items-center rounded-sm px-2 text-left text-xs outline-none transition-colors',
                    active
                      ? 'bg-accent text-accent-foreground'
                      : 'text-foreground hover:bg-accent/60',
                  )}
                  onMouseEnter={() => setActiveIndex(index)}
                  onClick={() => onSelectNode(node.id)}
                >
                  <span className="min-w-0 truncate">{node.title || node.id}</span>
                </button>
              );
            })}
          </div>
        )}
      </ScrollArea>
    </Card>
  );
}

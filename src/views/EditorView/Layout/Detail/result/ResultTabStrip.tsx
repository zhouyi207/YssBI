import type { CSSProperties } from 'react';
import { useTranslation } from 'react-i18next';
import {
  DndContext,
  PointerSensor,
  closestCenter,
  useDraggable,
  useDroppable,
  useSensor,
  useSensors,
  type DragEndEvent,
} from '@dnd-kit/core';
import { VscClose } from 'react-icons/vsc';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';
import { cn } from '@/lib/utils';
import { portAddressKey } from '@/features/domain/editorProjection';
import type { ResultTabRecord } from '@/features/core/resultWorkspace';

interface ResultTabStripProps {
  tabs: ResultTabRecord[];
  activeTabKey: string | null;
  onActivate(tabKey: string): void;
  onClose(tabKey: string): void;
  onMove(tabKey: string, targetTabKey: string): void;
}

function DraggableResultTab({
  tab,
  active,
  closeLabel,
  onActivate,
  onClose,
}: {
  tab: ResultTabRecord;
  active: boolean;
  closeLabel: string;
  onActivate(): void;
  onClose(): void;
}) {
  const draggable = useDraggable({ id: tab.tabKey });
  const droppable = useDroppable({ id: tab.tabKey });
  const setNodeRef = (node: HTMLDivElement | null) => {
    draggable.setNodeRef(node);
    droppable.setNodeRef(node);
  };
  const style: CSSProperties | undefined = draggable.transform
    ? { transform: `translate3d(${draggable.transform.x}px, ${draggable.transform.y}px, 0)` }
    : undefined;

  return (
    <div
      ref={setNodeRef}
      style={style}
      className={cn(
        'flex h-8 shrink-0 items-center border-r border-border bg-muted/20',
        active && 'bg-background text-foreground',
        draggable.isDragging && 'z-10 opacity-70',
      )}
    >
      <button
        {...draggable.attributes}
        {...draggable.listeners}
        type="button"
        role="tab"
        aria-selected={active}
        title={tab.source
          ? `${tab.source.graphPath} · ${portAddressKey(tab.source.port)}`
          : tab.resultId}
        className="h-full max-w-44 truncate px-2 text-xs"
        onClick={onActivate}
      >
        {tab.title || tab.resultId}
      </button>
      <Button
        type="button"
        variant="ghost"
        size="icon-xs"
        aria-label={closeLabel}
        onPointerDown={(event) => event.stopPropagation()}
        onClick={(event) => {
          event.stopPropagation();
          onClose();
        }}
      >
        <VscClose />
      </Button>
    </div>
  );
}

export function ResultTabStrip({
  tabs,
  activeTabKey,
  onActivate,
  onClose,
  onMove,
}: ResultTabStripProps) {
  const { t } = useTranslation();
  const sensors = useSensors(useSensor(PointerSensor, {
    activationConstraint: { distance: 4 },
  }));
  const handleDragEnd = ({ active, over }: DragEndEvent) => {
    if (over && active.id !== over.id) onMove(String(active.id), String(over.id));
  };

  return (
    <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
      <ScrollArea orientation="horizontal" className="shrink-0 border-b border-border">
        <div
          role="tablist"
          aria-label={t('detail.result.tabsAriaLabel')}
          className="flex min-w-max"
        >
          {tabs.map((tab) => (
            <DraggableResultTab
              key={tab.tabKey}
              tab={tab}
              active={tab.tabKey === activeTabKey}
              closeLabel={t('detail.result.closeTab', { title: tab.title || tab.resultId })}
              onActivate={() => onActivate(tab.tabKey)}
              onClose={() => onClose(tab.tabKey)}
            />
          ))}
        </div>
      </ScrollArea>
    </DndContext>
  );
}

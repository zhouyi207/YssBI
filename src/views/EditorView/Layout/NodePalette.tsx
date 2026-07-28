import type { Pin, Variable } from '@/shared/types/domain';
import type { FunctionResourceView } from '@/features/core/resource/functionResourceView';
import type { NodeCatalogItem } from '@/features/domain/nodeCatalog';
import { Card } from '@/components/ui/card';
import { NODE_CATALOG_UNAVAILABLE_MESSAGE } from '@/features/application/editor/editorMutationAvailability';

export function NodePalette({
  x,
  y,
}: {
  x: number;
  y: number;
  onSelect: (item: NodeCatalogItem) => void;
  filterPin?: Pin | null;
  variables?: Record<string, Variable>;
  functions?: Record<string, FunctionResourceView>;
  graphKind?: 'event' | 'function';
  graphPath?: string;
}) {
  return (
    <Card
      className="menu-container fixed z-50 w-80 p-4 text-sm text-muted-foreground shadow-2xl animate-zoom-in"
      style={{ left: x, top: y }}
      onPointerDown={(event) => event.stopPropagation()}
    >
      {NODE_CATALOG_UNAVAILABLE_MESSAGE}
    </Card>
  );
}

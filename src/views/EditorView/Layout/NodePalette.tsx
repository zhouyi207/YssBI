import { useMemo } from 'react';
import type { Graph, Pin, Variable } from '@/shared/types/domain';
import { buildContextualCatalogItems, type NodeCatalogItem } from '@/features/domain/nodeCatalog';
import { useNodeRegistryStore } from '@/features/core/nodeRegister';
import { Card } from '@/components/ui/card';
import { NodeCatalogTreeView } from './nodeCatalog/NodeCatalogTreeView';

/** @deprecated Use NodeCatalogItem */
export type PaletteItem = NodeCatalogItem;

export function NodePalette({
  x,
  y,
  onSelect,
  filterPin,
  variables = {},
  functions = {},
}: {
  x: number;
  y: number;
  onSelect: (item: PaletteItem) => void;
  filterPin?: Pin | null;
  variables?: Record<string, Variable>;
  functions?: Record<string, Graph>;
}) {
  const definitions = useNodeRegistryStore((s) => s.definitionsArray);

  const variableKeysStr = useMemo(() => Object.keys(variables).sort().join(','), [variables]);
  const functionKeysStr = useMemo(() => Object.keys(functions).sort().join(','), [functions]);

  const items = useMemo(
    () =>
      buildContextualCatalogItems({
        definitions,
        filterPin,
        variables,
        functions,
      }),
    [definitions, filterPin, variableKeysStr, functionKeysStr, variables, functions],
  );

  return (
    <Card
      className="menu-container fixed z-50 flex w-80 flex-col overflow-hidden shadow-2xl animate-zoom-in"
      style={{ left: x, top: y }}
      onPointerDown={(e) => e.stopPropagation()}
    >
      <NodeCatalogTreeView
        items={items}
        variant="popover"
        onLeafClick={onSelect}
        autoFocusSearch
      />
    </Card>
  );
}
